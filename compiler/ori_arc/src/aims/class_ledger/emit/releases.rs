//! Release (`BurdenDec`) planning at the class's death frontier: per-edge
//! RL-4 releases for dead successors, within-block RL-2 / RL-5 releases of
//! the iteration's residue, and dead-class per-birth releases.
//! Spec: Annex E §AIMS RL-2 + RL-4 + RL-5.

use rustc_hash::FxHashSet;

use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::ir::{ArcFunction, ArcVarId};

use super::super::events::{live_out, live_out_forward, successors_of, ClassEvent, ClassEvents};
use super::super::replace::{collect_def_points, def_reaches_slot, DefPoint};
use super::cfg_region::CycleRegions;
use super::{DeclineReason, PlanSlot, PlannedOp, PlannedOpKind};
use rustc_hash::FxHashMap as DefMap;

/// RL-2 unused-owned / RL-5 dead-at-entry releases for a class with zero
/// demand events: every acquiring event (positive delta) gets its release
/// immediately after its site.
pub(super) fn plan_dead_class_releases(
    func: &ArcFunction,
    ctx: &ReleaseCtx<'_>,
    events: &ClassEvents,
) -> Result<Vec<PlannedOp>, DeclineReason> {
    let mut ops = Vec::new();
    let mut fronts: FxHashSet<usize> = FxHashSet::default();
    for (block, evs) in events.per_block.iter().enumerate() {
        for ev in evs {
            if ev.delta <= 0 {
                continue;
            }
            match ev.site {
                EventSite::Body(index) => {
                    let var = ev.var.ok_or(DeclineReason::UnresolvedOpVar)?;
                    ops.push(PlannedOp {
                        slot: PlanSlot::AfterBody { block, index },
                        kind: PlannedOpKind::Dec,
                        var,
                    });
                }
                EventSite::BlockEntry => {
                    push_front_dec(ctx, events, block, block, &mut ops, &mut fronts)?;
                }
                EventSite::Terminator => {
                    let successors = successors_of(func, block);
                    if successors.is_empty() {
                        return Err(DeclineReason::UnplaceableRelease);
                    }
                    for successor in successors {
                        push_front_dec(ctx, events, block, successor, &mut ops, &mut fronts)?;
                    }
                }
            }
        }
    }
    Ok(ops)
}

/// Release placement at the class's death frontier, one walk over the
/// pre-release entry nets: per-edge (RL-4) releases for dead successors of
/// live-out blocks; within-block (RL-2 / RL-5) releases after the class's
/// last event where no successor is live.
#[expect(
    clippy::too_many_arguments,
    reason = "release planning consumes the full per-class dataflow context; a param struct would name no domain concept beyond 'everything plan_class computed'"
)]
pub(super) fn plan_releases(
    func: &ArcFunction,
    ctx: &ReleaseCtx<'_>,
    preds: &[Vec<usize>],
    regions: &CycleRegions,
    events: &ClassEvents,
    activity_live: &[bool],
    full_closure: bool,
    entry_net: &[Option<i64>],
    delta: &[i64],
    dom: &crate::graph::DominatorTree,
    ops: &mut Vec<PlannedOp>,
) -> Result<(), DeclineReason> {
    let mut fronts: FxHashSet<usize> = FxHashSet::default();
    // RL-2 block-local: every event of the class in ONE block, netting one
    // owed reference, last event a body instruction — the class is born and
    // last-used there; the release lands right after the last event
    // regardless of cycle-polluted liveness (a back-edge re-entering the
    // block is the NEXT iteration's instance, so the per-iteration balance
    // is exactly what makes the loop header's owed count agree).
    let occupied: Vec<usize> = events
        .per_block
        .iter()
        .enumerate()
        .filter(|(_, evs)| !evs.is_empty())
        .map(|(block, _)| block)
        .collect();
    if let [only_block] = occupied[..] {
        if delta[only_block] == 1 {
            if let Some(ClassEvent {
                site: EventSite::Body(index),
                ..
            }) = events.per_block[only_block].last()
            {
                let slot = PlanSlot::AfterBody {
                    block: only_block,
                    index: *index,
                };
                let var = release_var_for_slot(ctx, events, only_block, slot)?;
                ops.push(PlannedOp {
                    slot,
                    kind: PlannedOpKind::Dec,
                    var,
                });
                return Ok(());
            }
        }
    }
    for (block, block_entry) in entry_net.iter().enumerate().take(func.blocks.len()) {
        let Some(entry) = block_entry else {
            continue;
        };
        let exit = entry + delta[block];
        // Same discriminator as plan_class's liveness vectors: the
        // per-block query mode must match the vector's closure mode.
        let block_live_out = if full_closure {
            live_out(func, block, activity_live)
        } else {
            live_out_forward(func, block, activity_live, dom)
        };
        if block_live_out {
            plan_edge_releases(ctx, events, activity_live, block, exit, ops, &mut fronts)?;
        } else if exit > 0 && !events.per_block[block].is_empty() {
            plan_block_release(
                ctx,
                func,
                preds,
                regions,
                events,
                block,
                *entry,
                dom,
                ops,
                &mut fronts,
            )?;
        }
    }
    Ok(())
}

