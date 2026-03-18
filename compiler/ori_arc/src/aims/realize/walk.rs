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

use rustc_hash::FxHashMap;

use crate::aims::emit_rc::{is_live_at_exit, rc_strategy, BlockCtx, LastUse};
use crate::aims::emit_reuse::{ctor_to_shape, is_reusable_ctor, AllocEvent, DeathEvent};
use crate::aims::lattice::SizeClass;
use crate::ir::{ArcInstr, ArcVarId, RcStrategy, ValueRepr};

use super::decide::{decide, DecisionContext, DecisionSite, RcDecision, UseSemantics};
use super::walk_dec::{emit_post_instr_decs_unified, is_rc_managed};

/// Result of the unified forward walk on a single block's body.
pub(super) struct BodyWalkResult {
    /// Use counts accumulated during the walk (for Phase C terminator RC).
    pub uses_so_far: FxHashMap<ArcVarId, usize>,
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
pub(super) fn walk_body_unified(
    ctx: &BlockCtx<'_>,
    old_body: &[ArcInstr],
    new_body: &mut Vec<ArcInstr>,
    iter_fn_name: ori_ir::Name,
) -> BodyWalkResult {
    let mut uses_so_far: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut deferred: Vec<(ArcVarId, RcStrategy, LastUse)> = Vec::new();
    let mut death_events = Vec::new();
    let mut alloc_events = Vec::new();
    let mut metrics = super::metrics::SynergyMetrics::default();

    // Pre-compute: is this block an unwind cleanup block?
    // Unwind blocks end with Resume. Their explicit RcDec instructions
    // must be kept to balance callee-internal RcIncs (e.g., the RcInc
    // added for iterator creation).
    let is_unwind_block = matches!(
        ctx.func.blocks[ctx.blk.index()].terminator,
        crate::ir::ArcTerminator::Resume
    );

    for (instr_idx, instr) in old_body.iter().enumerate() {
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
        }

        // Push the instruction itself.
        new_body.push(instr.clone());

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
        deferred.retain(|&(var, strategy, effective_last)| {
            if effective_last == LastUse::Body(instr_idx)
                && !is_live_at_exit(ctx.state_map, ctx.blk, var)
            {
                new_body.push(ArcInstr::RcDec { var, strategy });
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
        uses_so_far,
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
        let semantics = classify_use_semantics(ctx, var, instr);
        let decision = decide(&DecisionContext {
            site: DecisionSite::Use {
                has_future_use,
                semantics,
            },
            is_rc_managed: true,
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
                });
            }
        }
    }
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
/// Determines whether the use is a normal RC use or a `Project` source
/// (borrowing vs transfer).
///
/// Let aliases (`%dst = %src`) use Normal semantics — the standard
/// `has_future_use` check provides correct `RcInc` placement.
/// `is_ownership_transfer()` handles the Dec side (suppressing last-use
/// Dec for the source at the alias instruction).
fn classify_use_semantics(ctx: &BlockCtx<'_>, var: ArcVarId, instr: &ArcInstr) -> UseSemantics {
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

    UseSemantics::Normal
}
