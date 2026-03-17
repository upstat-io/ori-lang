//! RC emission helpers for the unified realization pipeline.
//!
//! Contains helper functions, submodules, and re-exports used by `realize/`
//! (Section 10) for RC operations. The legacy `emit_rc_ops()` entry point
//! has been removed — RC emission is now driven by `realize_rc_reuse()`.
//!
//! # Submodules
//!
//! - [`arg_ownership`] — Apply/Invoke ownership propagation
//! - [`cow`] — COW annotation helpers
//! - [`drop_hints`] — drop hint helpers
//! - [`coalesce`] — adjacent RC op merging
//! - [`dead_cleanup`] — dead-at-entry/invoke-dst cleanup
//! - [`edge_cleanup`] — inter-block edge RC decrements
//! - [`forward_walk`] — terminator RC emission
//! - [`helpers`] — block context, use precomputation, liveness queries
//! - [`queries`] — RC state queries (incremented vars)
//!
//! # References
//!
//! - Perceus (Reinking et al., PLDI 2021): dup/drop = contraction/weakening
//! - Lean 4 `RC.lean`: backward liveness-driven insertion with last-use opt

pub mod arg_ownership;
mod coalesce;
pub mod cow;
mod dead_cleanup;
pub mod drop_hints;
mod edge_cleanup;
mod forward_walk;
mod helpers;
mod queries;

use crate::ir::{ArcBlockId, ArcFunction, ArcVarId, RcStrategy};

/// Edge-specific RC decrement: variable + strategy.
pub(crate) type EdgeDec = (ArcVarId, RcStrategy);

// Re-export for cow/drop_hints that import via `super::collect_rc_incremented_vars`.
pub(crate) use queries::{collect_param_borrowed_vars, collect_rc_incremented_vars};

// Re-exports for `realize/` unified annotation walk (Section 10.3).
pub(crate) use cow::is_borrow_disjoint_from_siblings;
pub(crate) use drop_hints::{collect_borrowed_call_args, is_collection_var};

// Re-exports for `realize/` unified forward walk (Section 10.2).
pub(crate) use coalesce::coalesce_block_rc;
pub(crate) use dead_cleanup::{emit_dead_at_entry_decs, emit_dead_invoke_dsts};
pub(crate) use edge_cleanup::emit_edge_cleanup;
pub(crate) use forward_walk::emit_terminator_rc;
pub(crate) use helpers::{
    collect_all_borrowed_defs, collect_borrowed_defs, collect_defined_vars,
    collect_project_borrowed_defs, compute_child_effective_last_use, is_consuming_primop,
    is_live_at_exit, is_owned_at_entry, is_ownership_transfer, precompute_block_uses, BlockCtx,
    LastUse,
};

/// Compute `RcStrategy` for a variable, returning `None` for scalars.
///
/// Visible to all sibling submodules (`edge_cleanup`, `dead_cleanup`,
/// `forward_walk`, `helpers`) via `super::rc_strategy`, and to
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
    Some(RcStrategy::from_var(
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