/// RL-4 per-edge releases for `block`'s dead successors: the class stays
/// live at the block exit but is dead at a successor — the dec is
/// attributed to that edge, materialized at the dead successor's front
/// (a multi-pred dead successor with divergent per-edge counts is caught by
/// the phase-2 merge-agreement gate). Same-class jump hand-offs are silent
/// in the event streams, so a threaded reference never reads as an edge
/// death.
fn plan_edge_releases(
    ctx: &ReleaseCtx<'_>,
    events: &ClassEvents,
    activity_live: &[bool],
    block: usize,
    exit: i64,
    ops: &mut Vec<PlannedOp>,
    fronts: &mut FxHashSet<usize>,
) -> Result<(), DeclineReason> {
    for successor in successors_of(ctx.func, block) {
        if activity_live.get(successor).copied().unwrap_or(false) {
            continue;
        }
        if exit == 0 {
            continue;
        }
        if exit != 1 {
            return Err(DeclineReason::UnplaceableRelease);
        }
        push_front_dec(ctx, events, block, successor, ops, fronts)?;
    }
    Ok(())
}

/// RL-2 / RL-5 within-block release of THIS iteration's residue.
///
/// The residue = entry + pre-terminator deltas + terminator CONSUMES —
/// terminator credits belong to the outgoing edge (the next reference's
/// ledger) and never count toward what dies here. Placement: after the
/// last pre-terminator body event (RL-2 last-use); at the block's own
/// front when the class has no pre-terminator use here (RL-5
/// dead-at-entry); at every successor's front when a terminator-site READ
/// is the last use (the release must follow the terminator).
#[expect(
    clippy::too_many_arguments,
    reason = "block-release placement consumes the full per-class dataflow context; a param struct would name no domain concept beyond 'everything plan_releases holds'"
)]
fn plan_block_release(
    ctx: &ReleaseCtx<'_>,
    func: &ArcFunction,
    preds: &[Vec<usize>],
    regions: &CycleRegions,
    events: &ClassEvents,
    block: usize,
    entry: i64,
    dom: &crate::graph::DominatorTree,
    ops: &mut Vec<PlannedOp>,
    fronts: &mut FxHashSet<usize>,
) -> Result<(), DeclineReason> {
    let evs = &events.per_block[block];
    // Ops already placed in this block (funding incs, seeds) are real
    // pre-terminator deltas: the caller's per-block delta includes them,
    // and the residue must agree or a funded reference reads as released.
    let placed_delta: i64 = ops
        .iter()
        .filter(|op| op.slot.block() == block)
        .map(|op| match op.kind {
            PlannedOpKind::Inc => 1,
            PlannedOpKind::Dec | PlannedOpKind::DecPartial { .. } => -1,
        })
        .sum();
    let pre_delta: i64 = evs
        .iter()
        .filter(|ev| ev.site != EventSite::Terminator)
        .map(|ev| ev.delta)
        .sum::<i64>()
        + placed_delta;
    let terminator_consumes: i64 = evs
        .iter()
        .filter(|ev| ev.site == EventSite::Terminator && ev.delta < 0)
        .map(|ev| ev.delta)
        .sum();
    let residue = entry + pre_delta + terminator_consumes;
    let edge_credits: i64 = evs
        .iter()
        .filter(|ev| ev.site == EventSite::Terminator && ev.delta > 0)
        .map(|ev| ev.delta)
        .sum();
    let forward_credit_target = if edge_credits > 0 {
        // A FORWARD Jump-arg credit hands the reference to the successor's
        // param; the !live_out gate already proved the class has no
        // downstream activity, so the credited reference dies on arrival —
        // release at the single successor's front (a Jump has exactly one
        // successor). A BACK-edge credit funds the next iteration's ledger
        // and falls through to the residue logic.
        match successors_of(func, block)[..] {
            [successor] if !dom.dominates(func.blocks[successor].id, func.blocks[block].id) => {
                Some(successor)
            }
            _ => None,
        }
    } else {
        None
    };
    if let Some(successor) = forward_credit_target {
        if residue + edge_credits != 1 {
            return Err(DeclineReason::UnplaceableRelease);
        }
        if !regions.is_in_cycle(successor) {
            return push_front_dec(ctx, events, block, successor, ops, fronts);
        }
        // The credited reference enters a CYCLE (a loop-threaded class): a
        // header-front dec would fire again on every back-edge re-entry.
        // The reference stays live through the cycle and dies on the exit
        // frontier — a front dec at each single-pred exit block (exactly
        // one exit executes per path).
        for &exit in regions.exit_frontier(successor) {
            if preds.get(exit).is_none_or(|p| p.len() != 1) {
                return Err(DeclineReason::UnplaceableRelease);
            }
            push_front_dec(ctx, events, block, exit, ops, fronts)?;
        }
        return Ok(());
    }
    if residue == 0 {
        return Ok(());
    }
    if residue != 1 {
        return Err(DeclineReason::UnplaceableRelease);
    }
    let terminator_read = evs
        .iter()
        .any(|ev| ev.site == EventSite::Terminator && ev.floor > 0 && ev.delta == 0);
    if terminator_read {
        let successors = successors_of(func, block);
        if successors.is_empty() {
            return Err(DeclineReason::UnplaceableRelease);
        }
        for successor in successors {
            push_front_dec(ctx, events, block, successor, ops, fronts)?;
        }
        return Ok(());
    }
    let last_pre = evs.iter().rev().find(|ev| ev.site != EventSite::Terminator);
    match last_pre.map(|ev| ev.site) {
        Some(EventSite::Body(index)) => {
            let slot = PlanSlot::AfterBody { block, index };
            let var = release_var_for_slot(ctx, events, block, slot)?;
            ops.push(PlannedOp {
                slot,
                kind: PlannedOpKind::Dec,
                var,
            });
            Ok(())
        }
        Some(EventSite::BlockEntry) | None => {
            push_front_dec(ctx, events, block, block, ops, fronts)
        }
        Some(EventSite::Terminator) => unreachable!("filtered to non-terminator sites"),
    }
}

