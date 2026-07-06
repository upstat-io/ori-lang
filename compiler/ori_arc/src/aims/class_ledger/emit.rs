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
    event_blocks, live_from, live_from_forward, live_out, live_out_forward, successors_of,
    ClassEvent, ClassEvents, EventKind,
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
///
/// Releases plan on the PRE-release entry nets; the merge-agreement gate
/// runs on the COMPLETED op set — a branch-exclusive class disagrees at its
/// merge exactly until the dead-arm RL-4 dec lands, so gating before the
/// releases would decline a plannable class. A planning error DEFERS to the
/// final gate: when the completed nets still disagree, `MergeDisagree` is the
/// dominant diagnosis (a cyclic per-iteration imbalance reports the merge
/// disagreement, not its downstream unplaceable-release symptom). The
/// per-class verify walk re-checks the completed plan independently
/// (floors + terminal nets), so a mis-planned release can never read Clean.
pub(crate) fn plan_class(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    events: &ClassEvents,
    seed_ops: &[PlannedOp],
) -> ClassOutcome {
    if seed_ops.is_empty() && births_only(events) {
        // RL-2 unused-owned / RL-5 dead-at-entry: a class whose only events
        // are births (no demand, no credits — a credit is itself a live use
        // of its subject) releases each owed birth immediately after its
        // site — the general death-frontier walk cannot place these (a
        // birth inside a cycle keeps every loop block activity-live, so no
        // block ever reads as the frontier).
        return match plan_dead_class_releases(func, events) {
            Ok(ops) => ClassOutcome::Planned(ops),
            Err(reason) => ClassOutcome::Declined(reason),
        };
    }
    // Funding (inc) decisions use FORWARD-only demand liveness: a back-edge
    // suffix is the next iteration's events, never continued use of the
    // current reference. Release placement keeps the full closure.
    let dom = crate::graph::DominatorTree::build(func);
    let demand_live = live_from_forward(func, &event_blocks(events, true), &dom);
    let activity_live = live_from(func, &event_blocks(events, false));
    let mut ops = seed_ops.to_vec();
    match plan_incs(func, events, &demand_live, &dom) {
        Ok(incs) => ops.extend(incs),
        Err(reason) => return ClassOutcome::Declined(reason),
    }
    let delta = per_block_delta(events, &ops, func.blocks.len());
    let nets = compute_burden_entry_nets(func, preds, &delta);
    if !nets.converged {
        return ClassOutcome::Declined(DeclineReason::NonConverged);
    }
    let plan_error = plan_releases(
        func,
        events,
        &activity_live,
        &nets.entry_net,
        &delta,
        &mut ops,
    )
    .err();
    let final_delta = per_block_delta(events, &ops, func.blocks.len());
    let final_nets = compute_burden_entry_nets(func, preds, &final_delta);
    if !final_nets.converged {
        return ClassOutcome::Declined(DeclineReason::NonConverged);
    }
    if !final_nets.disagree_blocks.is_empty() {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            gate = "plan-merge-disagree",
            disagree = ?final_nets.disagree_blocks,
            delta = ?final_delta,
            planned = ?ops,
            events = ?events.per_block,
            "class declined: completed-plan owed counts disagree at a merge"
        );
        return ClassOutcome::Declined(DeclineReason::MergeDisagree);
    }
    if let Some(reason) = plan_error {
        return ClassOutcome::Declined(reason);
    }
    ClassOutcome::Planned(ops)
}

/// Whether the class's events are exclusively births — no demand (Read /
/// Mutate / Consume) and no credits (a placed inc reads its subject, so a
/// credit-bearing class is never dead-on-creation).
fn births_only(events: &ClassEvents) -> bool {
    events
        .per_block
        .iter()
        .flatten()
        .all(|ev| ev.kind == EventKind::Birth)
}

