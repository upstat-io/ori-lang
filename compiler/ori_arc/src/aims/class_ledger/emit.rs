//! Owed-invariant insertion planning per partition class.
//!
//! Per class: fund every ownership hand-off that duplicates (`BurdenInc`
//! before a CONSUME when the class stays live past it; ALWAYS for a
//! borrowed-rooted class), then place the class's releases (`BurdenDec`) at
//! its death frontier — after the last event in the dying block, or at the
//! front of a dead successor for a per-edge death — under the merge
//! invariant that the owed count agrees on every edge into every merge
//! block. A class whose per-class net dataflow does not converge, disagrees
//! at a merge, or needs an inexpressible release is DECLINED: nothing is
//! planned for it (fail-closed, never a wrong placement).
//! Spec: Annex E §AIMS RL-1 + RL-2 + RL-4 + RL-5.

use rustc_hash::FxHashSet;

use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::aims::verify::burden_delta::compute_burden_entry_nets;
use crate::ir::{ArcFunction, ArcVarId};

use super::events::{
    event_blocks, live_from, live_out, successors_of, ClassEvent, ClassEvents, EventKind,
};

/// One planned burden-op insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlannedOp {
    pub(crate) slot: PlanSlot,
    pub(crate) kind: PlannedOpKind,
    pub(crate) var: ArcVarId,
}

/// The op a plan entry materializes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlannedOpKind {
    /// `BurdenInc { var }` — funds a duplicated reference.
    Inc,
    /// `BurdenDec { var }` — releases the class's owed reference.
    Dec,
}

/// Where an insertion lands inside its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlanSlot {
    /// Front of the block, before every body instruction.
    BlockFront { block: usize },
    /// Immediately before the body instruction at `index`.
    BeforeBody { block: usize, index: usize },
    /// Immediately after the body instruction at `index`.
    AfterBody { block: usize, index: usize },
    /// After every body instruction, before the terminator.
    BeforeTerminator { block: usize },
}

impl PlanSlot {
    /// The block the slot lands in.
    pub(crate) fn block(self) -> usize {
        match self {
            Self::BlockFront { block }
            | Self::BeforeBody { block, .. }
            | Self::AfterBody { block, .. }
            | Self::BeforeTerminator { block } => block,
        }
    }
}

/// Why a class was declined (fail-closed: no ops planned for it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeclineReason {
    /// The per-class net dataflow exhausted its iteration cap.
    NonConverged,
    /// Merge predecessors exit with divergent owed counts.
    MergeDisagree,
    /// A required release has no expressible insertion slot, or the owed
    /// count at a death point is not exactly one.
    UnplaceableRelease,
    /// No member variable resolvable for a planned op.
    UnresolvedOpVar,
}

/// A class's planning outcome.
#[derive(Debug)]
pub(crate) enum ClassOutcome {
    Planned(Vec<PlannedOp>),
    Declined(DeclineReason),
}

/// Plan `BurdenInc`/`BurdenDec` insertions for one class.
pub(crate) fn plan_class(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    events: &ClassEvents,
) -> ClassOutcome {
    let demand_live = live_from(func, &event_blocks(events, true));
    let activity_live = live_from(func, &event_blocks(events, false));
    let mut ops = match plan_incs(func, events, &demand_live) {
        Ok(ops) => ops,
        Err(reason) => return ClassOutcome::Declined(reason),
    };
    let delta = per_block_delta(events, &ops, func.blocks.len());
    let nets = compute_burden_entry_nets(func, preds, &delta);
    if !nets.converged {
        return ClassOutcome::Declined(DeclineReason::NonConverged);
    }
    if !nets.disagree_blocks.is_empty() {
        return ClassOutcome::Declined(DeclineReason::MergeDisagree);
    }
    match plan_releases(
        func,
        events,
        &activity_live,
        &nets.entry_net,
        &delta,
        &mut ops,
    ) {
        Ok(()) => ClassOutcome::Planned(ops),
        Err(reason) => ClassOutcome::Declined(reason),
    }
}

/// `BurdenInc` before every CONSUME that duplicates: the class stays live
/// past the hand-off (a later Read / Mutate / Consume in the stream or a
/// successor), or the class is borrowed-rooted. A consume refunded by a
/// same-site CREDIT (the passthrough return leg) transfers the existing
/// reference and needs no inc on an owned-rooted class.
fn plan_incs(
    func: &ArcFunction,
    events: &ClassEvents,
    demand_live: &[bool],
) -> Result<Vec<PlannedOp>, DeclineReason> {
    let borrowed = events.is_borrowed_rooted();
    let mut ops = Vec::new();
    for (block, evs) in events.per_block.iter().enumerate() {
        for (position, ev) in evs.iter().enumerate() {
            if ev.kind != EventKind::Consume {
                continue;
            }
            if !borrowed
                && (same_site_credit_follows(evs, position)
                    || !(suffix_has_demand(evs, position) || live_out(func, block, demand_live)))
            {
                continue;
            }
            let Some(var) = ev.var else {
                return Err(DeclineReason::UnresolvedOpVar);
            };
            let slot = match ev.site {
                EventSite::Body(index) => PlanSlot::BeforeBody { block, index },
                EventSite::Terminator => PlanSlot::BeforeTerminator { block },
                EventSite::Params => return Err(DeclineReason::UnplaceableRelease),
            };
            ops.push(PlannedOp {
                slot,
                kind: PlannedOpKind::Inc,
                var,
            });
        }
    }
    Ok(ops)
}