/// Plan a block-front `BurdenDec`, deduplicated per target block. The named
/// variable's definition MUST reach the target's front — a member var from
/// one predecessor arm is undefined on the others (the op-var-placement
/// gate); the chooser walks the class's member vars for a dominating one.
fn push_front_dec(
    ctx: &ReleaseCtx<'_>,
    events: &ClassEvents,
    from_block: usize,
    target: usize,
    ops: &mut Vec<PlannedOp>,
    fronts: &mut FxHashSet<usize>,
) -> Result<(), DeclineReason> {
    if !fronts.insert(target) {
        return Ok(());
    }
    let slot = PlanSlot::BlockFront { block: target };
    let var = release_var_for_slot(ctx, events, from_block, slot)?;
    ops.push(PlannedOp {
        slot,
        kind: PlannedOpKind::Dec,
        var,
    });
    Ok(())
}

/// Dominance context for release-var selection.
pub(super) struct ReleaseCtx<'a> {
    pub(super) func: &'a ArcFunction,
    pub(super) dom: &'a crate::graph::DominatorTree,
    pub(super) defs: DefMap<ArcVarId, DefPoint>,
}

impl<'a> ReleaseCtx<'a> {
    pub(super) fn new(func: &'a ArcFunction, dom: &'a crate::graph::DominatorTree) -> Self {
        Self {
            func,
            dom,
            defs: collect_def_points(func),
        }
    }
}

/// The member variable a release names: the last resolved event var in the
/// releasing block whose definition reaches the slot, else ANY class member
/// var that reaches it (same allocation — every member names the same
/// reference), else `UnresolvedOpVar` (fail-closed).
fn release_var_for_slot(
    ctx: &ReleaseCtx<'_>,
    events: &ClassEvents,
    block: usize,
    slot: PlanSlot,
) -> Result<ArcVarId, DeclineReason> {
    let reaches = |var: ArcVarId| {
        ctx.defs
            .get(&var)
            .is_some_and(|&def| def_reaches_slot(ctx.func, ctx.dom, def, slot))
    };
    events.per_block[block]
        .iter()
        .rev()
        .filter_map(|ev| ev.var)
        .find(|&v| reaches(v))
        .or_else(|| {
            events
                .per_block
                .iter()
                .flatten()
                .filter_map(|ev| ev.var)
                .find(|&v| reaches(v))
        })
        .ok_or(DeclineReason::UnresolvedOpVar)
}
