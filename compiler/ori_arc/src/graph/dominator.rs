//! Dominator tree for ARC IR functions.
//!
//! Uses the Cooper-Harvey-Kennedy iterative algorithm, which is simpler than
//! Lengauer-Tarjan and fast enough for typical function sizes (< 100 blocks).
//! The algorithm works on reverse postorder and converges in O(n * d) where
//! d is the loop nesting depth — typically 2-3 iterations.
//!
//! Used by cross-block reset/reuse detection and FBIP diagnostics to verify
//! that a token defined in block A can be used in block B (requires A
//! dominates B).
//!
//! Reference: Cooper, Harvey, Kennedy — "A Simple, Fast Dominance Algorithm" (2001)

use crate::ir::{ArcBlockId, ArcFunction};

use super::{chk_intersect, compute_postorder, compute_predecessors};

/// Dominator tree for ARC IR functions.
pub struct DominatorTree {
    /// Immediate dominator for each block, indexed by block index.
    /// `idom[entry] == None`, all others have `Some(dominator_index)`.
    pub(crate) idom: Vec<Option<usize>>,
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
                        new_idom_val = chk_intersect(pred, new_idom_val, &idom, &rpo_pos);
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
}
