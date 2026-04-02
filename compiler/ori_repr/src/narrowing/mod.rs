//! Narrowing pipeline — integer and float narrowing passes.
//!
//! Narrows semantic types to smaller machine representations when the
//! compiler can prove no loss of precision or correctness:
//! - `int` (i64) → i8/i16/i32 via value range analysis
//! - `float` (f64) → f32 via precision analysis
//!
//! # Architecture
//!
//! The narrowing passes read from `ReprPlan` (ranges, field summaries)
//! and write back narrowed `MachineRepr` into the plan.
//!
//! Five modules:
//! - **`abi`**: ABI boundary classification and widening policy
//! - **`int`**: Integer struct/tuple field narrowing
//! - **`float`**: Float precision analysis and field narrowing
//! - **`overflow`** (future): Overflow guard insertion
//!
//! Narrowing phases:
//! - **Phase A**: Struct field narrowing via field-summary ranges/precision
//! - **Phase B**: Local variable narrowing + overflow guards
//! - **Phase C**: Collection element narrowing

use ori_types::{Idx, Pool};

use crate::plan::{DecisionReason, DecisionSource, ReprDecision, ReprPlan};
use crate::repr::MachineRepr;

pub mod abi;
pub mod float;
pub mod int;
pub mod overflow;

#[cfg(test)]
mod tests;

/// Store a narrowing decision and propagate to the resolved Pool index.
///
/// Many narrowing passes need to store a decision for both the original
/// type index and its resolved counterpart (e.g., `Named("Pixel")` resolves
/// to `Struct(fields)`). Without propagation, codegen queries the resolved
/// index and finds the original canonical representation.
pub(crate) fn commit_narrowing_decision(
    plan: &mut ReprPlan,
    pool: &Pool,
    idx: Idx,
    source: DecisionSource,
    repr: MachineRepr,
    reason: DecisionReason,
) {
    let decision = ReprDecision {
        source,
        type_idx: idx,
        repr: repr.clone(),
        reason: reason.clone(),
    };
    plan.set_repr(idx, decision);
    let resolved = pool.resolve_fully(idx);
    if resolved != idx {
        plan.set_repr(
            resolved,
            ReprDecision {
                source,
                type_idx: resolved,
                repr,
                reason,
            },
        );
    }
}

/// Check if a type is eligible for field narrowing.
///
/// Types with fixed-layout attributes (`#repr("c")`, `#repr("packed")`,
/// `#repr("transparent")`) and public types (ABI contract) are not
/// narrowing candidates.
pub(crate) fn is_narrowing_candidate(plan: &ReprPlan, idx: Idx) -> bool {
    !has_fixed_layout_attr(plan, idx) && !plan.is_public_type(idx)
}

/// Check if a type has a fixed-layout attribute that prevents narrowing.
///
/// `#repr("c")`, `#repr("c", aligned N)`, `#repr("packed")`, and
/// `#repr("transparent")` all fix the field layout — narrowing would
/// violate the ABI contract. `#repr("aligned", N)` alone does NOT
/// prevent narrowing: it sets whole-struct alignment, not field layout.
pub(crate) fn has_fixed_layout_attr(plan: &ReprPlan, idx: Idx) -> bool {
    plan.repr_attr(idx).is_some_and(|attr| {
        matches!(
            attr,
            crate::plan::ReprAttribute::C
                | crate::plan::ReprAttribute::CAligned(_)
                | crate::plan::ReprAttribute::Packed
                | crate::plan::ReprAttribute::Transparent
        )
    })
}
