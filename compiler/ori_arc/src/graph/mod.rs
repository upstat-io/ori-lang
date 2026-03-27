//! Shared graph analysis utilities for ARC optimization passes.
//!
//! Functions in this module are generic graph operations on [`ArcFunction`]
//! that multiple independent passes need. They live here rather than in a
//! specific pass module so that passes do not import from each other —
//! keeping the dependency graph flat (all passes depend on `graph`, none
//! depend on each other).
//!
//! ## Submodules
//!
//! - [`call_graph`] — inter-function call graph for SCC-based borrow inference
//! - [`dominator`] — dominator tree (Cooper-Harvey-Kennedy algorithm)
//! - [`post_dominator`] — post-dominator tree (CHK on reverse CFG)

pub mod call_graph;
mod dominator;
mod post_dominator;
pub mod scc;

pub use dominator::DominatorTree;
pub use post_dominator::PostDominatorTree;

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::{smallvec, SmallVec};

use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};

/// Compute the predecessor list for each block (deduplicated).
///
/// Returns a vector indexed by block index, where each entry is the
/// list of distinct predecessor block indices.
pub fn compute_predecessors(func: &ArcFunction) -> Vec<Vec<usize>> {
    let num_blocks = func.blocks.len();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); num_blocks];

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen = FxHashSet::default();
        for succ_id in successor_block_ids(&block.terminator) {
            let succ_idx = succ_id.index();
            if succ_idx < num_blocks && seen.insert(succ_idx) {
                predecessors[succ_idx].push(block_idx);
            }
        }
    }

    predecessors
}

/// Extract successor block IDs from a terminator.
///
/// Returns `SmallVec<[ArcBlockId; 4]>` to avoid heap allocation for the
/// common case (max 2 successors except Switch with many cases).
pub fn successor_block_ids(terminator: &ArcTerminator) -> SmallVec<[ArcBlockId; 4]> {
    match terminator {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {
            SmallVec::new()
        }
        ArcTerminator::Jump { target, .. } => smallvec![*target],
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => smallvec![*then_block, *else_block],
        ArcTerminator::Switch { cases, default, .. } => {
            let mut targets = SmallVec::with_capacity(cases.len() + 1);
            for &(_, b) in cases {
                targets.push(b);
            }
            targets.push(*default);
            targets
        }
        ArcTerminator::Invoke { normal, unwind, .. } => smallvec![*normal, *unwind],
    }
}

/// Collect Invoke `dst` definitions mapped to their normal successor blocks.
///
/// An `Invoke { dst, normal, .. }` defines `dst` at the entry of `normal`.
/// This is analogous to how LLVM's `invoke` instruction defines its result
/// in the normal successor only, not the unwind successor. We collect these
/// so `compute_gen_kill` can add them to the kill set of the normal block.
pub(crate) fn collect_invoke_defs(func: &ArcFunction) -> FxHashMap<ArcBlockId, Vec<ArcVarId>> {
    let mut map = FxHashMap::default();
    for block in &func.blocks {
        if let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator {
            map.entry(*normal).or_insert_with(Vec::new).push(*dst);
        }
    }
    map
}

/// Compute predecessor counts for each block (deduplicated).
///
/// Unlike [`compute_predecessors`] which returns full predecessor lists,
/// this returns only counts — sufficient for single-predecessor checks
/// and cheaper to compute.
pub(crate) fn compute_pred_counts(func: &ArcFunction) -> Vec<usize> {
    let num_blocks = func.blocks.len();
    let mut counts = vec![0usize; num_blocks];

    for block in &func.blocks {
        let mut seen = FxHashSet::default();
        for succ in successor_block_ids(&block.terminator) {
            let si = succ.index();
            if si < num_blocks && seen.insert(si) {
                counts[si] += 1;
            }
        }
    }

    counts
}

/// Compute a postorder traversal of the CFG starting from the entry block.
///
/// Uses an iterative DFS with an explicit stack to avoid recursion depth
/// issues on deeply nested CFGs. Only visits reachable blocks.
///
/// Used by liveness analysis (convergence ordering) and the dominator tree
/// (reverse postorder). Shared here so both consumers use the same
/// traversal implementation.
pub fn compute_postorder(func: &ArcFunction) -> Vec<usize> {
    let num_blocks = func.blocks.len();
    let mut visited = vec![false; num_blocks];
    let mut postorder = Vec::with_capacity(num_blocks);

    // Stack entries: (block_index, children_processed).
    // When children_processed is false, we push successors.
    // When true, we emit the block to postorder.
    let mut stack: Vec<(usize, bool)> = vec![(func.entry.index(), false)];

    while let Some(&mut (block_idx, ref mut children_done)) = stack.last_mut() {
        if *children_done {
            postorder.push(block_idx);
            stack.pop();
            continue;
        }

        *children_done = true;

        if block_idx >= num_blocks {
            stack.pop();
            continue;
        }

        if visited[block_idx] {
            stack.pop();
            continue;
        }
        visited[block_idx] = true;

        // Push successors (they'll be processed before we come back to
        // emit this block).
        let block = &func.blocks[block_idx];
        for succ_id in successor_block_ids(&block.terminator) {
            let succ_idx = succ_id.index();
            if succ_idx < num_blocks && !visited[succ_idx] {
                stack.push((succ_idx, false));
            }
        }
    }

    postorder
}

/// CHK intersect: walk two fingers upward until they meet.
///
/// Both `a` and `b` must be reachable from the entry — their idom chain
/// always leads to the entry node, so `idom[x]` is always `Some` here.
///
/// Shared between [`DominatorTree`] and [`PostDominatorTree`].
pub(super) fn chk_intersect(
    mut a: usize,
    mut b: usize,
    idom: &[Option<usize>],
    rpo_pos: &[usize],
) -> usize {
    while a != b {
        while rpo_pos[a] > rpo_pos[b] {
            // Safety: CHK algorithm guarantees convergence — all reachable
            // nodes have an idom leading to the entry.
            let Some(next) = idom[a] else {
                debug_assert!(false, "intersect: broken idom chain at {a}");
                return a;
            };
            a = next;
        }
        while rpo_pos[b] > rpo_pos[a] {
            let Some(next) = idom[b] else {
                debug_assert!(false, "intersect: broken idom chain at {b}");
                return b;
            };
            b = next;
        }
    }
    a
}

#[cfg(test)]
mod tests;
