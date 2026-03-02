//! Post-dominator tree for ARC IR functions.
//!
//! Block A post-dominates block B if every path from B to any function
//! exit must pass through A. Computed via CHK on the reverse CFG with
//! a virtual exit node unifying all exit blocks (`Return`/`Resume`/`Unreachable`).
//!
//! Used by cross-block reset/reuse detection to verify that a `Construct`
//! block always executes after the `RcDec` block — preventing memory leaks
//! when the `Construct` is on only one branch of a conditional.
//!
//! Reference: Cooper, Harvey, Kennedy — "A Simple, Fast Dominance Algorithm" (2001)

use smallvec::SmallVec;

use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator};

use super::{chk_intersect, successor_block_ids};

/// Post-dominator tree for ARC IR functions.
pub struct PostDominatorTree {
    /// Immediate post-dominator per block. The virtual exit node (index `n`)
    /// is the root but is not exposed to callers.
    pub(crate) ipdom: Vec<Option<usize>>,
}

impl PostDominatorTree {
    /// Build the post-dominator tree for a function.
    ///
    /// The reverse CFG uses a virtual exit node (index = `num_blocks`) that
    /// unifies all exit blocks. The CHK algorithm runs on reverse postorder
    /// of the reverse CFG starting from the virtual exit.
    pub fn build(func: &ArcFunction) -> Self {
        let n = func.blocks.len();
        if n == 0 {
            return Self { ipdom: vec![] };
        }

        let virtual_exit = n;

        // Build reverse CFG predecessors: for each edge A → B in the original
        // CFG, add A as a successor of B in the reverse CFG.
        // The virtual exit has all exit blocks as successors (reverse: exit
        // blocks have virtual_exit as predecessor in reverse CFG).
        let mut rev_succs: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); n + 1];
        let mut exit_blocks = Vec::new();

        for (block_idx, block) in func.blocks.iter().enumerate() {
            if matches!(
                block.terminator,
                ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable
            ) {
                exit_blocks.push(block_idx);
            }

            for succ_id in successor_block_ids(&block.terminator) {
                let succ_idx = succ_id.index();
                if succ_idx < n {
                    // Reverse edge: succ → block in reverse CFG
                    rev_succs[succ_idx].push(block_idx);
                }
            }
        }

        // Virtual exit → exit blocks (in reverse CFG, exit blocks reach virtual exit)
        for &exit_idx in &exit_blocks {
            rev_succs[virtual_exit].push(exit_idx);
        }

        // If there are no exit blocks, every block is unreachable and post-dominance
        // is trivially undefined.
        if exit_blocks.is_empty() {
            return Self {
                ipdom: vec![None; n],
            };
        }

        // Compute reverse postorder of the reverse CFG starting from virtual_exit.
        let rpo = Self::reverse_postorder_from(&rev_succs, virtual_exit, n + 1);

        // Build reverse predecessor lists (successors in the reverse CFG are
        // predecessors for the CHK algorithm on the reverse graph).
        let mut rev_preds: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); n + 1];
        for (src, dsts) in rev_succs.iter().enumerate() {
            for &dst in dsts {
                rev_preds[dst].push(src);
            }
        }

        // Map node → RPO position
        let mut rpo_pos = vec![0usize; n + 1];
        for (pos, &node) in rpo.iter().enumerate() {
            rpo_pos[node] = pos;
        }

        // CHK fixed-point on the reverse CFG
        let mut ipdom: Vec<Option<usize>> = vec![None; n + 1];
        ipdom[virtual_exit] = Some(virtual_exit);

        let mut changed = true;
        while changed {
            changed = false;
            for &node in &rpo[1..] {
                let mut new_idom = None;
                for &pred in &rev_preds[node] {
                    if ipdom[pred].is_some() {
                        new_idom = Some(pred);
                        break;
                    }
                }

                let Some(mut new_idom_val) = new_idom else {
                    continue;
                };

                for &pred in &rev_preds[node] {
                    if pred == new_idom_val {
                        continue;
                    }
                    if ipdom[pred].is_some() {
                        new_idom_val = chk_intersect(pred, new_idom_val, &ipdom, &rpo_pos);
                    }
                }

                if ipdom[node] != Some(new_idom_val) {
                    ipdom[node] = Some(new_idom_val);
                    changed = true;
                }
            }
        }

        // Truncate to only real blocks (drop virtual exit entry).
        ipdom.truncate(n);

        Self { ipdom }
    }

    /// Does block `a` post-dominate block `b`?
    ///
    /// Returns `true` if every path from `b` to any function exit must
    /// pass through `a`. A block post-dominates itself.
    pub fn post_dominates(&self, a: ArcBlockId, b: ArcBlockId) -> bool {
        let a_idx = a.index();
        let n = self.ipdom.len();
        let mut current = b.index();
        loop {
            if current == a_idx {
                return true;
            }
            match self.ipdom.get(current).copied().flatten() {
                Some(dom) if dom != current && dom < n => current = dom,
                // Reached virtual exit (dom >= n) or self-loop → stop.
                _ => return current == a_idx,
            }
        }
    }

    /// Compute reverse postorder of a graph from a given root.
    fn reverse_postorder_from(
        succs: &[SmallVec<[usize; 4]>],
        root: usize,
        num_nodes: usize,
    ) -> Vec<usize> {
        let mut visited = vec![false; num_nodes];
        let mut postorder = Vec::with_capacity(num_nodes);
        let mut stack: Vec<(usize, bool)> = vec![(root, false)];

        while let Some(&mut (node, ref mut children_done)) = stack.last_mut() {
            if *children_done {
                postorder.push(node);
                stack.pop();
                continue;
            }
            *children_done = true;

            if node >= num_nodes || visited[node] {
                stack.pop();
                continue;
            }
            visited[node] = true;

            for &succ in &succs[node] {
                if succ < num_nodes && !visited[succ] {
                    stack.push((succ, false));
                }
            }
        }

        postorder.reverse();
        postorder
    }
}