/// Whether a same-site CREDIT follows `position` (the transfer-with-refund
/// pair a passthrough call classifies to).
fn same_site_credit_follows(evs: &[ClassEvent], position: usize) -> bool {
    let Some(current) = evs.get(position) else {
        return false;
    };
    evs[position + 1..]
        .iter()
        .any(|ev| ev.kind == EventKind::Credit && ev.site == current.site)
}

/// Whether a later value use (Read / Mutate / Consume) exists in the block.
fn suffix_has_demand(evs: &[ClassEvent], position: usize) -> bool {
    evs[position + 1..].iter().any(|ev| {
        matches!(
            ev.kind,
            EventKind::Read | EventKind::Mutate | EventKind::Consume
        )
    })
}

/// Per-block owed delta: event deltas plus planned ops.
fn per_block_delta(events: &ClassEvents, ops: &[PlannedOp], num_blocks: usize) -> Vec<i64> {
    let mut delta = vec![0i64; num_blocks];
    for (block, evs) in events.per_block.iter().enumerate() {
        delta[block] += evs.iter().map(|ev| ev.delta).sum::<i64>();
    }
    for op in ops {
        let signed = match op.kind {
            PlannedOpKind::Inc => 1,
            PlannedOpKind::Dec => -1,
        };
        delta[op.slot.block()] += signed;
    }
    delta
}

/// Release placement at the class's death frontier.
///
/// - Within-block (RL-2 / RL-5): no live successor and one owed reference —
///   ONE `BurdenDec` after the class's last event in the block (at the block
///   front when the last event is a param birth; at every successor's front
///   when it is the terminator).
/// - Per-edge (RL-4): the class stays live at the block exit but is dead at
///   a successor — the dec is attributed to that edge, materialized at the
///   dead successor's front (sound: the pre-release entry nets agree, so
///   every path into the dead successor carries the same owed count).
///   Same-class jump hand-offs are silent in the event streams, so a
///   threaded reference never reads as an edge death.
fn plan_releases(
    func: &ArcFunction,
    events: &ClassEvents,
    activity_live: &[bool],
    entry_net: &[Option<i64>],
    delta: &[i64],
    ops: &mut Vec<PlannedOp>,
) -> Result<(), DeclineReason> {
    let mut fronts: FxHashSet<usize> = FxHashSet::default();
    for (block, block_entry) in entry_net.iter().enumerate().take(func.blocks.len()) {
        let Some(entry) = block_entry else {
            continue;
        };
        let exit = entry + delta[block];
        if live_out(func, block, activity_live) {
            plan_edge_releases(func, events, activity_live, block, exit, ops, &mut fronts)?;
        } else if exit > 0 && !events.per_block[block].is_empty() {
            plan_block_release(func, events, block, exit, ops, &mut fronts)?;
        }
    }
    Ok(())
}

/// RL-4 per-edge releases for `block`'s dead successors.
fn plan_edge_releases(
    func: &ArcFunction,
    events: &ClassEvents,
    activity_live: &[bool],
    block: usize,
    exit: i64,
    ops: &mut Vec<PlannedOp>,
    fronts: &mut FxHashSet<usize>,
) -> Result<(), DeclineReason> {
    for successor in successors_of(func, block) {
        if activity_live.get(successor).copied().unwrap_or(false) {
            continue;
        }
        if exit == 0 {
            continue;
        }
        if exit != 1 {
            return Err(DeclineReason::UnplaceableRelease);
        }
        push_front_dec(events, block, successor, ops, fronts)?;
    }
    Ok(())
}

/// RL-2 / RL-5 within-block release after the class's last event.
fn plan_block_release(
    func: &ArcFunction,
    events: &ClassEvents,
    block: usize,
    exit: i64,
    ops: &mut Vec<PlannedOp>,
    fronts: &mut FxHashSet<usize>,
) -> Result<(), DeclineReason> {
    if exit != 1 {
        return Err(DeclineReason::UnplaceableRelease);
    }
    let Some(last) = events.per_block[block].last() else {
        return Err(DeclineReason::UnplaceableRelease);
    };
    match last.site {
        EventSite::Body(index) => {
            let var = release_var(events, block)?;
            ops.push(PlannedOp {
                slot: PlanSlot::AfterBody { block, index },
                kind: PlannedOpKind::Dec,
                var,
            });
        }
        EventSite::Params => {
            push_front_dec(events, block, block, ops, fronts)?;
        }
        EventSite::Terminator => {
            let successors = successors_of(func, block);
            if successors.is_empty() {
                return Err(DeclineReason::UnplaceableRelease);
            }
            for successor in successors {
                push_front_dec(events, block, successor, ops, fronts)?;
            }
        }
    }
    Ok(())
}

/// Plan a block-front `BurdenDec`, deduplicated per target block.
fn push_front_dec(
    events: &ClassEvents,
    from_block: usize,
    target: usize,
    ops: &mut Vec<PlannedOp>,
    fronts: &mut FxHashSet<usize>,
) -> Result<(), DeclineReason> {
    if !fronts.insert(target) {
        return Ok(());
    }
    let var = release_var(events, from_block)?;
    ops.push(PlannedOp {
        slot: PlanSlot::BlockFront { block: target },
        kind: PlannedOpKind::Dec,
        var,
    });
    Ok(())
}

/// The member variable a release names: the last resolved event var in the
/// releasing block, else the class's first resolved var anywhere.
fn release_var(events: &ClassEvents, block: usize) -> Result<ArcVarId, DeclineReason> {
    events.per_block[block]
        .iter()
        .rev()
        .find_map(|ev| ev.var)
        .or_else(|| events.per_block.iter().flatten().find_map(|ev| ev.var))
        .ok_or(DeclineReason::UnresolvedOpVar)
}
