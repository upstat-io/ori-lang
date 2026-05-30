//! Post-instruction RC dec emission and death event collection.
//!
//! Split from `walk.rs` — handles `RcDec` for defined-dead variables,
//! last-use variables, and collects death events for the reuse planner.
//!
//! BUG-04-104: dec emission consults SSA-alias equivalence classes
//! ([`AimsStateMap::ssa_alias_class_of`]) as a SUPPRESSION FILTER on the
//! existing per-var emission flow:
//!
//! 1. PIN-4 class-liveness skip — when about to emit a `RcDec` on a class
//!    member, skip if any other class member is live AFTER this instruction.
//!    The class is collectively still alive; emission happens at the class's
//!    absolute last use.
//! 2. PIN-5 same-instruction batching — when multiple class members all reach
//!    their per-var "last use" at the same instruction, emit ONE `RcDec`
//!    only. The first member encountered emits via the existing
//!    `apply_last_use_decision` path; subsequent same-class same-instr
//!    members are suppressed.
//!
//! Critically, the class-aware filter NEVER bypasses the existing
//! `apply_last_use_decision` logic — it runs the full `decide()` →
//! return-transfer-suppression → reuse-death-event flow per var, and only
//! suppresses the actual `RcDec` push when the class indicates redundancy.
//! Singletons (vars NOT in a multi-member class) flow through unchanged.

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::emit_rc::{
    is_consuming_primop, is_live_at_exit, is_owned_at_entry, is_ownership_transfer,
    is_take_project, rc_strategy, same_alloc, BlockCtx, LastUse,
};
use crate::aims::emit_reuse::DeathEvent;
use crate::aims::lattice::SizeClass;
use crate::ir::{is_transitive_drop_strategy, ArcInstr, ArcVarId, RcStrategy, ValueRepr};

use super::decide::{
    decide, DecisionContext, DecisionSite, InstructionDecisions, RcDecision, ReuseContext,
    ReuseDecision,
};

/// Post-instruction: `RcDec` for defined-dead and last-use variables.
///
/// Routes decisions through `decide()` with `DecisionSite::DefinedDead`
/// or `DecisionSite::LastUse`. Collects death events inline when reuse
/// candidacy is detected.
///
/// BUG-04-104: per-instruction `emitted_classes_this_instr` set tracks
/// which SSA-alias classes have already received a `RcDec` at this
/// instruction; subsequent same-class dec attempts are suppressed (PIN-5
/// batching). PIN-4 class-liveness skip is consulted before each potential
/// emission.
pub(super) fn emit_post_instr_decs_unified(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
    deferred: &mut Vec<(ArcVarId, RcStrategy, LastUse)>,
    death_events: &mut Vec<DeathEvent>,
    metrics: &mut super::metrics::SynergyMetrics,
) {
    // PIN-5: per-instruction class-id set. After a class emits its first
    // RcDec at this instruction, subsequent same-class same-instr emissions
    // are suppressed.
    let mut emitted_classes_this_instr: FxHashSet<u32> = FxHashSet::default();

    // PIN-6 (BUG-04-104 §2.6.3): pre-collect the set of classes whose canonical
    // dec will fire at this instruction. The same-emission branch of
    // `pin6_any_ancestor_will_cover` queries this map to detect parents whose
    // PIN-5-batched dec covers a child class's RC slot at the SAME instruction.
    // Per the §2.6.3 STRENGTHENED GATE annotation: skipping this pre-collection
    // silently breaks the same_emission branch — the streaming
    // `emitted_classes_this_instr` cannot serve PIN-6 because it tracks
    // AFTER-emit, while PIN-6 needs BEFORE-emit signal.
    let classes_dying_here = collect_classes_dying_here(ctx, instr, instr_idx);

    // DefinedDead: variable defined by this instruction but never used.
    emit_defined_dead(
        ctx,
        instr,
        instr_idx,
        new_body,
        &mut emitted_classes_this_instr,
        &classes_dying_here,
        metrics,
    );

    // Skip last-use decs for consuming PrimOps and ownership transfers.
    // These instructions handle operand RC internally.
    if is_consuming_primop(instr, ctx.func) || is_ownership_transfer(instr, ctx.func, ctx.pool) {
        return;
    }

    // LastUse: variables whose last use is this instruction and dead at exit.
    emit_last_use_decs(
        ctx,
        instr,
        instr_idx,
        new_body,
        deferred,
        death_events,
        &mut emitted_classes_this_instr,
        &classes_dying_here,
        metrics,
    );
}

