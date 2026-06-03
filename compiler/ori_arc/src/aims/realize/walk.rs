//! Unified forward walk for Phase 1 realization.
//!
//! One traversal per block. Routes all RC and reuse decisions through
//! [`decide()`] and collects death/alloc events inline, eliminating the
//! separate scans in `collect_death_events()` and `collect_alloc_events()`.
//!
//! Replaces `emit_rc/forward_walk::emit_body_forward_walk()` with a walk
//! that calls `decide()` per (var, instruction) site and produces both
//! RC operations and reuse event data in a single pass.
//!
//! Post-instruction dec emission and death event collection live in
//! [`super::walk_dec`].

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::emit_rc::{is_live_at_exit, rc_strategy, BlockCtx, LastUse};
use crate::aims::emit_reuse::{ctor_to_shape, is_reusable_ctor, AllocEvent, DeathEvent};
use crate::aims::intraprocedural::state_map::ApplyAliasSource;
use crate::aims::lattice::SizeClass;
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, RcAtomicity, RcStrategy, ValueRepr};

use super::decide::{decide, DecisionContext, DecisionSite, RcDecision, UseSemantics};
use super::walk_dec::{emit_post_instr_decs_unified, is_rc_managed};

/// Result of the unified forward walk on a single block's body.
pub(super) struct BodyWalkResult {
    /// Deferred parent `RcDec` operations whose borrowed children have
    /// later uses (for edge cleanup or terminator-exit emission).
    pub terminator_deferred: Vec<(ArcVarId, RcStrategy)>,
    /// Death events: `RcDec` sites with reuse potential (replaces
    /// `collect_death_events()` scan).
    pub death_events: Vec<DeathEvent>,
    /// Allocation events: `Construct` instructions with reusable constructors
    /// (replaces `collect_alloc_events()` scan).
    pub alloc_events: Vec<AllocEvent>,
    /// Synergy metrics accumulated during this block's walk.
    pub walk_metrics: super::metrics::SynergyMetrics,
}

