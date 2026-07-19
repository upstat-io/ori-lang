//! Per-edge merge equalization for disagreeing owed counts.
//!
//! A merge whose predecessor edges exit with divergent owed counts is
//! equalized by releasing the surplus on each owing cycle-interior edge at
//! the predecessor's end, driven to a fixpoint one merge per round.

use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::aims::verify::burden_delta::{compute_burden_entry_nets, BurdenEntryNets};
use crate::ir::{ArcFunction, ArcVarId};

use super::super::events::{successors_of, ClassEvents};
use super::{per_block_delta, CycleRegions, PlanSlot, PlannedOp, PlannedOpKind};

/// Drive per-edge merge equalization to a fixpoint: a downstream merge's
/// disagreement can be DERIVED from an upstream one (the frozen
/// first-agreed net propagates), so equalize ONE resolvable merge per
/// round and recompute; an unresolvable residue falls through to the
/// final `MergeDisagree` gate. `false` iff a recompute failed to converge.
pub(super) fn equalize_disagreeing_merges(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    regions: &CycleRegions,
    events: &ClassEvents,
    ops: &mut Vec<PlannedOp>,
    delta: &mut Vec<i64>,
    nets: &mut BurdenEntryNets,
) -> bool {
    let mut rounds = 0;
    while !nets.disagree_blocks.is_empty() && rounds <= nets.disagree_blocks.len() {
        rounds += 1;
        let Some(extra) = nets.disagree_blocks.iter().find_map(|&(merge, _)| {
            equalize_one_merge(func, preds, regions, events, delta, nets, merge)
        }) else {
            break;
        };
        ops.extend(extra);
        *delta = per_block_delta(events, ops, func.blocks.len());
        *nets = compute_burden_entry_nets(func, preds, delta);
        if !nets.converged {
            return false;
        }
    }
    true
}

/// The per-edge equalizing releases for disagreeing merges: for each merge
/// whose predecessor edges exit with divergent owed counts, release the
/// surplus (exit − min-exit) on each owing edge at the predecessor's end.
/// The released member is the var a MIN-exit sibling edge hands off at its
/// Terminator (the class's merge-feeding member, dying unpassed on the
/// owing edge). `None` when the shape is not equalizable — a pred with an
/// undefined net, no unique sibling hand-off candidate, a multi-successor
/// owing pred (the release would leak onto other edges), or the candidate
/// passed by the owing pred itself.
fn equalize_one_merge(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    regions: &CycleRegions,
    events: &ClassEvents,
    delta: &[i64],
    nets: &BurdenEntryNets,
    merge: usize,
) -> Option<Vec<PlannedOp>> {
    let mut pred_exits: Vec<(usize, i64)> = Vec::new();
    for &p in &preds[merge] {
        let entry = nets.entry_net[p]?;
        pred_exits.push((p, entry + delta[p]));
    }
    if pred_exits.len() < 2 {
        return None;
    }
    let target = pred_exits.iter().map(|&(_, exit)| exit).min()?;
    // The candidate: the member a min-exit edge hands off at its Terminator
    // (a negative-delta Terminator event names the var whose reference the
    // sibling edge transfers; the owing edge's copy of the SAME member dies
    // unpassed).
    let candidate = pred_exits
        .iter()
        .filter(|&&(_, exit)| exit == target)
        .find_map(|&(p, _)| {
            events.per_block.get(p)?.iter().find_map(|event| {
                (matches!(event.site, EventSite::Terminator) && event.delta < 0)
                    .then_some(event.var)
                    .flatten()
            })
        })?;
    let mut extra: Vec<PlannedOp> = Vec::new();
    for &(p, exit) in &pred_exits {
        let surplus = exit - target;
        if surplus <= 0 {
            continue;
        }
        // Cycle-interior edges only: the death-frontier walk owns acyclic
        // dead-arm releases (its RL-4 edge dec); a surplus edge inside a
        // cycle is the shape the frontier cannot place (every loop block
        // stays activity-live).
        if !regions.is_in_cycle(p) {
            return None;
        }
        let successors = successors_of(func, p);
        if successors.len() != 1 || successors[0] != merge {
            return None;
        }
        if jump_args_contain(func, p, candidate) {
            return None;
        }
        for _ in 0..surplus {
            extra.push(PlannedOp {
                slot: PlanSlot::BeforeTerminator { block: p },
                kind: PlannedOpKind::Dec,
                var: candidate,
            });
        }
    }
    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

/// Whether block `b`'s terminator passes `var` as a jump argument.
fn jump_args_contain(func: &ArcFunction, b: usize, var: ArcVarId) -> bool {
    match &func.blocks[b].terminator {
        crate::ir::ArcTerminator::Jump { args, .. } => args.contains(&var),
        _ => false,
    }
}