/// Emit `RcDec` for a defined-but-dead variable.
fn emit_defined_dead(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
    emitted_classes_this_instr: &mut FxHashSet<u32>,
    classes_dying_here: &FxHashMap<u32, FxHashSet<ArcVarId>>,
    metrics: &mut super::metrics::SynergyMetrics,
) {
    let Some(dst) = instr.defined_var() else {
        return;
    };

    // Check managed: not excluded, not scalar, not borrowed, not unused.
    if ctx.state_map.is_excluded(dst)
        || ctx.func.var_reprs[dst.index()] == ValueRepr::Scalar
        || ctx.use_info.contains_key(&dst)
        || is_live_at_exit(ctx.state_map, ctx.blk, dst)
    {
        if ctx.func.var_reprs[dst.index()] != ValueRepr::Scalar {
            tracing::debug!(
                target: "ori_arc::aims::realize::defined_dead",
                func = ?ctx.func.name, var = dst.raw(),
                has_use = ctx.use_info.contains_key(&dst),
                live_exit = is_live_at_exit(ctx.state_map, ctx.blk, dst),
                excluded = ctx.state_map.is_excluded(dst),
                reason = "not-managed-defined-dead",
                "defined-dead RcDec SKIPPED (early)"
            );
        }
        return;
    }

    // Skip defined-dead RcDec for ALL borrowed variables.
    if ctx.all_borrowed_defs.contains(&dst) {
        tracing::debug!(
            target: "ori_arc::aims::realize::defined_dead",
            func = ?ctx.func.name, var = dst.raw(),
            reason = "all-borrowed-def",
            "defined-dead RcDec SUPPRESSED"
        );
        return;
    }
    if ctx.iter_element_defs.contains(&dst) {
        tracing::debug!(
            target: "ori_arc::aims::realize::defined_dead",
            func = ?ctx.func.name, var = dst.raw(),
            reason = "iter-element-def",
            "defined-dead RcDec SUPPRESSED"
        );
        return;
    }
    if ctx.inline_enum_projected_defs.contains(&dst) {
        tracing::debug!(
            target: "ori_arc::aims::realize::defined_dead",
            func = ?ctx.func.name, var = dst.raw(),
            reason = "inline-enum-projected-def",
            "defined-dead RcDec SUPPRESSED"
        );
        return;
    }

    // §04A.3 ITEM-3 — coexistence handshake. Defer to the burden walk
    // when dst's SSA-alias class is fully burden-covered.
    let class_covered = ctx
        .state_map
        .is_class_covered(ctx.state_map.class_id_of(dst));
    let decision = decide(&DecisionContext {
        site: DecisionSite::DefinedDead,
        is_rc_managed: true,
        class_covered,
    });

    if decision.rc != RcDecision::None {
        metrics.total_rc_decisions += 1;
    }

    if decision.rc == RcDecision::Dec {
        // Take-projected payloads (TPR-07-013) transferred ownership OUT of
        // the parent sum type; the parent's scope-exit drop is suppressed
        // (TPR-07-011), so NO ancestor covers this binding's RC slot — it MUST
        // get its own drop. PIN-6 inter-class payload-of suppression assumes
        // the parent's drop covers the projected child, which is false for a
        // take-project (the payload was moved out). Skip PIN-6 for take-
        // projects so the moved-out binding drops at scope exit (RL-2).
        let is_take = is_take_project(instr, ctx.func, ctx.pool);
        // PIN-6 (BUG-04-104 §2.6.3): inter-class payload-of suppression. Runs
        // BEFORE PIN-4 + PIN-5 per §2.6.6 Q3 — a positive PIN-6 hit makes
        // the per-class checks below redundant (parent's drop covers
        // regardless of whether the dst is also an apply-alias source).
        if is_take {
            // No PIN-6 suppression — fall through to class check + emit.
        } else if let Some(class_id) = ctx.state_map.ssa_alias_class_of(dst) {
            if pin6_any_ancestor_will_cover(ctx, class_id, dst, instr_idx, classes_dying_here) {
                return;
            }
        } else if ctx
            .state_map
            .project_alias_sources()
            .get(&dst)
            .is_some_and(|s| !s.is_empty())
        {
            // BUG-04-118 §05 — Project-alias-singleton fallback. Pure
            // Project destinations are PIN-2 singletons: ssa_alias_class_of
            // returns None for them. The existing pin6 entry skips. But
            // their project_alias_sources may surface a transitive-drop
            // ancestor in another class that covers their RC slot at
            // scope exit (the canonical nested-Project-chain double-free
            // shape). Use dst.raw() as the singleton class id; pin6's
            // own internal singleton-guard fires, the project-alias seed
            // walks the source chain, and the BFS finds covering parents.
            if pin6_any_ancestor_will_cover(ctx, dst.raw(), dst, instr_idx, classes_dying_here) {
                return;
            }
        }

        // PIN-4 + PIN-5: class-aware suppression.
        if !class_dec_should_emit(ctx, dst, instr_idx, emitted_classes_this_instr) {
            return;
        }
        if let Some(strategy) = rc_strategy(ctx.func, dst, ctx.pool) {
            new_body.push(ArcInstr::RcDec { var: dst, strategy });
            if let Some(class_id) = ctx.state_map.ssa_alias_class_of(dst) {
                emitted_classes_this_instr.insert(class_id);
            }
        }
    }
}