/// Phase B: unified forward walk through body instructions.
///
/// Replaces `emit_body_forward_walk()` in `emit_rc/forward_walk.rs`.
/// Routes all RC/reuse decisions through `decide()` and collects
/// death/alloc events for the reuse planner inline.
///
/// # Decision routing
///
/// Every RC decision (Inc, Dec, Defer, Skip) and every reuse candidacy
/// check is made by a single `decide()` call per (var, instruction) site.
/// This replaces the scattered inline logic in `emit_pre_instr_incs()`,
/// `emit_post_instr_decs()`, and `collect_death_events()`.
#[expect(clippy::too_many_lines, reason = "pre-existing")]
pub(super) fn walk_body_unified(
    ctx: &BlockCtx<'_>,
    old_body: &[ArcInstr],
    new_body: &mut Vec<ArcInstr>,
    iter_fn_name: ori_ir::Name,
    initial_deferred: Vec<(ArcVarId, RcStrategy, LastUse)>,
) -> BodyWalkResult {
    let mut uses_so_far: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut deferred: Vec<(ArcVarId, RcStrategy, LastUse)> = initial_deferred;
    let mut death_events = Vec::new();
    let mut alloc_events = Vec::new();
    let mut metrics = super::metrics::SynergyMetrics::default();

    // Pre-compute: is this block an unwind cleanup block?
    // Unwind blocks end with Resume. Their explicit RcDec instructions
    // must be kept to balance callee-internal RcIncs (e.g., the RcInc
    // emitted at iterator creation sites).
    let is_unwind_block = matches!(
        ctx.func.blocks[ctx.blk.index()].terminator,
        crate::ir::ArcTerminator::Resume
    );

    for (instr_idx, instr) in old_body.iter().enumerate() {
        // INVARIANT: Burden* instructions are TF-N/A metadata annotations —
        // transparent to the predicate-stack realize walk. Counting them as
        // uses corrupts last-use computation and produces RcInc/RcDec emission
        // decisions at burden-op positions vs actual code-level last uses,
        // yielding leaks (BurdenInc/BurdenDec are no-op codegen markers per
        // RE-1 scalar exemption and cannot substitute for suppressed RcDec).
        if matches!(
            instr,
            ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. }
                | ArcInstr::BurdenDecPartial { .. }
                | ArcInstr::BurdenDecVariant { .. }
                | ArcInstr::BurdenDecField { .. }
        ) {
            // Preserve the burden instruction; do not run use-tracking,
            // RcInc/RcDec decisions, or alloc-event collection on it.
            new_body.push(instr.clone());
            continue;
        }

        // Inline alloc event collection (replaces collect_alloc_events scan).
        collect_alloc_event(ctx, instr, instr_idx, &mut alloc_events);

        // Sub-phase A: RcInc before uses with future uses.
        emit_pre_instr_incs_unified(
            ctx,
            instr,
            instr_idx,
            &mut uses_so_far,
            new_body,
            &mut metrics,
            iter_fn_name,
        );

        // Skip explicit RcDec for parameter-borrowed variables in normal
        // (non-unwind) blocks. When a borrowed collection parameter is
        // used in a for-loop, the lowering emits an explicit RcDec at the
        // loop exit. But the caller already handles cleanup via the
        // own→borrow reconciliation RcDec. Keeping both would double-dec.
        //
        // In unwind blocks (Resume terminator), keep the explicit RcDec —
        // it balances the callee's internal RcInc for the iterator.
        if let ArcInstr::RcDec { var, .. } = instr {
            let in_all = ctx.all_borrowed_defs.contains(var);
            let in_proj = ctx.project_borrowed_defs.contains(var);
            if !is_unwind_block && in_all && !in_proj {
                continue;
            }
            // Return-transfer dec suppression: suppress scope-exit
            // dec on Owned params that flow directly to a Return terminator
            // on this path. Path-sensitive: only fires when the current
            // block's forward CFG terminates in `Return { value: v }` where
            // `v` aliases the param. Sibling paths that don't return the
            // param still emit the dec normally.
            if crate::aims::emit_rc::should_suppress_return_transfer_dec(
                ctx,
                *var,
                ctx.blk,
                is_unwind_block,
            ) {
                continue;
            }
        }

        // Push the instruction itself.
        new_body.push(instr.clone());

        // Return-transfer compensating RcInc: emit compensating
        // RcInc immediately after a Project whose dst flows to Return AND
        // whose source resolves to an Owned param with
        // `return_alias = Some(Project { field })` matching this Project's
        // field. Without this Inc, the param's scope-exit
        // `RcDec param [AggFields]` walks fields and decrements the projected
        // allocation BEFORE the Return — caller receives a freed pointer.
        // The map is precomputed by `build_return_project_inc_targets` in
        // `emit_unified.rs` so the walk only does an O(1) hashmap lookup
        // per Project. Empty map (common case: no contract or no Project
        // return_alias) short-circuits before lookup.
        if let ArcInstr::Project { dst, .. } = instr {
            if let Some(&strategy) = ctx.return_project_inc_targets.get(dst) {
                new_body.push(ArcInstr::RcInc {
                    var: *dst,
                    count: 1,
                    strategy,
                    atomicity: RcAtomicity::default_atomic(),
                });
            }
        }

        // Select-aware compensating RcInc:
        //
        // `Select cond ? %x : %y` aliases dst to one of {x, y} at
        // runtime (AIMS §1.9 conditional alias). RL-2 emits per-source
        // last-use decs on x and y; each dec hits its operand's
        // allocation. When dst aliases the chosen operand, the dec on
        // chosen frees dst's allocation → UAF on downstream consumer
        // (Return, Apply Owned, Construct arg).
        //
        // Fix: emit RcInc(dst) BETWEEN the Select instruction and the
        // per-source decs. Compensates the chosen's RC against its
        // source dec; the unchosen's source dec balances normally.
        //
        // Path-sensitivity: emit only when at least one operand will
        // actually receive a per-source dec at this Select (last-use
        // here AND not Session-E return-transfer-suppressed). Without
        // this gate, an Inc would over-increment when both operand
        // decs are suppressed.
        if let ArcInstr::Select {
            dst,
            true_val,
            false_val,
            ..
        } = instr
        {
            if needs_select_compensating_inc(
                ctx,
                *dst,
                *true_val,
                *false_val,
                instr_idx,
                is_unwind_block,
            ) {
                if let Some(strategy) = rc_strategy(ctx.func, *dst, ctx.pool) {
                    new_body.push(ArcInstr::RcInc {
                        var: *dst,
                        count: 1,
                        strategy,
                        atomicity: RcAtomicity::default_atomic(),
                    });
                }
            }
        }

        // Sub-phase B: RcDec for defined-dead and last-use variables.
        emit_post_instr_decs_unified(
            ctx,
            instr,
            instr_idx,
            new_body,
            &mut deferred,
            &mut death_events,
            &mut metrics,
        );

        // Emit deferred parent decs whose children's last use is this instruction.
        // Payload-of-ancestor gating: even when a deferred dec's effective last-use lands
        // here, suppress the dec when an inter-class transitive-drop ancestor
        // will cover the var's RC slot. The deferred path is a distinct
        // emission site from `walk_dec.rs` (which gates defined-dead and
        // inline last-use); without the payload-of-ancestor gate here, the canonical dec leaks past
        // the suppression.
        let pin6_classes_dying_here =
            super::walk_dec::collect_classes_dying_here(ctx, instr, instr_idx);
        deferred.retain(|&(var, strategy, effective_last)| {
            if effective_last == LastUse::Body(instr_idx)
                && !is_live_at_exit(ctx.state_map, ctx.blk, var)
            {
                if let Some(class_id) = ctx.state_map.ssa_alias_class_of(var) {
                    if super::walk_dec::pin6_any_ancestor_will_cover(
                        ctx,
                        class_id,
                        var,
                        instr_idx,
                        &pin6_classes_dying_here,
                    ) {
                        return false;
                    }
                }
                new_body.push(ArcInstr::RcDec {
                    var,
                    strategy,
                    atomicity: RcAtomicity::default_atomic(),
                });
                false
            } else {
                true
            }
        });
    }

    let terminator_deferred = deferred
        .into_iter()
        .map(|(var, strategy, _)| (var, strategy))
        .collect();

    BodyWalkResult {
        terminator_deferred,
        death_events,
        alloc_events,
        walk_metrics: metrics,
    }
}

