//! COW annotation helpers for AIMS state map.
//!
//! Contains helper functions used by `realize/` for COW annotation decisions.
//! The legacy `compute_aims_cow_annotations()` entry point has been removed —
//! COW annotations are now computed by `realize_annotations()` (Section 10).

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{
    AccessClass, AimsState, BorrowSource, Cardinality, Consumption, Uniqueness,
};
use crate::ir::ArcVarId;

/// Check if a receiver's borrow is disjoint from all sibling borrows.
///
/// For the optimization to apply, ALL of:
/// 1. The receiver has `BorrowSource::Exact { source, field: Some(f) }`
/// 2. The source variable is `Unique` (or qualifies for COW-aware borrowing)
/// 3. ALL other borrows from the same source have field info (`Some(g)`)
///    where `g != f` — i.e., they borrow different fields
///
/// If any sibling borrow targets the same field or has `None` (whole-object
/// borrow), the optimization is unsound and we return `false`.
pub(crate) fn is_borrow_disjoint_from_siblings(
    state_map: &AimsStateMap,
    receiver: ArcVarId,
) -> bool {
    let Some(&BorrowSource::Exact {
        source,
        field: Some(receiver_field),
    }) = state_map.borrow_source(receiver)
    else {
        return false;
    };

    // Check that the source is uniquely owned.
    let source_state = state_map.var_state_at_block_entry(super::block_id(0), source);
    if source_state.uniqueness != Uniqueness::Unique && !is_cow_aware_unique(&source_state) {
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

/// Check if a variable's combined state qualifies for COW-aware borrowing.
///
/// The combined state `(Owned, Linear, Once)` proves unique ownership from
/// cross-dimensional reasoning:
/// - `Owned`: callee received ownership transfer (one reference)
/// - `Linear`: callee consumes the value exactly once (no duplication/RcInc)
/// - `Once`: single use point (no additional references created)
///
/// Together, these guarantee that at the COW mutation point, the callee has
/// the only reference it was given and has not created additional ones. This
/// allows `StaticUnique` even when `uniqueness` is `MaybeShared` (conservative
/// backward default for parameters).
///
/// Note: this optimization requires the caller to pass `ArgOwnership::Owned`
/// (not borrowed). The `access == Owned` check confirms the callee's contract
/// demands ownership.
fn is_cow_aware_unique(state: &AimsState) -> bool {
    state.access == AccessClass::Owned
        && state.consumption == Consumption::Linear
        && state.cardinality == Cardinality::Once
}
