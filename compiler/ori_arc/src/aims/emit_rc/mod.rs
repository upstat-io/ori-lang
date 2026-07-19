//! Transitional ownership-event carrier helpers for unified realization.
//!
//! This compatibility-named module materializes logical owner-credit and
//! cleanup obligations through the current ARC IR `RcInc`/`RcDec` spellings.
//! Those names do not select or require a physical reference-counter layout.
//! Realization is driven by `realize_rc_reuse()`.
//!
//! # Submodules
//!
//! - [`coalesce`] — current-carrier ownership-event coalescing peephole
//! - [`cow`] — COW annotation helpers
//! - [`drop_hints`] — drop hint helpers
//! - [`queries`] — post-emission owner-credit carrier tracking
//! - [`unwind_cleanup`] — Invoke-terminator unwind cleanup
//!
//! # References
//!
//! - Perceus (Reinking et al., PLDI 2021): historical dup/drop realization of
//!   contraction/weakening
//! - Historical backward-liveness RC insertion with last-use optimization
//!   (counting-immutable-beans technique); AIMS retains the logical event shape

pub mod arg_ownership;
mod coalesce;
pub mod cow;
pub mod drop_hints;
pub(crate) mod queries;
pub(crate) mod unwind_cleanup;

use crate::ir::ArcBlockId;

// Compatibility-named re-export for COW/drop-hint consumers of the current
// ownership-event carrier.
pub(crate) use queries::{collect_param_borrowed_vars, collect_rc_incremented_vars};

// Re-exports for `realize/` unified annotation walk.
pub(crate) use cow::{has_borrows_from_aggregate, is_borrow_disjoint_from_siblings};
pub(crate) use drop_hints::{collect_borrowed_call_args, is_collection_var};

// Compatibility-named current-carrier peephole.
pub(crate) use coalesce::coalesce_block_rc;

/// Convert a `usize` block index to `ArcBlockId`.
#[inline]
pub(crate) fn block_id(idx: usize) -> ArcBlockId {
    ArcBlockId::new(
        u32::try_from(idx).unwrap_or_else(|_| panic!("block index {idx} exceeds u32::MAX")),
    )
}