/// Emit `RcDec` (or defer) for variables at their last use.
///
/// Also collects death events for reuse-candidate variables.
#[expect(clippy::too_many_arguments, reason = "pre-existing")]
fn emit_last_use_decs(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
    deferred: &mut Vec<(ArcVarId, RcStrategy, LastUse)>,
    death_events: &mut Vec<DeathEvent>,
    emitted_classes_this_instr: &mut FxHashSet<u32>,
    classes_dying_here: &FxHashMap<u32, FxHashSet<ArcVarId>>,
    metrics: &mut super::metrics::SynergyMetrics,
) {
    for (pos, var) in instr.used_vars().into_iter().enumerate() {
        if !is_rc_managed(ctx, var) {
            continue;
        }

        if ctx.iter_element_defs.contains(&var) {
            continue;
        }

        if instr.is_owned_position(pos) {
            continue;
        }

        let Some(&(_total, last_use)) = ctx.use_info.get(&var) else {
            continue;
        };

        if last_use != LastUse::Body(instr_idx) || is_live_at_exit(ctx.state_map, ctx.blk, var) {
            continue;
        }

        let has_deferred_children = has_live_borrowed_children(ctx, var, instr_idx);
        let reuse_ctx = build_reuse_context(ctx, var);

        // §04A.3 ITEM-3 — coexistence handshake.
        let class_covered = ctx
            .state_map
            .is_class_covered(ctx.state_map.class_id_of(var));
        let decision = decide(&DecisionContext {
            site: DecisionSite::LastUse {
                is_consuming_primop: false,
                is_ownership_transfer: false,
                is_owned_call_position: false,
                has_deferred_children,
                reuse: reuse_ctx,
            },
            is_rc_managed: true,
            class_covered,
        });

        if decision.rc != RcDecision::None {
            metrics.total_rc_decisions += 1;
        }
        if decision.reuse != ReuseDecision::None {
            metrics.reuse_decisions += 1;
        }

        apply_last_use_decision(
            ctx,
            var,
            &decision,
            last_use,
            instr_idx,
            new_body,
            deferred,
            death_events,
            emitted_classes_this_instr,
            classes_dying_here,
        );
    }
}

/// Apply the decision from `decide()` for a last-use variable.
///
/// BUG-04-104: PIN-4 + PIN-5 class-aware suppression is applied at the
/// emission point — AFTER `should_suppress_return_transfer_dec` clears
/// (return-transfer suppression has its own path-sensitivity that class-
/// aware logic must not undermine), and BEFORE the actual `new_body.push`.
/// Class membership tracked in `emitted_classes_this_instr` for PIN-5
/// dedup; class-liveness consulted via `class_dec_should_emit` for PIN-4.
#[expect(clippy::too_many_arguments, reason = "pre-existing")]
fn apply_last_use_decision(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    decision: &InstructionDecisions,
    last_use: LastUse,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
    deferred: &mut Vec<(ArcVarId, RcStrategy, LastUse)>,
    death_events: &mut Vec<DeathEvent>,
    emitted_classes_this_instr: &mut FxHashSet<u32>,
    classes_dying_here: &FxHashMap<u32, FxHashSet<ArcVarId>>,
) {
    match decision.rc {
        RcDecision::Dec => {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                let is_unwind_block = matches!(
                    ctx.func.blocks[ctx.blk.index()].terminator,
                    crate::ir::ArcTerminator::Resume
                );
                if crate::aims::emit_rc::should_suppress_return_transfer_dec(
                    ctx,
                    var,
                    ctx.blk,
                    is_unwind_block,
                ) {
                    return;
                }

                // PIN-6 (BUG-04-104 §2.6.3): inter-class payload-of suppression.
                // Runs BEFORE PIN-4 + PIN-5 per §2.6.6 Q3 — when class B's
                // transitive drop covers class A's RC slot at-or-after this
                // instr (alive-after parent OR same-emission parent), class A's
                // canonical dec is suppressed to avoid double-free. The pre-
                // collected `classes_dying_here` is the same-emission signal.
                let pin6_class = ctx.state_map.ssa_alias_class_of(var);
                if let Some(class_id) = pin6_class {
                    if pin6_any_ancestor_will_cover(
                        ctx,
                        class_id,
                        var,
                        instr_idx,
                        classes_dying_here,
                    ) {
                        return;
                    }
                } else if ctx
                    .state_map
                    .project_alias_sources()
                    .get(&var)
                    .is_some_and(|s| !s.is_empty())
                {
                    // BUG-04-118 §05 — Project-alias-singleton fallback (see
                    // walk_dec.rs:155+ for full rationale).
                    if pin6_any_ancestor_will_cover(
                        ctx,
                        var.raw(),
                        var,
                        instr_idx,
                        classes_dying_here,
                    ) {
                        return;
                    }
                }

                // PIN-4 + PIN-5: class-aware suppression at the emission point.
                if !class_dec_should_emit(ctx, var, instr_idx, emitted_classes_this_instr) {
                    return;
                }

                new_body.push(ArcInstr::RcDec { var, strategy });
                if let Some(class_id) = ctx.state_map.ssa_alias_class_of(var) {
                    emitted_classes_this_instr.insert(class_id);
                }

                if decision.reuse != ReuseDecision::None {
                    record_death_event(ctx, var, new_body.len() - 1, death_events);
                }
            }
        }
        RcDecision::Defer => {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                let effective = ctx
                    .child_effective_last_use
                    .get(&var)
                    .copied()
                    .unwrap_or(last_use);
                deferred.push((var, strategy, effective));
            }
        }
        _ => {}
    }
}

