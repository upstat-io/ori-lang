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
///
/// # Extra roots
///
/// `extra_roots` seeds the DFS from additional entry points beyond
/// `func.entry`. Metadata-referenced landing blocks have no CFG predecessor,
/// so each must be seeded as a block root. Extra-root RPO follows the
/// entry-reachable RPO so cleanup operands are materialized first.
pub(super) fn compute_block_rpo(
    func: &ArcFunction,
    dead: &FxHashSet<usize>,
    extra_roots: &[usize],
) -> Vec<usize> {
    let n = func.blocks.len();
    let mut visited = vec![false; n];
    let mut entry_post_order = Vec::with_capacity(n);
    rpo_dfs(
        func,
        func.entry.index(),
        &mut visited,
        &mut entry_post_order,
        dead,
    );
    entry_post_order.reverse();

    let mut rpo = entry_post_order;
    for &root in extra_roots {
        let mut extra_post_order = Vec::new();
        rpo_dfs(func, root, &mut visited, &mut extra_post_order, dead);
        extra_post_order.reverse();
        rpo.extend(extra_post_order);
    }
    rpo
}