/// Collect an allocation event if the instruction is a `Construct` with a
/// reusable constructor.
fn collect_alloc_event(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    alloc_events: &mut Vec<AllocEvent>,
) {
    if let ArcInstr::Construct { dst, ty, ctor, .. } = instr {
        if is_reusable_ctor(ctor) && !ctx.state_map.is_excluded(*dst) {
            alloc_events.push(AllocEvent {
                block: ctx.blk,
                instr_idx,
                dst: *dst,
                ty: *ty,
                shape: ctor_to_shape(ctor),
                size_class: SizeClass::UNKNOWN,
            });
        }
    }
}

/// Pre-instruction: `RcInc` for variable uses with future uses.
///
/// Routes decisions through `decide()` with `DecisionSite::Use`.
fn emit_pre_instr_incs_unified(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    uses_so_far: &mut FxHashMap<ArcVarId, usize>,
    new_body: &mut Vec<ArcInstr>,
    metrics: &mut super::metrics::SynergyMetrics,
    iter_fn_name: ori_ir::Name,
) {
    // Special case: when a parameter-borrowed collection is passed to
    // an `@iter()` call, emit RcInc to balance the iterator's Drop
    // (which calls ori_buffer_rc_dec). Detect via the iter_fn_name
    // precomputed in walk_body_unified.
    if let ArcInstr::Apply { func, args, .. } = instr {
        if *func == iter_fn_name {
            if let Some(&coll_var) = args.first() {
                if ctx.all_borrowed_defs.contains(&coll_var)
                    && !ctx.project_borrowed_defs.contains(&coll_var)
                    && ctx.func.var_reprs[coll_var.index()] != ValueRepr::Scalar
                {
                    if let Some(strategy) = rc_strategy(ctx.func, coll_var, ctx.pool) {
                        new_body.push(ArcInstr::RcInc {
                            var: coll_var,
                            count: 1,
                            strategy,
                            atomicity: RcAtomicity::default_atomic(),
                        });
                    }
                }
            }
        }
    }

    for (pos, var) in instr.used_vars().into_iter().enumerate() {
        // Force RcInc for project-borrowed variables at owned call
        // positions. A Project-derived variable (borrowed ref to parent
        // aggregate's field) passed to an owned parameter must be
        // RcInc'd: the callee takes ownership of the data but the
        // Project didn't increment RC. Without this Inc, both the
        // parent aggregate and the callee's collection hold references
        // to the same data, causing double-free on cleanup.
        //
        // Example: `for w in words yield w` — the element `w` is
        // Project-derived (borrowed from iterator state). When yielded
        // via `ori_list_push(..., w [own], ...)`, `w`'s data pointer is
        // copied into the new list. Without RcInc, both the original
        // collection and the yield result point to the same str data
        // with RC=1, causing double-free.
        //
        // This check runs BEFORE is_rc_managed() because project-borrowed
        // variables are not considered "owned" by the state map and would
        // be filtered out by is_rc_managed().
        if instr.is_owned_position(pos)
            && ctx.project_borrowed_defs.contains(&var)
            && ctx.func.var_reprs[var.index()] != ValueRepr::Scalar
        {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy,
                    atomicity: RcAtomicity::default_atomic(),
                });
                metrics.total_rc_decisions += 1;
            }
            continue;
        }

        if !is_rc_managed(ctx, var) {
            continue;
        }

        let count = uses_so_far.entry(var).or_insert(0);
        *count += 1;

        let has_future_use = compute_has_future_use(ctx, var, *count, instr_idx);
        let semantics = classify_use_semantics(ctx, var, pos, instr);
        // Burden-coexistence handshake: defer to the burden walk when var's
        // SSA-alias class is fully burden-covered.
        let class_covered = ctx
            .state_map
            .is_class_covered(ctx.state_map.class_id_of(var));
        let decision = decide(&DecisionContext {
            site: DecisionSite::Use {
                has_future_use,
                semantics,
            },
            is_rc_managed: true,
            class_covered,
        });

        if decision.rc != RcDecision::None {
            metrics.total_rc_decisions += 1;
        }

        if decision.rc == RcDecision::Inc {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy,
                    atomicity: RcAtomicity::default_atomic(),
                });
            }
        }
    }
}

