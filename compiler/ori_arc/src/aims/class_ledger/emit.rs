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

mod cfg_region;
mod incs;
mod releases;

pub(crate) use cfg_region::CycleRegions;

use crate::aims::verify::burden_delta::compute_burden_entry_nets;
use crate::ir::{ArcFunction, ArcVarId};

use super::events::{event_blocks, live_from, live_from_forward, ClassEvents, EventKind};

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
    /// A required funding inc has no expressible insertion slot.
    UnplaceableInc,
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
    regions: &CycleRegions,
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
        return match releases::plan_dead_class_releases(func, events) {
            Ok(ops) => ClassOutcome::Planned(ops),
            Err(reason) => ClassOutcome::Declined(reason),
        };
    }
    // Funding (inc) decisions use FORWARD-only demand liveness: a back-edge
    // suffix is the next iteration's events, never continued use of the
    // current reference. Release placement keeps the full closure.
    let dom = crate::graph::DominatorTree::build(func);
    // Inv: per-block event indexing aligns with the function's block indexing
    // (and thus with CycleRegions).
    debug_assert_eq!(events.per_block.len(), func.blocks.len());
    // A class that silently THREADS a back-edge keeps the same reference
    // across iterations — full-closure liveness. A class with NO event
    // inside any CFG cycle is loop-INVARIANT (it crosses loops by
    // dominance, not per-iteration definition): a back-edge suffix IS
    // continued use of the same reference, so full closure applies to it
    // too. Otherwise back-edges are the next iteration's ledger:
    // forward-only for funding decisions AND the death frontier. ONE
    // discriminator drives BOTH the liveness vectors here and the per-block
    // live-out queries in the incs/releases sub-modules.
    let full_closure = events.threads_back_edge || !has_cycle_events(regions, events);
    let (demand_live, activity_live) = if full_closure {
        (
            live_from(func, &event_blocks(events, true)),
            live_from(func, &event_blocks(events, false)),
        )
    } else {
        (
            live_from_forward(func, &event_blocks(events, true), &dom),
            live_from_forward(func, &event_blocks(events, false), &dom),
        )
    };
    let mut ops = seed_ops.to_vec();
    match incs::plan_incs(func, events, &demand_live, full_closure, &dom) {
        Ok(planned) => ops.extend(planned),
        Err(reason) => return ClassOutcome::Declined(reason),
    }
    match incs::plan_select_credit_incs(events) {
        Ok(planned) => ops.extend(planned),
        Err(reason) => return ClassOutcome::Declined(reason),
    }
    let delta = per_block_delta(events, &ops, func.blocks.len());
    let nets = compute_burden_entry_nets(func, preds, &delta);
    if !nets.converged {
        return ClassOutcome::Declined(DeclineReason::NonConverged);
    }
    let plan_error = releases::plan_releases(
        func,
        preds,
        regions,
        events,
        &activity_live,
        full_closure,
        &nets.entry_net,
        &delta,
        &dom,
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

/// Whether any INSTANCE-CREATING event of the class sits inside a CFG
/// cycle (a block that can reach itself). Births, credits, and select
/// acquisitions inside a cycle mean a per-iteration instance (forward-only
/// liveness); reads and consumes never create instances — a class born
/// outside the cycle and only read within it is loop-invariant.
fn has_cycle_events(regions: &CycleRegions, events: &ClassEvents) -> bool {
    events
        .per_block
        .iter()
        .enumerate()
        .filter(|(_, evs)| {
            evs.iter()
                .any(|ev| ev.delta > 0 || ev.kind == EventKind::SelectCredit)
        })
        .any(|(block, _)| regions.in_cycle(block))
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
