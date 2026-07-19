//! Control-flow reachability queries for class-event extraction and planning.

use rustc_hash::FxHashSet;

use crate::graph::successor_block_ids;
use crate::ir::ArcFunction;

/// Blocks reachable from `block`'s successors (transitively; includes
/// `block` itself on a cycle), ascending order.
pub(super) fn reachable_from(func: &ArcFunction, block: usize) -> Vec<usize> {
    let mut reachable = FxHashSet::default();
    let mut stack = successors_of(func, block);
    while let Some(next) = stack.pop() {
        if reachable.insert(next) {
            stack.extend(successors_of(func, next));
        }
    }
    let mut ordered: Vec<usize> = reachable.into_iter().collect();
    ordered.sort_unstable();
    ordered
}

/// Distinct in-range successor indices of `block`.
pub(crate) fn successors_of(func: &ArcFunction, block: usize) -> Vec<usize> {
    let Some(arc_block) = func.blocks.get(block) else {
        return Vec::new();
    };
    let mut seen = FxHashSet::default();
    successor_block_ids(&arc_block.terminator)
        .iter()
        .map(|id| id.index())
        .filter(|&idx| idx < func.blocks.len() && seen.insert(idx))
        .collect()
}