/// Collect the WITHIN-CLASS retained-copy ROOTS for `ctx.blk` into `out`. A
/// retained-copy root is the `dst` of a `Let { dst, Var(src) }` whose `src` use
/// fires an establishment `RcInc` (the use-site `decide(DecisionSite::Use) ==
/// Inc` path, RL-1) — i.e. an alias that took its OWN owned reference of the
/// heap value and so needs its OWN dec (vs a pure renaming that shares one
/// reference). Cross-class incs (iter-fn balance, project-borrowed-at-owned-arg)
/// are NOT recorded: their reference leaves the SSA-alias-class, so the enclosing
/// value's drop balances them — they create no within-class retained copy.
///
/// Faithful read-only mirror of [`emit_pre_instr_incs_unified`]'s use-site inc
/// decision (same `uses_so_far` + `compute_has_future_use` + `decide` chain,
/// burden-skipped to stay in lock-step with `precompute_block_uses`). Runs
/// pre-walk on the un-RC'd body. Consumed (after Let-alias closure) by the
/// lineage-equality dec-suppression gate so a class partitions into N retained
/// lineages + 1 alloc-ref lineage, each deduped to exactly one dec per path
/// (`1 + N` total — 04B.2-under-elim.lean `rc_per_path_invariant`).
pub(crate) fn predict_retained_roots(
    ctx: &BlockCtx<'_>,
    iter_fn_name: ori_ir::Name,
    out: &mut FxHashSet<ArcVarId>,
) {
    let block = &ctx.func.blocks[ctx.blk.index()];
    let mut uses_so_far: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for (instr_idx, instr) in block.body.iter().enumerate() {
        // Burden annotations are TF-N/A metadata, not real uses — skip them so
        // `uses_so_far` stays in lock-step with `precompute_block_uses`.
        if matches!(
            instr,
            ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. }
                | ArcInstr::BurdenDecPartial { .. }
                | ArcInstr::BurdenDecVariant { .. }
                | ArcInstr::BurdenDecField { .. }
        ) {
            continue;
        }
        // iter-fn balance inc: cross-class (balances the iterator's Drop), not a
        // within-class retained copy — does NOT advance `uses_so_far`, skip.
        if let ArcInstr::Apply { func, .. } = instr {
            if *func == iter_fn_name {
                // no within-class retained root
            }
        }
        for (pos, var) in instr.used_vars().into_iter().enumerate() {
            // project-borrowed-at-owned-position inc: cross-class; keep the
            // `continue` so `uses_so_far` matches the emitter, but record no root.
            if instr.is_owned_position(pos)
                && ctx.project_borrowed_defs.contains(&var)
                && ctx.func.var_reprs[var.index()] != ValueRepr::Scalar
            {
                continue;
            }
            if !is_rc_managed(ctx, var) {
                continue;
            }
            let count = uses_so_far.entry(var).or_insert(0);
            *count += 1;
            let has_future_use = compute_has_future_use(ctx, var, *count, instr_idx);
            let semantics = classify_use_semantics(ctx, var, pos, instr);
            let class_covered = ctx
                .state_map
                .is_class_covered(ctx.state_map.class_id_of(var));
            let decision = decide(&DecisionContext {
                site: DecisionSite::Use {
                    has_future_use,
                    semantics,
                },
                is_rc_managed: true,
                class_covered,
            });
            // The inc fires for the USED var; the within-class retained copy it
            // creates is the `Let` dst that takes the bumped reference. Record
            // that dst as a retained root (the Let-alias closure in
            // `build_lineage_map` extends it to descendants).
            if decision.rc == RcDecision::Inc && rc_strategy(ctx.func, var, ctx.pool).is_some() {
                if let ArcInstr::Let {
                    dst,
                    value: crate::ir::ArcValue::Var(src),
                    ..
                } = instr
                {
                    if *src == var {
                        out.insert(*dst);
                    }
                }
            }
        }
    }
}

