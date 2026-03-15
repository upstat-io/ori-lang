//! Shared test utilities for ARC analysis passes.
//!
//! Consolidates factory functions used across `borrow`, `liveness`, `aims`,
//! and pipeline tests. Only compiled in test builds.

use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcParam, ArcVarId};
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
        var_reprs: Vec::new(),
        spans: span_vecs,
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
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
