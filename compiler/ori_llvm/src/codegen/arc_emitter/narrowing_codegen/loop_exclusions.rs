//! Loop-carried SSA values retain canonical width to preserve induction simplification.

#[cfg(test)]
mod tests;

use ori_arc::ir::{ArcFunction, ArcVarId};

/// Values carried by a CFG cycle stay at the canonical integer width.
///
/// Narrowing a loop induction from i64 to i8/i16 does not narrow any memory
/// object. It instead inserts truncation/sign-extension pairs around the phi
/// and every dependent arithmetic operation, which is strictly more work in
/// the hot loop and can obstruct LLVM's induction-variable simplification.
/// Tainting SSA dependents also covers the aliases introduced by ARC range
/// lowering while leaving unrelated bounded locals eligible for narrowing.
pub(super) fn loop_carried_narrowing_exclusions(
    func: &ArcFunction,
) -> rustc_hash::FxHashSet<ArcVarId> {
    let dominators = ori_arc::graph::DominatorTree::build(func);
    let predecessors = ori_arc::graph::compute_predecessors(func);
    let mut excluded = rustc_hash::FxHashSet::default();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let is_loop_header = predecessors[block_idx]
            .iter()
            .any(|&pred_idx| dominators.dominates(block.id, func.blocks[pred_idx].id));
        if is_loop_header {
            excluded.extend(block.params.iter().map(|&(var, _)| var));
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for instruction in func.blocks.iter().flat_map(|block| &block.body) {
            let Some(dst) = instruction.defined_var() else {
                continue;
            };
            if instruction
                .used_vars()
                .iter()
                .any(|source| excluded.contains(source))
            {
                changed |= excluded.insert(dst);
            }
        }
    }

    excluded
}
