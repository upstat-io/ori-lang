//! Shared CFG analysis utilities for ARC optimization passes.
//!
//! Functions in this module are generic graph operations on [`ArcFunction`]
//! that multiple independent passes need. They live here rather than in a
//! specific pass module so that passes do not import from each other —
//! keeping the dependency graph flat (all passes depend on `graph`, none
//! depend on each other).

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::{smallvec, SmallVec};

use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};

/// Compute the predecessor list for each block (deduplicated).
///
/// Returns a vector indexed by block index, where each entry is the
/// list of distinct predecessor block indices.
pub(crate) fn compute_predecessors(func: &ArcFunction) -> Vec<Vec<usize>> {
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
pub(crate) fn successor_block_ids(terminator: &ArcTerminator) -> SmallVec<[ArcBlockId; 4]> {
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

/// Compute a postorder traversal of the CFG starting from the entry block.
///
/// Uses an iterative DFS with an explicit stack to avoid recursion depth
/// issues on deeply nested CFGs. Only visits reachable blocks.
///
/// Used by liveness analysis (convergence ordering) and the dominator tree
/// (reverse postorder). Shared here so both consumers use the same
/// traversal implementation.
pub(crate) fn compute_postorder(func: &ArcFunction) -> Vec<usize> {
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

/// Dominator tree for ARC IR functions.
///
/// Uses the Cooper-Harvey-Kennedy iterative algorithm, which is simpler than
/// Lengauer-Tarjan and fast enough for typical function sizes (< 100 blocks).
/// The algorithm works on reverse postorder and converges in O(n * d) where
/// d is the loop nesting depth — typically 2-3 iterations.
///
/// Used by cross-block reset/reuse detection and FBIP diagnostics to verify
/// that a token defined in block A can be used in block B (requires A
/// dominates B).
///
/// Reference: Cooper, Harvey, Kennedy — "A Simple, Fast Dominance Algorithm" (2001)
pub struct DominatorTree {
    /// Immediate dominator for each block, indexed by block index.
    /// `idom[entry] == None`, all others have `Some(dominator_index)`.
    idom: Vec<Option<usize>>,
}

impl DominatorTree {
    /// Build the dominator tree for a function.
    pub fn build(func: &ArcFunction) -> Self {
        let n = func.blocks.len();
        if n == 0 {
            return Self { idom: vec![] };
        }

        let preds = compute_predecessors(func);
        let rpo = Self::reverse_postorder(func);

        // Map block index → RPO position for O(1) lookup
        let mut rpo_pos = vec![0usize; n];
        for (pos, &block_idx) in rpo.iter().enumerate() {
            rpo_pos[block_idx] = pos;
        }

        let entry = func.entry.index();
        let mut idom: Vec<Option<usize>> = vec![None; n];
        idom[entry] = Some(entry); // entry dominates itself

        let mut changed = true;
        while changed {
            changed = false;
            // Iterate in RPO (skip entry at position 0)
            for &block_idx in &rpo[1..] {
                // Find first processed predecessor
                let mut new_idom = None;
                for &pred in &preds[block_idx] {
                    if idom[pred].is_some() {
                        new_idom = Some(pred);
                        break;
                    }
                }

                let Some(mut new_idom_val) = new_idom else {
                    continue;
                };

                // Intersect with remaining processed predecessors
                for &pred in &preds[block_idx] {
                    if pred == new_idom_val {
                        continue;
                    }
                    if idom[pred].is_some() {
                        new_idom_val = Self::intersect(pred, new_idom_val, &idom, &rpo_pos);
                    }
                }

                if idom[block_idx] != Some(new_idom_val) {
                    idom[block_idx] = Some(new_idom_val);
                    changed = true;
                }
            }
        }

        Self { idom }
    }

    /// Does block `a` dominate block `b`?
    ///
    /// A block dominates itself. The entry block dominates all blocks.
    pub fn dominates(&self, a: ArcBlockId, b: ArcBlockId) -> bool {
        let a_idx = a.index();
        let mut current = b.index();
        loop {
            if current == a_idx {
                return true;
            }
            match self.idom[current] {
                Some(dom) if dom != current => current = dom,
                _ => return current == a_idx,
            }
        }
    }

    /// Return dominated blocks of `a` in preorder (for walking the subtree).
    pub fn dominated_preorder(&self, root: ArcBlockId, num_blocks: usize) -> Vec<ArcBlockId> {
        // Build children lists from idom
        let mut children: Vec<Vec<usize>> = vec![vec![]; num_blocks];
        for (idx, &idom) in self.idom.iter().enumerate() {
            if let Some(dom) = idom {
                if dom != idx {
                    children[dom].push(idx);
                }
            }
        }

        let mut result = Vec::new();
        let mut stack = vec![root.index()];
        while let Some(idx) = stack.pop() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR block counts fit in u32"
            )]
            result.push(ArcBlockId::new(idx as u32));
            // Push in reverse order so left children are visited first
            for &child in children[idx].iter().rev() {
                stack.push(child);
            }
        }
        result
    }

    /// Compute reverse postorder traversal of the CFG.
    fn reverse_postorder(func: &ArcFunction) -> Vec<usize> {
        let mut rpo = compute_postorder(func);
        rpo.reverse();
        rpo
    }

    /// CHK intersect: walk two fingers upward until they meet.
    ///
    /// Both `a` and `b` must be reachable from the entry — their idom chain
    /// always leads to the entry node, so `idom[x]` is always `Some` here.
    fn intersect(mut a: usize, mut b: usize, idom: &[Option<usize>], rpo_pos: &[usize]) -> usize {
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
}

#[cfg(test)]
mod tests;
