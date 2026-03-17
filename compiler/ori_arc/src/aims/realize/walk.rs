//! Unified forward walk for Phase 1 realization.
//!
//! One traversal per block. Routes all RC and reuse decisions through
//! [`decide()`] and collects death/alloc events inline, eliminating the
//! separate scans in `collect_death_events()` and `collect_alloc_events()`.
//!
//! Replaces `emit_rc/forward_walk::emit_body_forward_walk()` with a walk
//! that calls `decide()` per (var, instruction) site and produces both
//! RC operations and reuse event data in a single pass.

use rustc_hash::FxHashMap;

use crate::aims::emit_rc::{
    is_consuming_primop, is_live_at_exit, is_owned_at_entry, is_ownership_transfer, rc_strategy,
    BlockCtx, LastUse,
};
use crate::aims::emit_reuse::{ctor_to_shape, is_reusable_ctor, AllocEvent, DeathEvent};
use crate::aims::lattice::{Cardinality, ShapeClass, SizeClass, Uniqueness};
use crate::ir::{ArcInstr, ArcVarId, RcStrategy, ValueRepr};

use super::decide::{
    decide, DecisionContext, DecisionSite, InstructionDecisions, RcDecision, ReuseContext,
    ReuseDecision, UseSemantics,
};

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
) -> BodyWalkResult {
    let mut uses_so_far: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut deferred: Vec<(ArcVarId, RcStrategy, LastUse)> = Vec::new();
    let mut death_events = Vec::new();
    let mut alloc_events = Vec::new();
    let mut metrics = super::metrics::SynergyMetrics::default();

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
        );

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
) {
    for var in instr.used_vars() {
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

/// Post-instruction: `RcDec` for defined-dead and last-use variables.
///
/// Routes decisions through `decide()` with `DecisionSite::DefinedDead`
/// or `DecisionSite::LastUse`. Collects death events inline when reuse
/// candidacy is detected.
fn emit_post_instr_decs_unified(
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

    // Check managed (not excluded, not scalar, not borrowed).
    if ctx.state_map.is_excluded(dst)
        || ctx.func.var_reprs[dst.index()] == ValueRepr::Scalar
        || ctx.use_info.contains_key(&dst)
        || is_live_at_exit(ctx.state_map, ctx.blk, dst)
    {
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
fn is_rc_managed(ctx: &BlockCtx<'_>, var: ArcVarId) -> bool {
    is_owned_at_entry(
        ctx.state_map,
        ctx.blk,
        var,
        ctx.defined_in_block,
        ctx.borrowed_defs,
        ctx.all_borrowed_defs,
    )
}

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