/// PIN-4 + PIN-5 class-aware emission filter.
///
/// Returns true if the dec for `var` SHOULD be emitted (singleton, OR class
/// is dying here AND no same-class dec already emitted at this instr).
/// Returns false if emission should be suppressed (class still has live
/// members beyond this instr → PIN-4, OR same class already emitted at this
/// instr → PIN-5).
fn class_dec_should_emit(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    instr_idx: usize,
    emitted_classes_this_instr: &FxHashSet<u32>,
) -> bool {
    let Some(class_id) = ctx.state_map.ssa_alias_class_of(var) else {
        tracing::debug!(
            target: "ori_arc::aims::realize::class_dec",
            func = ?ctx.func.name, var = var.raw(),
            decision = "emit", reason = "singleton-no-class",
            "class-dec PIN decision"
        );
        return true; // singleton — emit normally
    };

    // PIN-5: same-class dec already emitted at this instruction.
    if emitted_classes_this_instr.contains(&class_id) {
        tracing::debug!(
            target: "ori_arc::aims::realize::class_dec",
            func = ?ctx.func.name, var = var.raw(), class = class_id,
            decision = "suppress", reason = "pin5-same-instr-class-emitted",
            "class-dec PIN decision"
        );
        return false;
    }

    // PIN-4: class still has live members after this instruction.
    if class_alive_after(ctx, class_id, instr_idx, var) {
        tracing::debug!(
            target: "ori_arc::aims::realize::class_dec",
            func = ?ctx.func.name, var = var.raw(), class = class_id,
            decision = "suppress", reason = "pin4-class-alive-after",
            "class-dec PIN decision"
        );
        return false;
    }

    tracing::debug!(
        target: "ori_arc::aims::realize::class_dec",
        func = ?ctx.func.name, var = var.raw(), class = class_id,
        members = ?ctx.state_map.class_members(class_id),
        decision = "emit", reason = "no-suppression",
        "class-dec PIN decision"
    );
    true
}