/// Build the per-var retained-lineage map from the retained-copy ROOTS: each
/// retained root maps to itself, then `Let { dst, Var(src) }` renamings inherit
/// their source's root (forward fixpoint over the whole function). A var ABSENT
/// from the result is in the alloc-reference lineage. `lineage_of(a) ==
/// lineage_of(b)` (both `None`, or both `Some(same root)`) is the dec-suppression gate's
/// same-lineage predicate. Closure follows `Let`-Var renamings ONLY (same value,
/// same class); `Project`/`Select`/Jump-phi vars stay alloc-reference
/// (conservative — they may carry either reference at runtime).
pub(crate) fn build_lineage_map(
    func: &ArcFunction,
    retained_roots: &FxHashSet<ArcVarId>,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut lineage: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for &r in retained_roots {
        lineage.insert(r, r);
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: crate::ir::ArcValue::Var(src),
                    ..
                } = instr
                {
                    if let Some(&root) = lineage.get(src) {
                        // A retained root keeps its OWN lineage — the Let-alias
                        // closure extends only to non-root borrow-view
                        // descendants. Overwriting a distinct root's
                        // self-mapping merges separate owned references (each
                        // from a distinct establishment inc) into one lineage
                        // and under-counts decs on alias chains.
                        if !retained_roots.contains(dst) && lineage.insert(*dst, root).is_none() {
                            changed = true;
                        }
                    }
                }
            }
        }
        // Apply/Invoke-result passthrough closure: a Direct/Project apply-result
        // aliases the SAME allocation as its source arg (callee returns the arg
        // or one projection of it), so it carries that arg's retained reference
        // and inherits its lineage. Without this, the passthrough result falls
        // into the alloc-reference lineage with the root's own alias, and the
        // dec-suppression gate dedups two distinct owned references to one
        // (under-emission leak). Wrapped (separate wrapper allocation) and
        // Conditional (path-dependent) do NOT extend — their decs are scheduled
        // by the apply-aliased-dec suppression machinery.
        for (dst, source) in apply_result_aliases {
            let src = match source {
                ApplyAliasSource::Direct(arg) | ApplyAliasSource::Project { arg, .. } => *arg,
                ApplyAliasSource::Wrapped(_) | ApplyAliasSource::Conditional { .. } => continue,
            };
            if let Some(&root) = lineage.get(&src) {
                if !retained_roots.contains(dst) && lineage.insert(*dst, root).is_none() {
                    changed = true;
                }
            }
        }
    }
    lineage
}

