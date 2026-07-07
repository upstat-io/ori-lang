//! Backward liveness fixpoints over the class-ledger event streams: a
//! full-closure variant and a forward-only variant that skips back-edges,
//! plus the per-block live-out queries the emitter's incs/releases planning
//! consumes. Spec: Annex E §AIMS RL-1 + RL-2 + RL-4.

use crate::ir::ArcFunction;

use super::successors_of;

/// Backward closure: `true` at `b` iff `seed[b]` or any block reachable
/// from `b` is seeded. Monotone boolean fixpoint (at most `n` rounds).
pub(crate) fn live_from(func: &ArcFunction, seed: &[bool]) -> Vec<bool> {
    live_from_with(func, seed, |_, _| true)
}

/// [`live_from`] with entry-credit KILLS: a block whose entry carries a
/// CREDIT re-acquisition refunds its own (and downstream) demand, so demand
/// never propagates from a killed block back into its predecessors — the
/// credit, not a pre-consume duplication inc, funds that demand.
/// The killed block's own seed still counts at the block itself.
pub(crate) fn live_from_killing(func: &ArcFunction, seed: &[bool], kills: &[bool]) -> Vec<bool> {
    live_from_with(func, seed, |_, successor| {
        !kills.get(successor).copied().unwrap_or(false)
    })
}

/// [`live_from_killing`] restricted to FORWARD edges (see
/// [`live_from_forward`]).
pub(crate) fn live_from_forward_killing(
    func: &ArcFunction,
    seed: &[bool],
    kills: &[bool],
    dom: &crate::graph::DominatorTree,
) -> Vec<bool> {
    live_from_with(func, seed, |block, successor| {
        if kills.get(successor).copied().unwrap_or(false) {
            return false;
        }
        let (Some(from), Some(to)) = (func.blocks.get(block), func.blocks.get(successor)) else {
            return false;
        };
        !dom.dominates(to.id, from.id)
    })
}

/// [`live_from`] restricted to FORWARD edges: propagation skips back-edges
/// (a successor that dominates its block). A back-edge suffix is the NEXT
/// iteration's events, not continued use of the current reference — funding
/// decisions must not read it as later demand.
pub(crate) fn live_from_forward(
    func: &ArcFunction,
    seed: &[bool],
    dom: &crate::graph::DominatorTree,
) -> Vec<bool> {
    live_from_with(func, seed, |block, successor| {
        let (Some(from), Some(to)) = (func.blocks.get(block), func.blocks.get(successor)) else {
            return false;
        };
        !dom.dominates(to.id, from.id)
    })
}

/// Shared fixpoint over successor edges admitted by `edge_ok(block, succ)`.
fn live_from_with(
    func: &ArcFunction,
    seed: &[bool],
    edge_ok: impl Fn(usize, usize) -> bool,
) -> Vec<bool> {
    let mut live = seed.to_vec();
    let mut changed = true;
    while changed {
        changed = false;
        for block in 0..live.len() {
            if live[block] {
                continue;
            }
            if successors_of(func, block)
                .iter()
                .any(|&s| live[s] && edge_ok(block, s))
            {
                live[block] = true;
                changed = true;
            }
        }
    }
    live
}

/// Whether any successor of `block` is live.
pub(crate) fn live_out(func: &ArcFunction, block: usize, live: &[bool]) -> bool {
    successors_of(func, block)
        .iter()
        .any(|&s| live.get(s).copied().unwrap_or(false))
}

/// [`live_out`] restricted to FORWARD out-edges: a back-edge continuation
/// (successor dominates the block) is the next iteration, never continued
/// use of the current reference.
pub(crate) fn live_out_forward(
    func: &ArcFunction,
    block: usize,
    live: &[bool],
    dom: &crate::graph::DominatorTree,
) -> bool {
    successors_of(func, block).iter().any(|&s| {
        live.get(s).copied().unwrap_or(false)
            && match (func.blocks.get(block), func.blocks.get(s)) {
                (Some(from), Some(to)) => !dom.dominates(to.id, from.id),
                _ => false,
            }
    })
}