/// PIN-4 class-liveness primitive: returns true if a DIFFERENT class member
/// is the canonical-rep emitter for this class per the SSOT
/// `pin4_class_emits_dec_set` (BUG-04-111 §05 Step 4.5).
///
/// Pre-fix logic checked syntactic `is_live_after` — but a borrowed live use
/// emits no dec, so the suppression fired even when no class member would
/// actually emit, leaving the dec unbalanced (BUG-04-111 leak shape). The
/// canonical-rep query identifies the LATEST-emitting member per gemini
/// Round 2 F1; non-canonical members suppress.
///
/// When `canonical_rep_for` returns `None` (no class member is in
/// `pin4_class_emits_dec_set` for this block), no class member will emit a
/// dec here, so PIN-4 suppression has no canonical to defer to — return
/// false. Per AIMS §3 TF-11 transparent-alias semantics, a class member
/// being "syntactically alive after" via SSA Let-alias chain does NOT
/// constitute an RC-obligation that should suppress the canonical emitter's
/// dec — Let aliases are RC-inert per the AIMS lattice. The previous
/// syntactic-`is_live_after` fallback was over-firing on this case, leaving
/// the dec unbalanced. See `bug-tracker/plans/BUG-04-111/section-05-implementation.md`
/// HISTORY block for empirical fix delta + remaining-failure classification.
fn class_alive_after(ctx: &BlockCtx<'_>, class_id: u32, instr_idx: usize, var: ArcVarId) -> bool {
    // First: existing BUG-04-111 §05 Step 4.5 PIN-4 canonical-rep logic.
    // Preserves the generics::* WIN delta — when pin4_class_emits_dec_set
    // identifies a canonical emitter for this class, defer to it: suppress
    // when var != canonical, fire when var == canonical.
    // BUG-04-123 cluster fix: consult the FUNCTION-LEVEL canonical-rep so a
    // multi-use class spanning blocks elects one canonical emitter across ALL
    // blocks (vs the prior per-block `pin4_class_emits_dec_set`).
    if let Some(suppressed) =
        crate::aims::emit_rc::dead_cleanup::emission_site::class_member_suppresses(
            class_id,
            var,
            ctx.blk.index(),
            ctx.post_doms,
            ctx.global_pin4_emits,
            ctx.inc_counts,
        )
    {
        return suppressed;
    }

    // BUG-04-118 §05 Round 2 /tp-help (Option A refined): when pin4
    // returns no canonical, consult the typed class_dec_obligations
    // side-table for same-class dedup. This is the case BUG-04-111 §05
    // Step 4.5's fix returns false on (preserving its narrow correctness)
    // but where multi-member SSA alias classes from Let{Var} / Jump arg /
    // Conditional chains DO have RC obligations that need scheduling.
    //
    // Returns true (suppress) when this class has an intra-block
    // obligation point LATER than instr_idx — the later member's dec
    // fires the canonical class dec; this var's earlier emission is
    // redundant and would corrupt the shared RC slot.
    //
    // NOT consulting block_exit_members — too aggressive; the existing
    // emit machinery coordinates cross-block decs via pin4_class_emits_
    // dec_set + canonical-rep selection.
    //
    // Filter: skip classes formed via Direct apply-result-alias union
    // (apply_result_aliases entry exists for some class member with
    // `ApplyAliasSource::Direct`) OR classes containing a function
    // parameter member. Both are transparent-alias-only classes per
    // AIMS §3 TF-11 — Let{Var} aliases of an owned param/apply-result
    // are RC-inert; the realize walk emits no actual RC-dec at their
    // syntactic last-use positions. The existing pin4-based emit
    // machinery handles their dec scheduling correctly via canonical-
    // rep selection over actual emission sites; obligation-table
    // consultation over-suppresses by treating syntactic last-uses
    // (borrow positions) as if they were real RC-dec obligations.
    //
    // Param-member case (BUG-04-118 §05 Round 2 refinement):
    // `borrow_list_int_project_consumer_then_return_no_leak` callee
    // forms class {%0(param), %1, %6, %8, %14} via pure Let{Var}
    // chains of param %0. None of these vars are Apply destinations,
    // so the Direct-apply-alias check returns false; without the
    // param-member check, intra-block obligations [(%1, instr_3),
    // (%8, terminator)] (both borrow-position last-uses) would
    // suppress the legitimate canonical dec emission, leaking 2 RCs
    // back to the caller via Return %14.
    //
    // Direct-apply-alias case (BUG-04-118 §05 Round 2 original):
    // `apply_result_aliases[%6] = Direct(%5)` triggers `uf.union(%5, %6)`
    // forming class {%3, %5, %6, ...}. Same RC-inert semantics —
    // existing return-transfer + pin4 handle dec scheduling.
    //
    // The closure-bridge case (Let{Var} chains of PartialApply result)
    // has no apply_result_aliases entry on the closure value itself
    // — only on the indirect-call DESTINATIONS via the `Wrapped`
    // install — and those destinations live in DIFFERENT classes
    // (Wrapped does not union). The closure-value class also has no
    // param member (closures are body-defined values). So the
    // closure-value class still passes both filters.
    if class_uses_direct_apply_alias_union(ctx, class_id)
        || class_contains_param_member(ctx, class_id)
    {
        return false;
    }

    let obligations = ctx.state_map.class_dec_obligations();
    if let Some(entry) = obligations.get(&(ctx.blk, class_id)) {
        // The later obligation only covers THIS var's dec when it names the
        // SAME runtime allocation. A phi-merged class unions distinct
        // allocations (e.g. loop-carried `s` old-value alloc A with the
        // concat-result alloc B via the back-edge Jump-arg → block-param
        // edge); alloc B's later obligation must NOT suppress alloc A's dec,
        // or alloc A leaks every iteration (RL-4 P1 + §10 under-elim
        // per-path-net-0). `same_alloc` excludes the phi edge.
        let later_obligation_exists =
            entry
                .intra_block_obligations
                .iter()
                .any(|&(other_var, other_idx)| {
                    other_var != var
                        && other_idx > instr_idx
                        && same_alloc(ctx.same_alloc_reps, other_var, var)
                });
        if later_obligation_exists {
            return true;
        }
    }

    // No pin4 canonical AND no later obligation → no PIN-4 suppression
    // (BUG-04-111 §05 Step 4.5 fallback — Let aliases RC-inert per AIMS
    // lattice when no class member actually emits).
    false
}

