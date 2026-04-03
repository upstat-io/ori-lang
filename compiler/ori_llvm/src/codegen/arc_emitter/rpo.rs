//! Reverse Post-Order (RPO) traversal for ARC function blocks.
//!
//! RPO guarantees that a block's dominators (and thus variable definitions
//! from preceding blocks) are visited before the block itself. This is
//! critical after `expand_reuse`, which appends fast/slow/merge blocks at
//! the end of the block array — their Invoke terminators target existing
//! blocks with lower indices, creating forward references if iterated in
//! array order.

use ori_arc::ir::{ArcFunction, ArcTerminator};
use rustc_hash::FxHashSet;

/// DFS helper for RPO computation. Visits successors then appends self
/// to post-order list.
fn rpo_dfs(
    func: &ArcFunction,
    idx: usize,
    visited: &mut [bool],
    post_order: &mut Vec<usize>,
    dead: &FxHashSet<usize>,
) {
    if idx >= func.blocks.len() || visited[idx] || dead.contains(&idx) {
        return;
    }
    visited[idx] = true;

    match &func.blocks[idx].terminator {
        ArcTerminator::Jump { target, .. } => {
            rpo_dfs(func, target.index(), visited, post_order, dead);
        }
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            rpo_dfs(func, then_block.index(), visited, post_order, dead);
            rpo_dfs(func, else_block.index(), visited, post_order, dead);
        }
        ArcTerminator::Switch { cases, default, .. } => {
            for &(_, target) in cases {
                rpo_dfs(func, target.index(), visited, post_order, dead);
            }
            rpo_dfs(func, default.index(), visited, post_order, dead);
        }
        ArcTerminator::Invoke { normal, unwind, .. }
        | ArcTerminator::InvokeIndirect { normal, unwind, .. } => {
            rpo_dfs(func, normal.index(), visited, post_order, dead);
            rpo_dfs(func, unwind.index(), visited, post_order, dead);
        }
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }

    post_order.push(idx);
}

/// Compute Reverse Post-Order (RPO) traversal of ARC function blocks.
///
/// RPO guarantees that a block's dominators (and thus variable definitions
/// from preceding blocks) are visited before the block itself.
pub(super) fn compute_block_rpo(func: &ArcFunction, dead: &FxHashSet<usize>) -> Vec<usize> {
    let n = func.blocks.len();
    let mut visited = vec![false; n];
    let mut post_order = Vec::with_capacity(n);
    rpo_dfs(
        func,
        func.entry.index(),
        &mut visited,
        &mut post_order,
        dead,
    );
    post_order.reverse();
    post_order
}