// Helpers

/// Compute whether a variable has a future use from this point.
///
/// True if: remaining block uses > 0, or terminator use pending, or
/// live at block exit.
#[inline]
fn compute_has_future_use(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    uses_so_far: usize,
    instr_idx: usize,
) -> bool {
    if let Some(&(total_uses, last_use)) = ctx.use_info.get(&var) {
        debug_assert!(
            uses_so_far <= total_uses,
            "uses_so_far ({uses_so_far}) exceeds total_uses ({total_uses}) for {var:?}"
        );
        let remaining_in_block = total_uses.saturating_sub(uses_so_far);
        let live = is_live_at_exit(ctx.state_map, ctx.blk, var);
        remaining_in_block > 0
            || (matches!(last_use, LastUse::Terminator) && LastUse::Body(instr_idx) != last_use)
            || live
    } else {
        false
    }
}

/// Classify use semantics for a variable at an instruction site.
///
/// Determines whether the use is a normal RC use, a `Project` source
/// (borrowing vs transfer), or an `ApplyIndirect` closure receiver
/// (borrowing).
///
/// Let aliases (`%dst = %src`) use Normal semantics — the standard
/// `has_future_use` check provides correct `RcInc` placement.
/// `is_ownership_transfer()` handles the Dec side (suppressing last-use
/// Dec for the source at the alias instruction).
///
/// `pos` is the variable's position in `instr.used_vars()` (0-indexed).
/// Position-aware matching is required for `ApplyIndirect`: closure-as-arg
/// patterns like `ApplyIndirect %5(%3, %3)` (where %3 is both receiver and
/// first arg) must classify the receiver at pos 0 as `BorrowingApplyClosure`
/// while the arg at pos 1 stays `Normal` so it gets standard owned-arg RC.
/// Var-equality matching `*closure == var` would mis-suppress the arg-side
/// inc.
fn classify_use_semantics(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    pos: usize,
    instr: &ArcInstr,
) -> UseSemantics {
    // Project source classification (Lean 4 `proj i x` semantics):
    // - Scalar result → borrowing (no Inc for source)
    // - Non-scalar result → transfer (no Inc, suppress source Dec)
    if let ArcInstr::Project { value, dst, .. } = instr {
        if *value == var {
            if ctx.func.var_reprs[dst.index()] == ValueRepr::Scalar {
                return UseSemantics::BorrowingProject;
            }
            return UseSemantics::TransferProject;
        }
    }

    // ApplyIndirect closure receiver: borrow at the call site (Lean 4
    // `pap.app x`, Koka CheckFBIP, Swift SIL `apply` thick function
    // semantics). Match by POSITION, not var-equality — the closure may
    // also appear as an arg, in which case the arg occurrence stays
    // Normal so it gets the standard owned-arg Inc/Dec handling.
    if matches!(instr, ArcInstr::ApplyIndirect { .. }) && pos == 0 {
        return UseSemantics::BorrowingApplyClosure;
    }

    // Let { Var(src) } SSA alias for a closure handle: the alias and
    // source share the same RC slot (per TF-11 transparent-alias
    // semantics — IA-5 step (1) transfers state via seq_add without RC
    // change). Pre-fix the realize layer treated these as Normal with
    // has_future_use, emitting a per-alias Inc even though the alias
    // is just SSA renaming of the same allocation.
    // For closure types specifically (RcStrategy::Closure), the alias
    // chain leads to ApplyIndirect call sites which borrow the closure
    // (BorrowingApplyClosure semantics above); no per-alias Inc is
    // needed — the closure was alloc'd at PartialApply with RC=1 and
    // is freed by the LastUse Dec at scope exit.
    //
    // The match is gated to RcStrategy::Closure to avoid changing
    // semantics for non-closure aliases (str / list / struct), where
    // the existing Normal-with-future-use behavior is correct
    // (per the existing comment about Let alias Inc placement).
    if matches!(
        instr,
        ArcInstr::Let {
            value: ArcValue::Var(_),
            ..
        }
    ) && matches!(
        rc_strategy(ctx.func, var, ctx.pool),
        Some(RcStrategy::Closure)
    ) {
        return UseSemantics::BorrowingApplyClosure;
    }

    UseSemantics::Normal
}

