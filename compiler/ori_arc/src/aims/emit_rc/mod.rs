//! RC emission helpers for the unified realization pipeline.
//!
//! Contains helper functions, submodules, and re-exports used by `realize/`
//! for RC operations. RC emission is driven by `realize_rc_reuse()`.
//!
//! # Submodules
//!
//! - [`borrowed_defs`] — borrowed-definition collection for RC emission
//! - [`coalesce`] — static RC coalescing peephole pass
//! - [`cow`] — COW annotation helpers
//! - [`dec_suppression`] — Apply-aliased RC-dec suppression predicates
//! - [`drop_hints`] — drop hint helpers
//! - [`queries`] — post-emission RC-incremented variable tracking
//! - [`unwind_cleanup`] — Invoke-terminator unwind cleanup
//!
//! # References
//!
//! - Perceus (Reinking et al., PLDI 2021): dup/drop = contraction/weakening
//! - Backward liveness-driven RC insertion with last-use optimization
//!   (counting-immutable-beans technique)

pub mod arg_ownership;
pub(crate) mod borrowed_defs;
mod coalesce;
pub mod cow;
mod dec_suppression;
pub mod drop_hints;
pub(crate) mod queries;
pub(crate) mod unwind_cleanup;

use crate::ir::{ArcBlockId, ArcFunction, ArcVarId, RcStrategy};

// Re-export for cow/drop_hints that import via `super::collect_rc_incremented_vars`.
pub(crate) use queries::{collect_param_borrowed_vars, collect_rc_incremented_vars};

// Re-exports for `realize/` unified annotation walk.
pub(crate) use cow::{has_borrows_from_aggregate, is_borrow_disjoint_from_siblings};
pub(crate) use drop_hints::{collect_borrowed_call_args, is_collection_var};

// Re-exports for `realize/` unified forward walk.
pub(crate) use borrowed_defs::collect_iter_element_defs;
pub(crate) use coalesce::coalesce_block_rc;

/// Compute `RcStrategy` for a variable, returning `None` for scalars.
///
/// Visible to sibling submodules via `super::rc_strategy`, and to
/// `realize/` via `pub(crate)` re-export.
#[inline]
pub(crate) fn rc_strategy(
    func: &ArcFunction,
    var: ArcVarId,
    pool: &ori_types::Pool,
) -> Option<RcStrategy> {
    use crate::ir::ValueRepr;
    let repr = func.var_reprs[var.index()];
    if repr == ValueRepr::Scalar {
        return None;
    }
    Some(RcStrategy::from_repr(
        repr,
        pool,
        func.var_types[var.index()],
    ))
}

/// Convert a `usize` block index to `ArcBlockId`.
#[inline]
pub(crate) fn block_id(idx: usize) -> ArcBlockId {
    ArcBlockId::new(
        u32::try_from(idx).unwrap_or_else(|_| panic!("block index {idx} exceeds u32::MAX")),
    )
}

/// Same-allocation union-find representative per var (`Let{Var}` + apply
/// Direct/Conditional; EXCLUDES Jump-arg phi).
///
/// Thin projection over the §1.9 unified alias-table construction
/// (`project_aliases::compute_genuine_same_alloc_reps`) — ONE builder behind
/// both this query and the table's over-approximation classification; no
/// parallel tracker. Spec: Annex E §AIMS.
pub(crate) fn compute_same_alloc_reps(
    func: &crate::ir::ArcFunction,
    apply_result_aliases: &rustc_hash::FxHashMap<
        crate::ir::ArcVarId,
        crate::aims::intraprocedural::state_map::ApplyAliasSource,
    >,
) -> rustc_hash::FxHashMap<crate::ir::ArcVarId, crate::ir::ArcVarId> {
    crate::aims::intraprocedural::project_aliases::compute_genuine_same_alloc_reps(
        func,
        apply_result_aliases,
    )
}
