//! COW annotation helpers for AIMS state map.
//!
//! Contains helper functions used by `realize/` for COW annotation decisions.
//! The legacy `compute_aims_cow_annotations()` entry point has been removed —
//! COW annotations are now computed by `realize_annotations()` (Section 10).

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{BorrowSource, Uniqueness};
use crate::ir::{ArcBlockId, ArcVarId};

#[cfg(test)]
mod tests;

/// Check if a receiver's borrow is disjoint from all sibling borrows.
///
/// Spec §DP-5 + §RL-10 (local field-disjoint mutation) require that the
/// source is provably uniquely owned at the receiver's program point AND
/// the receiver's borrow field is disjoint from every sibling borrow.
/// Source uniqueness is established SOLELY by the Uniqueness dimension —
/// the former cross-dimensional path using `is_cow_aware_unique`
/// (`Owned + Linear + Once`) was removed as unsound per §DP-10 removal
/// rationale (derived past uniqueness from future consumption, which
/// cannot prove RC == 1 at the present program point). §RL-31 is the
/// related interprocedural rule for `noalias` metadata on disjoint
/// borrowed parameters; this helper implements the local mutation case.
///
/// For the optimization to apply, ALL of:
/// 1. The receiver has `BorrowSource::Exact { source, field: Some(f) }`
/// 2. The source variable has `Uniqueness::Unique`
/// 3. ALL other borrows from the same source have field info (`Some(g)`)
///    where `g != f` — i.e., they borrow different fields
///
/// If any sibling borrow targets the same field or has `None` (whole-object
/// borrow), the optimization is unsound and we return `false`.
pub(crate) fn is_borrow_disjoint_from_siblings(
    state_map: &AimsStateMap,
    receiver: ArcVarId,
    block: ArcBlockId,
) -> bool {
    let Some(&BorrowSource::Exact {
        source,
        field: Some(receiver_field),
    }) = state_map.borrow_source(receiver)
    else {
        return false;
    };

    // Source uniqueness must be proved by the Uniqueness dimension itself
    // AT THE RECEIVER'S ACTUAL PROGRAM POINT — not at function entry. A
    // source may be Unique at entry but become MaybeShared later via
    // RcInc; using the entry state would incorrectly promote a MaybeShared
    // receiver to StaticUnique (pre-existing bug TPR-04-001-codex-phase5,
    // exposed by BUG-04-059 removal of the `is_cow_aware_unique` fallback).
    let source_state = state_map.var_state_at_block_entry(block, source);
    if source_state.uniqueness != Uniqueness::Unique {
        return false;
    }

    // Check all sibling borrows from the same source for field disjointness.
    for (borrow_var, borrow_field) in state_map.borrows_from_source(source) {
        if borrow_var == receiver {
            continue; // skip self
        }
        match borrow_field {
            // Same field → aliasing, not safe.
            Some(f) if f == receiver_field => return false,
            // Whole-object borrow (no field) → conservatively unsafe.
            None => return false,
            // Different field → disjoint, safe.
            Some(_) => {}
        }
    }

    true
}
