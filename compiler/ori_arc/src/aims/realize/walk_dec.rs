//! Post-instruction RC dec emission and death event collection.
//!
//! Split from `walk.rs` — handles `RcDec` for defined-dead variables,
//! last-use variables, and collects death events for the reuse planner.

use crate::aims::emit_rc::{
    is_consuming_primop, is_live_at_exit, is_owned_at_entry, is_ownership_transfer, rc_strategy,
    BlockCtx, LastUse,
};
use crate::aims::emit_reuse::DeathEvent;
use crate::aims::lattice::{Cardinality, ShapeClass, SizeClass, Uniqueness};
use crate::ir::{ArcInstr, ArcVarId, RcStrategy, ValueRepr};

use super::decide::{
    decide, DecisionContext, DecisionSite, InstructionDecisions, RcDecision, ReuseContext,
    ReuseDecision,
};

/// Post-instruction: `RcDec` for defined-dead and last-use variables.
///
/// Routes decisions through `decide()` with `DecisionSite::DefinedDead`
/// or `DecisionSite::LastUse`. Collects death events inline when reuse
/// candidacy is detected.
pub(super) fn emit_post_instr_decs_unified(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
    deferred: &mut Vec<(ArcVarId, RcStrategy, LastUse)>,
    death_events: &mut Vec<DeathEvent>,
    metrics: &mut super::metrics::SynergyMetrics,
) {
    // DefinedDead: variable defined by this instruction but never used.
    emit_defined_dead(ctx, instr, new_body, metrics);

    // Skip last-use decs for consuming PrimOps and ownership transfers.
    // These instructions handle operand RC internally.
    if is_consuming_primop(instr, ctx.func) || is_ownership_transfer(instr, ctx.func) {
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
        metrics,
    );
}

/// Emit `RcDec` for a defined-but-dead variable.
fn emit_defined_dead(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    new_body: &mut Vec<ArcInstr>,
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

    // Skip defined-dead RcDec for parameter-borrowed variables.
    // These are Let aliases of borrowed function parameters (e.g., the
    // __for_coll phantom in the for-loop exit block). The caller handles
    // their cleanup via own→borrow reconciliation.
    //
    // Project-borrowed variables that come from iterator element extraction
    // (__iter_next) also skip — their parent collection's elem_dec_fn
    // handles cleanup. Other project-borrowed vars (e.g., struct field
    // access, tuple destructuring) DO need RcDec as their source aggregate
    // may not handle per-field cleanup.
    if ctx.all_borrowed_defs.contains(&dst) && !ctx.project_borrowed_defs.contains(&dst) {
        return;
    }
    // Iterator-element projections: skip RcDec for elements extracted
    // from __iter_next results. These are borrowed from the collection
    // buffer, and elem_dec_fn handles cleanup when the collection is freed.
    if ctx.iter_element_defs.contains(&dst) {
        return;
    }
    // InlineEnum projections: skip RcDec for values projected from
    // Option/Result/Enum sources. The parent's InlineEnum RcDec handles
    // per-field cleanup via a tag-switch with per-variant RC ops.
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
        if let Some(strategy) = rc_strategy(ctx.func, dst, ctx.pool) {
            new_body.push(ArcInstr::RcDec { var: dst, strategy });
        }
    }
}

/// Emit `RcDec` (or defer) for variables at their last use.
///
/// Also collects death events for reuse-candidate variables.
fn emit_last_use_decs(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
    deferred: &mut Vec<(ArcVarId, RcStrategy, LastUse)>,
    death_events: &mut Vec<DeathEvent>,
    metrics: &mut super::metrics::SynergyMetrics,
) {
    for (pos, var) in instr.used_vars().into_iter().enumerate() {
        if !is_rc_managed(ctx, var) {
            continue;
        }

        // Iterator-element defs and __iter_next type markers: these are
        // borrowed from the collection buffer. Skip RcDec — elem_dec_fn
        // handles cleanup when the collection is freed.
        if ctx.iter_element_defs.contains(&var) {
            continue;
        }

        // Callee takes ownership at this position — no caller-side Dec.
        if instr.is_owned_position(pos) {
            continue;
        }

        let Some(&(_total, last_use)) = ctx.use_info.get(&var) else {
            continue;
        };

        if last_use != LastUse::Body(instr_idx) || is_live_at_exit(ctx.state_map, ctx.blk, var) {
            continue;
        }

        // Check for deferred children (parent with live borrowed children).
        let has_deferred_children = has_live_borrowed_children(ctx, var, instr_idx);

        // Build reuse context from state map (single query per death site).
        let reuse_ctx = build_reuse_context(ctx, var);

        // Snapshot reuse context for metrics before moving into DecisionContext.
        let is_cross_dim_reuse_candidate = reuse_ctx.uniqueness == Uniqueness::MaybeShared
            && reuse_ctx.cardinality == Cardinality::Once
            && matches!(reuse_ctx.shape, ShapeClass::ReusableCtor(_));

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

        // Synergy metrics: count RC decisions and multi-dim reuse.
        if decision.rc != RcDecision::None {
            metrics.total_rc_decisions += 1;
        }
        if decision.reuse != ReuseDecision::None {
            metrics.reuse_decisions += 1;
            if decision.reuse == ReuseDecision::StaticReuse && is_cross_dim_reuse_candidate {
                metrics.cross_dim_reuse += 1;
            }
        }

        apply_last_use_decision(
            ctx,
            var,
            &decision,
            last_use,
            new_body,
            deferred,
            death_events,
        );
    }
}

/// Apply the decision from `decide()` for a last-use variable.
fn apply_last_use_decision(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    decision: &InstructionDecisions,
    last_use: LastUse,
    new_body: &mut Vec<ArcInstr>,
    deferred: &mut Vec<(ArcVarId, RcStrategy, LastUse)>,
    death_events: &mut Vec<DeathEvent>,
) {
    match decision.rc {
        RcDecision::Dec => {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcDec { var, strategy });

                // Cross-decision interaction: if decide() identified reuse
                // potential, record the death event for the reuse planner.
                // The RcDec will be removed later by the reuse emission if
                // a matching Construct is found.
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

/// Record a death event for the reuse planner.
///
/// Queries the state map for uniqueness, cardinality, and shape — these
/// are the same queries that `collect_death_events()` makes, but done
/// inline during the forward walk instead of in a separate scan.
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
        uniqueness: state.uniqueness,
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
///
/// If so, the parent's `RcDec` must be deferred until all children are dead.
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
///
/// Queries shape (per-variable), uniqueness and cardinality (block exit state).
fn build_reuse_context(ctx: &BlockCtx<'_>, var: ArcVarId) -> ReuseContext {
    let state = ctx.state_map.var_state_at_block_exit(ctx.blk, var);
    let shape = ctx.state_map.var_shape(var);

    ReuseContext {
        shape,
        uniqueness: state.uniqueness,
        cardinality: state.cardinality,
    }
}
