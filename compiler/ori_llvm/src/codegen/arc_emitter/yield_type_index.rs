//! Yield-allocation element-type indexing for function emission.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;
use rustc_hash::FxHashMap;

/// Index typed yield facts by their shared element-size operand.
pub(super) fn index_yield_types_by_elem_size_var(
    func: &ArcFunction,
) -> FxHashMap<ArcVarId, (Idx, Idx)> {
    func.yield_allocations
        .iter()
        .map(|fact| {
            (
                fact.elem_size_var,
                (func.var_type(fact.result), fact.elem_ty),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