/// BUG-04-118 §05 Round 2 obligation-table filter: detect classes formed
/// via `Direct` apply-result-alias union (caller-arg unioned with call
/// result). Returns true when ANY class member has an
/// `apply_result_aliases` entry with `ApplyAliasSource::Direct(_)` shape.
///
/// These classes are scheduled correctly by the existing pin4-based
/// emit machinery; obligation-table consultation on them over-suppresses
/// decs and causes leaks (regressed
/// `borrow_list_int_project_consumer_then_return_no_leak`).
fn class_uses_direct_apply_alias_union(ctx: &BlockCtx<'_>, class_id: u32) -> bool {
    use crate::aims::intraprocedural::state_map::ApplyAliasSource;
    let Some(members) = ctx.state_map.class_members(class_id) else {
        return false;
    };
    let aliases = ctx.state_map.apply_result_aliases();
    members
        .iter()
        .any(|m| matches!(aliases.get(m), Some(ApplyAliasSource::Direct(_))))
}

/// BUG-04-118 §05 Round 2 obligation-table filter: detect classes
/// containing a function parameter member. Such classes are formed by
/// Let{Var} aliases of the param, transitive-alias chains, or both —
/// per AIMS §3 TF-11 these are transparent-alias semantics, RC-inert.
///
/// Function-parameter classes have specific dec scheduling: the param's
/// initial RC arrives from the caller (transferred via Owned ABI), and
/// is re-transferred via Return when the function returns the same slot.
/// The realize walk emits no body-walk decs for these param-aliased
/// vars — `should_suppress_return_transfer_dec` and `is_owned_at_entry`
/// gate body-walk emissions correctly.
///
/// The obligation-table records syntactic last-uses of Let{Var} aliases
/// (e.g., `%1` at `Apply __index(%1 [borrow], ...)`) as if they were
/// real RC-dec obligations. Borrow-position last-uses do NOT emit
/// decs (per `emit_last_use_decs`'s `is_owned_position` skip), so
/// these obligations are phantom — consulting the table over-suppresses
/// the canonical pin4-based emission elsewhere in the chain.
///
/// Regressed `borrow_list_int_project_consumer_then_return_no_leak` at
/// the callee `@borrow_project_then_return(xs: [int]) -> [int]` — class
/// {%0, %1, %6, %8, %14} contains param %0; obligations [(%1, `idx_3`),
/// (%8, terminator)] are both borrow-position phantoms. The pin4-based
/// emission machinery (BUG-04-111 §05 Step 4.5 SSOT) correctly handles
/// these classes without obligation-table input.
fn class_contains_param_member(ctx: &BlockCtx<'_>, class_id: u32) -> bool {
    let Some(members) = ctx.state_map.class_members(class_id) else {
        return false;
    };
    let param_vars: FxHashSet<ArcVarId> = ctx.func.params.iter().map(|p| p.var).collect();
    members.iter().any(|m| param_vars.contains(m))
}

/// Record a death event for the reuse planner.
fn record_death_event(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    instr_idx: usize,
    death_events: &mut Vec<DeathEvent>,
) {
    let state = ctx.state_map.var_state_at_block_exit(ctx.blk, var);
    let shape = ctx.state_map.var_shape(var);
    let ty = ctx.func.var_types[var.index()];

    death_events.push(DeathEvent {
        var,
        block: ctx.blk,
        instr_idx,
        uniqueness: ctx
            .state_map
            .effective_uniqueness_at_block_exit(ctx.blk, var),
        cardinality: state.cardinality,
        ty,
        shape,
        size_class: SizeClass::UNKNOWN,
    });
}

// Helpers

/// Whether a variable is RC-managed: owned (per state map) and non-scalar.
#[inline]
pub(super) fn is_rc_managed(ctx: &BlockCtx<'_>, var: ArcVarId) -> bool {
    is_owned_at_entry(
        ctx.state_map,
        ctx.blk,
        var,
        ctx.defined_in_block,
        ctx.borrowed_defs,
        ctx.all_borrowed_defs,
    )
}

/// Whether a parent aggregate variable has borrowed children with later uses.
fn has_live_borrowed_children(ctx: &BlockCtx<'_>, var: ArcVarId, instr_idx: usize) -> bool {
    if let Some(&child_last) = ctx.child_effective_last_use.get(&var) {
        match child_last {
            LastUse::Body(c) => c > instr_idx,
            LastUse::Terminator => true,
        }
    } else {
        false
    }
}