/// Whether a `Select` instruction needs a compensating `RcInc(dst)` for
/// path-sensitive RC balance per AIMS §1.9 + RL-2.
///
/// Returns `true` when:
/// - `dst` is RC-managed (non-scalar, not excluded by `AimsStateMap`).
/// - `dst` has a downstream consumer (future use in block OR live at exit).
/// - At least one source operand (`true_val` or `false_val`) will receive
///   a per-source `RcDec` at this Select (last-use here AND not suppressed
///   by the return-transfer gate).
///
/// The compensating `RcInc(dst)` keeps the chosen operand's allocation
/// alive across its source dec (the chosen operand IS dst at runtime per
/// AIMS §1.9 conditional alias), so the consumer's eventual dec frees a
/// single live RC instead of double-decrementing freed memory.
fn needs_select_compensating_inc(
    ctx: &BlockCtx<'_>,
    dst: ArcVarId,
    true_val: ArcVarId,
    false_val: ArcVarId,
    instr_idx: usize,
    is_unwind_block: bool,
) -> bool {
    if ctx.state_map.is_excluded(dst) {
        return false;
    }
    if ctx.func.var_reprs[dst.index()] == ValueRepr::Scalar {
        return false;
    }
    let dst_has_consumer =
        ctx.use_info.contains_key(&dst) || is_live_at_exit(ctx.state_map, ctx.blk, dst);
    if !dst_has_consumer {
        return false;
    }

    let operand_emits_dec = |v: ArcVarId| -> bool {
        if !is_rc_managed(ctx, v) {
            return false;
        }
        // Two cases need compensation:
        //   (a) Synthetic dec via `emit_post_instr_decs_unified` at the
        //       Select instruction itself (last-use of operand at this
        //       instr).
        //   (b) Explicit RcDec on the operand AFTER the Select in the
        //       same block (lowering-emitted, common for match-dispatch
        //       and select<T> shapes).
        // In both cases, the dec on the operand will hit the chosen's
        // allocation at runtime → compensate via Inc on the Select dst.
        let block = &ctx.func.blocks[ctx.blk.index()];
        let has_explicit_dec_after = block.body.iter().enumerate().any(|(i, instr)| {
            i > instr_idx
                && matches!(
                    instr,
                    ArcInstr::RcDec { var, .. } if *var == v
                )
        });
        let has_synthetic_dec = ctx
            .use_info
            .get(&v)
            .is_some_and(|&(_total, last_use)| last_use == LastUse::Body(instr_idx))
            && !is_live_at_exit(ctx.state_map, ctx.blk, v)
            && !crate::aims::emit_rc::should_suppress_return_transfer_dec(
                ctx,
                v,
                ctx.blk,
                is_unwind_block,
            );
        has_explicit_dec_after || has_synthetic_dec
    };

    operand_emits_dec(true_val) || operand_emits_dec(false_val)
}
