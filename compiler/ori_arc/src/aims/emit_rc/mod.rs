//! RC emission helpers for the unified realization pipeline.
//!
//! Contains helper functions, submodules, and re-exports used by `realize/`
//! for RC operations. RC emission is driven by `realize_rc_reuse()`.
//!
//! # Submodules
//!
//! - [`coalesce`] — static RC coalescing peephole pass
//! - [`cow`] — COW annotation helpers
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
mod coalesce;
pub mod cow;
pub mod drop_hints;
pub(crate) mod queries;
pub(crate) mod unwind_cleanup;

use crate::ir::ArcBlockId;

// Re-export for cow/drop_hints that import via `super::collect_rc_incremented_vars`.
pub(crate) use queries::{collect_param_borrowed_vars, collect_rc_incremented_vars};

// Re-exports for `realize/` unified annotation walk.
pub(crate) use cow::{has_borrows_from_aggregate, is_borrow_disjoint_from_siblings};
pub(crate) use drop_hints::{collect_borrowed_call_args, is_collection_var};

pub(crate) use coalesce::coalesce_block_rc;

/// Convert a `usize` block index to `ArcBlockId`.
#[inline]
pub(crate) fn block_id(idx: usize) -> ArcBlockId {
    ArcBlockId::new(
        u32::try_from(idx).unwrap_or_else(|_| panic!("block index {idx} exceeds u32::MAX")),
    )
}