/// Build the reuse context for a dying variable from the state map.
fn build_reuse_context(ctx: &BlockCtx<'_>, var: ArcVarId) -> ReuseContext {
    let state = ctx.state_map.var_state_at_block_exit(ctx.blk, var);
    let shape = ctx.state_map.var_shape(var);

    ReuseContext {
        shape,
        uniqueness: ctx
            .state_map
            .effective_uniqueness_at_block_exit(ctx.blk, var),
        cardinality: state.cardinality,
    }
}

/// Pre-collect the set of classes whose canonical dec will fire at this
/// instruction (BUG-04-104 §2.6.3 STRENGTHENED GATE).
///
/// Per §2.6.3, the same-emission branch of `pin6_any_ancestor_will_cover`
/// queries this map to detect parent classes whose PIN-5-batched dec covers
/// a child class's RC slot at the SAME instruction. The streaming
/// `emitted_classes_this_instr` cannot serve PIN-6's needs because it tracks
/// AFTER-emit; PIN-6 needs BEFORE-emit signal — the SET of classes about to
/// die at `instr_idx`, populated BEFORE the per-var emission loop's first
/// iteration.
///
/// Walks `instr.used_vars()` ∪ {`instr.defined_var()`} per §2.6.3, filters to
/// classes whose absolute last-use is THIS instruction (no class member alive
/// after `instr_idx` per PIN-4 class-liveness), and collects var members per
/// class id.
pub(super) fn collect_classes_dying_here(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
) -> FxHashMap<u32, FxHashSet<ArcVarId>> {
    let mut result: FxHashMap<u32, FxHashSet<ArcVarId>> = FxHashMap::default();
    let mut consider = |var: ArcVarId| {
        let Some(class_id) = ctx.state_map.ssa_alias_class_of(var) else {
            return;
        };
        let Some(members) = ctx.state_map.class_members(class_id) else {
            return;
        };
        // Class's absolute last-use is at THIS instr iff no class member is
        // live after instr_idx. The PIN-4 class-liveness primitive already
        // implements this check; reuse it here.
        let class_alive_after_here = members.iter().any(|&m| ctx.is_live_after(instr_idx, m));
        if !class_alive_after_here {
            result.entry(class_id).or_default().insert(var);
        }
    };
    for var in instr.used_vars() {
        consider(var);
    }
    if let Some(dst) = instr.defined_var() {
        consider(dst);
    }
    result
}