/// RL-2 unused-owned / RL-5 dead-at-entry releases for a class with zero
/// demand events: every acquiring event (positive delta) gets its release
/// immediately after its site — after the acquiring body instruction, at
/// the block front for a block-entry acquisition, or at every successor's
/// front for a terminator-site acquisition (a cross-class jump credit lands
/// in the target block). Zero-delta events (an externally-funded birth) owe
/// nothing.
fn plan_dead_class_releases(
    func: &ArcFunction,
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
                    push_front_dec(events, block, block, &mut ops, &mut fronts)?;
                }
                EventSite::Terminator => {
                    let successors = successors_of(func, block);
                    if successors.is_empty() {
                        return Err(DeclineReason::UnplaceableRelease);
                    }
                    for successor in successors {
                        push_front_dec(events, block, successor, &mut ops, &mut fronts)?;
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
fn plan_releases(
    func: &ArcFunction,
    events: &ClassEvents,
    activity_live: &[bool],
    entry_net: &[Option<i64>],
    delta: &[i64],
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
                let var = release_var(events, only_block)?;
                ops.push(PlannedOp {
                    slot: PlanSlot::AfterBody {
                        block: only_block,
                        index: *index,
                    },
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
        if live_out(func, block, activity_live) {
            plan_edge_releases(func, events, activity_live, block, exit, ops, &mut fronts)?;
        } else if exit > 0 && !events.per_block[block].is_empty() {
            plan_block_release(func, events, block, exit, ops, &mut fronts)?;
        }
    }
    Ok(())
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
    dom: &crate::graph::DominatorTree,
) -> Result<Vec<PlannedOp>, DeclineReason> {
    let borrowed = events.is_externally_funded();
    let mut ops = Vec::new();
    for (block, evs) in events.per_block.iter().enumerate() {
        for (position, ev) in evs.iter().enumerate() {
            if ev.kind != EventKind::Consume {
                continue;
            }
            if !borrowed
                && (same_site_credit_follows(evs, position)
                    || invoke_refund_credit_follows(func, events, block, ev)
                    || !(suffix_has_demand(evs, position)
                        || live_out_forward(func, block, demand_live, dom)))
            {
                continue;
            }
            let Some(var) = ev.var else {
                return Err(DeclineReason::UnresolvedOpVar);
            };
            let slot = match ev.site {
                EventSite::Body(index) => PlanSlot::BeforeBody { block, index },
                EventSite::Terminator => PlanSlot::BeforeTerminator { block },
                EventSite::BlockEntry => return Err(DeclineReason::UnplaceableRelease),
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

/// Whether a Terminator-site consume is refunded at the SAME call boundary
/// across an `Invoke`'s normal edge: the result credit routes to the NORMAL
/// successor's block entry, so the RL-34 transfer-with-refund pair spans
/// the edge instead of sharing a site.
fn invoke_refund_credit_follows(
    func: &ArcFunction,
    events: &ClassEvents,
    block: usize,
    consume: &ClassEvent,
) -> bool {
    if consume.site != EventSite::Terminator {
        return false;
    }
    let Some(arc_block) = func.blocks.get(block) else {
        return false;
    };
    let normal = match &arc_block.terminator {
        crate::ir::ArcTerminator::Invoke { normal, .. }
        | crate::ir::ArcTerminator::InvokeIndirect { normal, .. } => normal.index(),
        _ => return false,
    };
    events.per_block.get(normal).is_some_and(|evs| {
        evs.iter()
            .any(|ev| ev.kind == EventKind::Credit && ev.site == EventSite::BlockEntry)
    })
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

/// RL-4 per-edge releases for `block`'s dead successors: the class stays
/// live at the block exit but is dead at a successor — the dec is
/// attributed to that edge, materialized at the dead successor's front
/// (a multi-pred dead successor with divergent per-edge counts is caught by
/// the phase-2 merge-agreement gate). Same-class jump hand-offs are silent
/// in the event streams, so a threaded reference never reads as an edge
/// death.
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
        EventSite::BlockEntry => {
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
