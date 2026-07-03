//! COW annotation helpers for AIMS state map.
//!
//! Contains helper functions used by `realize/` for COW annotation decisions.

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{BorrowSource, Uniqueness};
use crate::ir::{ArcBlockId, ArcVarId};

#[cfg(test)]
mod tests;

/// Check if any borrow from the given aggregate exists anywhere in the function.
///
/// Spec DP-5 requires `no_active_overlapping_borrows` for in-place mutation.
/// This is a FUNCTION-WIDE conservative check: if ANY `BorrowSource::Exact`
/// entry has `source == aggregate`, return `true`. No block or instruction
/// liveness parameter — maximally conservative to avoid the block-entry
/// liveness unsoundness (backward analysis removes variables at definition,
/// making mid-block borrows invisible at block entry).
pub(crate) fn has_borrows_from_aggregate(state_map: &AimsStateMap, aggregate: ArcVarId) -> bool {
    state_map.borrows_from_source(aggregate).next().is_some()
}

/// Check if a receiver's borrow is disjoint from all sibling borrows.
///
/// Spec §DP-5 + §RL-10 (local field-disjoint mutation) require that the
/// source is provably uniquely owned at the receiver's program point AND
/// the receiver's borrow field is disjoint from every sibling borrow.
/// Source uniqueness is established SOLELY by the Uniqueness dimension.
/// §RL-31 is the related interprocedural rule for `noalias` metadata on
/// disjoint borrowed parameters; this helper implements the local mutation case.
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
    // receiver to StaticUnique.
    //
    // Query `effective_uniqueness_at_block_entry` (NOT raw
    // lattice uniqueness) so a contract-derived MaybeShared from a callee's
    // ReturnContract reaches this gate. The JOIN semantics preserve any
    // backward-demand widening the lattice already captured.
    if state_map.effective_uniqueness_at_block_entry(block, source) != Uniqueness::Unique {
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
