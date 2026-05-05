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
    is_consuming_primop, is_live_at_exit, is_owned_at_entry, is_ownership_transfer, rc_strategy,
    BlockCtx, LastUse,
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
        return;
    }

    // Skip defined-dead RcDec for ALL borrowed variables.
    if ctx.all_borrowed_defs.contains(&dst) {
        return;
    }
    if ctx.iter_element_defs.contains(&dst) {
        return;
    }
    if ctx.inline_enum_projected_defs.contains(&dst) {
        return;
    }

    let decision = decide(&DecisionContext {
        site: DecisionSite::DefinedDead,
        is_rc_managed: true,
    });

    if decision.rc != RcDecision::None {
        metrics.total_rc_decisions += 1;
    }

    if decision.rc == RcDecision::Dec {
        // PIN-6 (BUG-04-104 §2.6.3): inter-class payload-of suppression. Runs
        // BEFORE PIN-4 + PIN-5 per §2.6.6 Q3 — a positive PIN-6 hit makes
        // the per-class checks below redundant (parent's drop covers
        // regardless of whether the dst is also an apply-alias source).
        if let Some(class_id) = ctx.state_map.ssa_alias_class_of(dst) {
            if pin6_any_ancestor_will_cover(ctx, class_id, instr_idx, classes_dying_here) {
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

        let decision = decide(&DecisionContext {
            site: DecisionSite::LastUse {
                is_consuming_primop: false,
                is_ownership_transfer: false,
                is_owned_call_position: false,
                has_deferred_children,
                reuse: reuse_ctx,
            },
            is_rc_managed: true,
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
                eprintln!(
                    "[PIN6 last_use] fn={:?} blk={} var={:?} class_id={:?} payload_of={:?} classes_dying_here={:?}",
                    ctx.func.name,
                    ctx.blk.index(),
                    var,
                    pin6_class,
                    pin6_class.and_then(|c| ctx.state_map.class_payload_of(c)),
                    classes_dying_here.keys().collect::<Vec<_>>(),
                );
                if let Some(class_id) = pin6_class {
                    if pin6_any_ancestor_will_cover(ctx, class_id, instr_idx, classes_dying_here) {
                        eprintln!("[PIN6 last_use] SUPPRESSED var={var:?}");
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
        return true; // singleton — emit normally
    };

    // PIN-5: same-class dec already emitted at this instruction.
    if emitted_classes_this_instr.contains(&class_id) {
        return false;
    }

    // PIN-4: class still has live members after this instruction.
    if class_alive_after(ctx, class_id, instr_idx, var) {
        return false;
    }

    true
}

/// PIN-4 class-liveness primitive: returns true if any class member OTHER
/// than `var` is live after `instr_idx`. The exclusion of `var` itself is
/// load-bearing — `var` IS dying at this instruction, so its own
/// `is_live_after` would be checked against `last_use == Body(instr_idx)`
/// and could fall through to `is_live_at_exit` which would then return true
/// via stale demand state. Class-liveness is forward-looking from the
/// perspective of OTHER members of the class.
fn class_alive_after(ctx: &BlockCtx<'_>, class_id: u32, instr_idx: usize, var: ArcVarId) -> bool {
    let Some(members) = ctx.state_map.class_members(class_id) else {
        return false;
    };
    members
        .iter()
        .any(|&m| m != var && ctx.is_live_after(instr_idx, m))
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
    instr_idx: usize,
    classes_dying_here: &FxHashMap<u32, FxHashSet<ArcVarId>>,
) -> bool {
    let mut visited: FxHashSet<u32> = FxHashSet::default();
    let mut queue: VecDeque<u32> = VecDeque::new();
    if let Some(parents) = ctx.state_map.class_payload_of(class_id) {
        queue.extend(parents.iter().copied());
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
