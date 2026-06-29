//! Phase-6.99 credit-net verification — per-edge dataflow + the ordered
//! intra-block aliveness walk over a [`LineageModel`]'s events.
//!
//! Spec: Annex E §AIMS RL-2 + RL-4 + RL-comp.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::verify::burden_delta::compute_burden_entry_nets;
use crate::ir::{ArcFunction, ArcTerminator};

use super::model::{BlockEvents, BlockReads, LineageModel};

/// A simulated repair applied to the model during verification: an optional
/// inc-event removal plus an optional placed release. Singles are the Mode
/// 1 / Mode 2 repairs; both together are the Mode-3 combined repair (the
/// wrapped-anchor two-deficit shape).
#[derive(Clone, Copy, Default)]
pub(super) struct Change {
    /// Drop the body event at `(block, event_idx)` (an inc removal).
    pub(super) remove: Option<(usize, usize)>,
    /// A placed release at `(block, position)`.
    pub(super) place: Option<(usize, PlaceAt)>,
}

/// Where a placed release lands inside its block.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PlaceAt {
    /// After all body events, before the terminator events.
    EndOfBody,
    /// At the block front (after the entry credit).
    BlockFront,
}

impl Change {
    /// Whether the body event at `(block, event_idx)` is removed.
    fn removes(self, block: usize, event_idx: usize) -> bool {
        self.remove == Some((block, event_idx))
    }

    /// The placed release's position in `block`, if any.
    fn placed_at(self, block: usize) -> Option<PlaceAt> {
        match self.place {
            Some((b, at)) if b == block => Some(at),
            _ => None,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub(super) enum NetVerdict {
    /// Every Return terminal nets 0; every Resume/Unreachable nets >= 0; the
    /// running net stays >= 1 at every member use point.
    Clean,
    /// Some Return terminal nets > 0 (a leak); everything else holds.
    LeakOnly,
    /// Merge disagreement, a negative terminal, a dead member use, or no
    /// Return terminal.
    Unprovable,
}

/// Classify the model's nets under `change` (per-edge dataflow via the
/// 6.98-family [`compute_burden_entry_nets`] SSOT + the ordered intra-block
/// aliveness walk).
pub(super) fn classify_model(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    model: &LineageModel,
    change: Change,
) -> NetVerdict {
    let n = func.blocks.len();
    let mut delta = vec![0i64; n];
    for (b, ev) in model.events.iter().enumerate() {
        delta[b] = block_total_delta(ev, b, change);
    }

    let nets = compute_burden_entry_nets(func, preds, &delta);
    // Convergence guard: this net-zero verdict reads `entry_net`, authoritative
    // only on a converged result. On `!converged` (freeze-on-disagree / cap
    // exhaustion) the nets are stale and `disagree_blocks` may be empty, so a
    // release/elision verdict derived from them would be non-deterministic.
    // Decline — Unprovable falls back to the safe baseline; the verifier
    // reports the non-convergence as a HARD failure (Spec: Annex E §AIMS RL-2).
    if !nets.converged {
        return NetVerdict::Unprovable;
    }
    if !nets.disagree_blocks.is_empty() {
        return NetVerdict::Unprovable;
    }

    let mut saw_return = false;
    let mut leak = false;
    for (b, block) in func.blocks.iter().enumerate() {
        let Some(Some(entry)) = nets.entry_net.get(b) else {
            continue;
        };
        let Some(exit) = block_running_net(&model.events[b], b, *entry, change) else {
            // A member use point with running net < 1 — a dead-value use.
            return NetVerdict::Unprovable;
        };
        match &block.terminator {
            ArcTerminator::Return { .. } => {
                saw_return = true;
                if exit > 0 {
                    leak = true;
                } else if exit < 0 {
                    return NetVerdict::Unprovable;
                }
            }
            ArcTerminator::Resume | ArcTerminator::Unreachable => {
                if exit < 0 {
                    return NetVerdict::Unprovable;
                }
            }
            _ => {}
        }
    }
    if !saw_return {
        return NetVerdict::Unprovable;
    }
    if leak {
        NetVerdict::LeakOnly
    } else {
        NetVerdict::Clean
    }
}

/// The block's total delta under `change` (entry credit + body + term).
fn block_total_delta(ev: &BlockEvents, b: usize, change: Change) -> i64 {
    let mut total = ev.entry_credit;
    for (idx, e) in ev.body.iter().enumerate() {
        if change.removes(b, idx) {
            continue;
        }
        total += e.delta;
    }
    for e in &ev.term {
        total += e.delta;
    }
    if change.placed_at(b).is_some() {
        total -= 1;
    }
    total
}

/// Walk the block's ordered events from `entry`, enforcing the aliveness
/// invariant (running `>= 1` before every `alive` event). Returns the exit
/// net, or `None` on a dead-value use.
fn block_running_net(ev: &BlockEvents, b: usize, entry: i64, change: Change) -> Option<i64> {
    let mut running = entry + ev.entry_credit;
    let placed = change.placed_at(b);
    if placed == Some(PlaceAt::BlockFront) {
        if running < 1 {
            return None;
        }
        running -= 1;
    }
    for (idx, e) in ev.body.iter().enumerate() {
        if change.removes(b, idx) {
            continue;
        }
        if e.alive && running < 1 {
            return None;
        }
        running += e.delta;
    }
    if placed == Some(PlaceAt::EndOfBody) {
        if running < 1 {
            return None;
        }
        running -= 1;
    }
    for e in &ev.term {
        if e.alive && running < 1 {
            return None;
        }
        running += e.delta;
    }
    Some(running)
}

/// Candidate sinks: blocks with a member value-read and NO member value-read
/// in any block strictly forward-reachable from them.
pub(super) fn final_value_read_sinks(
    func: &ArcFunction,
    read_blocks: &FxHashMap<usize, BlockReads>,
) -> Vec<usize> {
    let mut sinks: Vec<usize> = Vec::new();
    for &b in read_blocks.keys() {
        let mut reachable = FxHashSet::default();
        let mut stack: Vec<usize> = successor_indices(func, b);
        while let Some(s) = stack.pop() {
            if !reachable.insert(s) {
                continue;
            }
            stack.extend(successor_indices(func, s));
        }
        if read_blocks
            .keys()
            .any(|&r| r != b && reachable.contains(&r))
        {
            continue;
        }
        // A read block that re-reaches itself is loop-carried — not a sink.
        if reachable.contains(&b) {
            continue;
        }
        sinks.push(b);
    }
    sinks.sort_unstable();
    sinks
}

fn successor_indices(func: &ArcFunction, b: usize) -> Vec<usize> {
    func.blocks
        .get(b)
        .map(|block| {
            crate::graph::successor_block_ids(&block.terminator)
                .iter()
                .map(|s| s.index())
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `b` lies on a CFG cycle (reachable from itself).
pub(super) fn block_in_cycle(func: &ArcFunction, b: usize) -> bool {
    let mut reachable = FxHashSet::default();
    let mut stack = successor_indices(func, b);
    while let Some(s) = stack.pop() {
        if s == b {
            return true;
        }
        if reachable.insert(s) {
            stack.extend(successor_indices(func, s));
        }
    }
    false
}
