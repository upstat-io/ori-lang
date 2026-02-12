//! Shared test utilities for ARC analysis passes.
//!
//! Consolidates factory functions used across `borrow`, `liveness`,
//! `rc_insert`, `rc_elim`, `reset_reuse`, `expand_reuse`, and pipeline
//! tests. Only compiled in test builds.

use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcVarId};
use crate::ownership::Ownership;

/// Shorthand for `ArcVarId::new(n)`.
pub(crate) fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Shorthand for `ArcBlockId::new(n)`.
pub(crate) fn b(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

/// Build a minimal `ArcFunction` with a default name (`Name::from_raw(1)`).
pub(crate) fn make_func(
    params: Vec<ArcParam>,
    return_type: Idx,
    blocks: Vec<ArcBlock>,
    var_types: Vec<Idx>,
) -> ArcFunction {
    make_func_named(Name::from_raw(1), params, return_type, blocks, var_types)
}

/// Build a minimal `ArcFunction` with an explicit name.
///
/// Used by borrow inference tests that need distinct names for
/// multi-function analysis.
pub(crate) fn make_func_named(
    name: Name,
    params: Vec<ArcParam>,
    return_type: Idx,
    blocks: Vec<ArcBlock>,
    var_types: Vec<Idx>,
) -> ArcFunction {
    let span_vecs: Vec<Vec<Option<Span>>> =
        blocks.iter().map(|bl| vec![None; bl.body.len()]).collect();
    ArcFunction {
        name,
        params,
        return_type,
        blocks,
        entry: ArcBlockId::new(0),
        var_types,
        spans: span_vecs,
    }
}

/// Create an owned parameter.
pub(crate) fn owned_param(var: u32, ty: Idx) -> ArcParam {
    ArcParam {
        var: ArcVarId::new(var),
        ty,
        ownership: Ownership::Owned,
    }
}

/// Create a borrowed parameter.
pub(crate) fn borrowed_param(var: u32, ty: Idx) -> ArcParam {
    ArcParam {
        var: ArcVarId::new(var),
        ty,
        ownership: Ownership::Borrowed,
    }
}

/// Count total RC ops (`RcInc` + `RcDec`) across the entire function.
pub(crate) fn count_rc_ops(func: &ArcFunction) -> usize {
    func.blocks
        .iter()
        .flat_map(|bl| bl.body.iter())
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .count()
}

/// Count total RC ops (`RcInc` + `RcDec`) in a single block.
pub(crate) fn count_block_rc_ops(func: &ArcFunction, block_idx: usize) -> usize {
    func.blocks[block_idx]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .count()
}

/// Count `RcInc` for a specific var in a block.
pub(crate) fn count_inc(func: &ArcFunction, block_idx: usize, var: ArcVarId) -> usize {
    func.blocks[block_idx]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::RcInc { var: v, .. } if *v == var))
        .count()
}

/// Count `RcDec` for a specific var in a block.
pub(crate) fn count_dec(func: &ArcFunction, block_idx: usize, var: ArcVarId) -> usize {
    func.blocks[block_idx]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::RcDec { var: v } if *v == var))
        .count()
}