/// PIN-6 (BUG-04-104 §2.6.3) inter-class payload-of suppression predicate.
///
/// Walks the transitive `class_payload_of` ancestor chain via BFS from
/// `class_id` (sketch v2 per Plan TPR Round 19 — handles singleton parents,
/// same-emission parent drops via PIN-5 batching, nested transitive-drop
/// chains, AND cross-block coverage via `is_live_after` fallthrough to
/// `is_live_at_exit`). Returns `true` when ANY ancestor class B is:
/// (a) transitive-drop strategy per [`is_transitive_drop_strategy`], AND
/// (b) "alive after `instr_idx`" (some member of B is live after) OR
///     "dying at the same instruction" (B is in `classes_dying_here`).
///
/// A `true` return means class B's drop will dec class A's RC slot
/// transitively — class A's canonical dec at `instr_idx` is REDUNDANT and must
/// be suppressed to avoid double-free.
///
/// Cycle prevention via `visited` set; defensive grandparent walk on
/// `class_members == None` covers the case where the singleton-class
/// population invariant from §2.6.2 was violated (no false-suppression
/// guarantee — chain walk continues to a covering grandparent).
pub(super) fn pin6_any_ancestor_will_cover(
    ctx: &BlockCtx<'_>,
    class_id: u32,
    var: ArcVarId,
    instr_idx: usize,
    classes_dying_here: &FxHashMap<u32, FxHashSet<ArcVarId>>,
) -> bool {
    let mut visited: FxHashSet<u32> = FxHashSet::default();
    let mut queue: VecDeque<u32> = VecDeque::new();
    let existing_parents = ctx.state_map.class_payload_of(class_id);
    if let Some(parents) = existing_parents {
        queue.extend(parents.iter().copied());
    }
    // BUG-04-118 §05 — Project-alias seed (codex Q2 architectural cure for
    // nested Project chain RED shapes). Narrowed trigger: only fire when
    // class_payload_of(class_id) is empty — Direct apply-alias union shapes
    // populate class_payload_of and are handled by the existing pin4 +
    // obligation-table machinery; additional seeds there over-suppress.
    // Project destinations like `%value = Project %inner.field` are the
    // intended target: they have NO class_payload_of edges (PIN-2: Project
    // doesn't union classes), but DO have project_alias_sources entries
    // pointing at the original Construct chain via Rule 6 nested-Project
    // transitivity (`project_aliases.rs` Step 2). Walking those sources
    // surfaces the transitive-drop ancestor that covers `var`'s RC slot.
    //
    // Soundness: a Project chain physically aliases the source's storage
    // — when the source's Construct chain has a transitive-drop ancestor
    // that covers its slot (alive_after OR same-emission), the same
    // coverage applies to the projected destination because the bytes are
    // physically shared. PIN-2 stays preserved (no class merge); we only
    // seed the BFS with additional candidate ancestors at query time.
    //
    // Use `var` directly (not class_members iteration): `class_value` may
    // not have a class_members entry at all because ensure_singleton_class
    // is called only for classes appearing in class_payload_of edges
    // (population-side at record_payload_edge in post_convergence.rs).
    // Pure-Project-destination singletons never appear there. Looking up
    // project_alias_sources by `var` directly bypasses that gap.
    if existing_parents.is_none() {
        // Singleton guard — exclude Direct apply-alias union shapes (multi-
        // member classes) where existing pin4 + obligation-table machinery
        // owns the dec; firing the project-alias seed there over-suppresses.
        // Two singleton signals: class_members entry with exactly one member
        // (post-convergence-materialized), OR class_id == var.raw() (pure
        // Project destination never put through ensure_singleton_class).
        let is_singleton = match ctx.state_map.class_members(class_id) {
            Some(members) => members.len() == 1,
            None => class_id == var.raw(),
        };
        if is_singleton {
            if let Some(sources) = ctx.state_map.project_alias_sources().get(&var) {
                for &source_var in sources {
                    if let Some(source_class) = ctx.state_map.ssa_alias_class_of(source_var) {
                        queue.push_back(source_class);
                        if let Some(source_parents) = ctx.state_map.class_payload_of(source_class) {
                            queue.extend(source_parents.iter().copied());
                        }
                    }
                }
            }
        }
    }
    while let Some(parent_class) = queue.pop_front() {
        if !visited.insert(parent_class) {
            continue;
        }

        // Resolve parent's representative var (PIN-6 singleton invariant per
        // §2.6.2: population MUST ensure class_members is populated for every
        // class with a class_payload_of entry — including singletons).
        let Some(parent_members) = ctx.state_map.class_members(parent_class) else {
            // Singleton invariant violated — defensively continue chain walk
            // to grandparents. No false suppressions: cannot resolve THIS
            // parent's strategy, so don't suppress through it.
            if let Some(gps) = ctx.state_map.class_payload_of(parent_class) {
                queue.extend(gps.iter().copied());
            }
            continue;
        };
        let Some(&parent_rep) = parent_members.iter().next() else {
            if let Some(gps) = ctx.state_map.class_payload_of(parent_class) {
                queue.extend(gps.iter().copied());
            }
            continue;
        };
        let Some(parent_strategy) = rc_strategy(ctx.func, parent_rep, ctx.pool) else {
            // Scalar parent — no RC, no transitive drop.
            continue;
        };

        if is_transitive_drop_strategy(parent_strategy) {
            // Will the parent emit its canonical dec at-or-after instr_idx?
            let alive_after = parent_members
                .iter()
                .any(|&v| ctx.is_live_after(instr_idx, v));
            // R19 Codex F2 — same-emission parent (PIN-5 batching at this instr).
            // class_dec_should_emit will fire parent's canonical dec AT THIS
            // instruction since parent_class is in classes_dying_here. The dec
            // walks parent's transitive payload (including class A's slot),
            // covering A's dec.
            let same_emission = classes_dying_here.contains_key(&parent_class);
            if alive_after || same_emission {
                return true; // PIN-6 hit — parent's drop covers A.
            }
            // Parent has transitive-drop strategy BUT is dead AND not
            // same-emission. Parent's canonical dec already fired in a prior
            // instruction — it walked class A's slot at that point. A's
            // canonical dec at instr_idx would double-free; this is a
            // population-time bug (class A → parent_class should not have
            // been recorded if A's lifetime extends beyond parent's).
            // Defensively continue the chain walk in case some grandparent
            // still covers.
        }
        // R19 Codex F3 — non-transitive-drop parent OR transitive-drop-but-
        // dead parent: walk further up the chain. Nested case: A → B → C
        // where B is non-transitive (e.g., FatPointer) but C is transitive
        // (InlineEnum) — C still covers A through B's chain.
        if let Some(gps) = ctx.state_map.class_payload_of(parent_class) {
            queue.extend(gps.iter().copied());
        }
    }
    false
}
