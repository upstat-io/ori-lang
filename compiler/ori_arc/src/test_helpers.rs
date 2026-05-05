//! Shared test utilities for ARC analysis passes.
//!
//! Consolidates factory functions used across `borrow`, `liveness`, `aims`,
//! and pipeline tests. Only compiled in test builds.

use ori_ir::{Name, Span};
use ori_types::{Idx, Pool, Tag};

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcParam, ArcTerminator, ArcVarId};
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
        var_rc_strategies: Vec::new(),
        spans: span_vecs,
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Build a minimal empty `ArcBlock` with no params, empty body, and
/// `Unreachable` terminator. `id = ArcBlockId::new(n)`.
///
/// Used by validator tests that need a block skeleton but no actual
/// control flow (§04.4 Test Fixture Strategy — the validator walks
/// `blocks[*].params[*].1` type positions, not the body or terminator).
pub(crate) fn minimal_block(n: u32) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(n),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Unreachable,
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

/// Allocate a `Tag::Var` with the caller-specified pool var id.
///
/// Ensures pool var-state capacity covers `var_id`, then interns
/// `Tag::Var(var_id)`. Distinct from `Pool::fresh_var()` which
/// auto-increments the next available id.
///
/// Used by validator tests that need deterministic pool var ids for
/// exemption-set contrast (e.g., §04.4 Test Fixture Strategy — primary-seam
/// empty-exempt pin needs `Tag::Var(1)` to contrast against a synthetic
/// `scheme_var_ids = [1, 2, 3]` secondary-site exempt set).
pub(crate) fn allocate_pool_var_with_id(pool: &mut Pool, var_id: u32) -> Idx {
    pool.ensure_var_capacity(var_id + 1);
    pool.intern(Tag::Var, var_id)
}
