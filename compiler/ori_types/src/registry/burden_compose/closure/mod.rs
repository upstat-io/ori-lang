//! Closure capture composition — `UserBurdenSpec` population at
//! closure-type-registration time.
//!
//! Spec: Annex E §AIMS — captured environments introduce logical ownership
//! identities whose burdens are composed at the lambda-expression type-check
//! site. The composed `UserBurdenSpec` carries:
//!
//! - `self_owned_identity: true` — the closure value receives a conservative
//!   logical callable identity; no storage class or header is implied.
//! - `owned_fields[i]` — one entry per captured-by-value capture (`Idx` = the
//!   captured binding's resolved type Idx).
//! - `borrowed_fields[i]` — one entry per captured-by-reference capture (per
//!   `Tag::Borrowed`-target lifetime tie-back to the parent variable).
//! - `element_burden: None` — closures are not collections.
//! - `variant_burdens: []` — closures are not sums.
//! - `drop_operation` — a stable logical cleanup identity; physical
//!   projections may discharge it, inline it, or map it to a helper.
//! - `user_drop: None` — closures cannot have user `@drop` (only types do per
//!   `drop-trait-proposal.md §Auto-derive`).
//!
//! `field_path` indices in `UserOwnedField` / `UserBorrowedField` start at 0
//! (logical capture position). A VM or compiled layout plan maps those paths to
//! its own storage; no physical offset enters this spec.

use ori_registry::burden::FnSym;

use super::scc::mint_drop_operation_sym;
use crate::registry::burden::{
    UserBorrowedField, UserBurdenSpec, UserOwnedField, UserVariantBurden,
};
use crate::Idx;

/// A single capture descriptor used to compose a closure's `UserBurdenSpec`.
///
/// `field_index` is the logical capture position (0-based) — corresponds to the
/// caller's argument order at the `PartialApply` site. `field_type` is the
/// resolved `Idx` of the captured binding's type.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ClosureCapture {
    pub field_index: u32,
    pub field_type: Idx,
}

/// Compose a `UserBurdenSpec` for a closure type given its captured bindings.
///
/// Inputs:
/// - `closure_idx` — the resolved `Idx` of the synthesized closure type. The
///   minted `drop_operation` identity keys on this `Idx` via
///   `mint_drop_operation_sym`.
/// - `owned_captures` — captured-by-value captures. One `UserOwnedField` per
///   entry; `field_path: vec![capture.field_index]`.
/// - `borrowed_captures` — captured-by-reference captures. One
///   `UserBorrowedField` per entry; same `field_path` convention.
///
/// Output: `UserBurdenSpec` populated as:
/// - `self_owned_identity: true`
/// - `owned_fields` / `borrowed_fields` mapped from inputs
/// - `element_burden: None`, `variant_burdens: []`
/// - `drop_operation: Some(mint_drop_operation_sym(closure_idx))`
/// - `user_drop: None`
///
/// The composed spec is then registered via
/// `TypeRegistry::register_user_burden(closure_idx, spec)` at the lambda's
/// type-check site (`ori_types::infer::expr::infer_lambda` per
/// `compiler/ori_types/src/infer/expr/blocks.rs`).
#[must_use]
pub fn compose_closure_burden_spec(
    closure_idx: Idx,
    owned_captures: &[ClosureCapture],
    borrowed_captures: &[ClosureCapture],
) -> UserBurdenSpec {
    UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: owned_captures
            .iter()
            .map(|c| UserOwnedField {
                field_path: vec![c.field_index],
                field_type: c.field_type,
            })
            .collect(),
        borrowed_fields: borrowed_captures
            .iter()
            .map(|c| UserBorrowedField {
                field_path: vec![c.field_index],
                field_type: c.field_type,
            })
            .collect(),
        variant_burdens: Vec::<UserVariantBurden>::new(),
        element_burden: None,
        drop_operation: Some(mint_closure_drop_operation(closure_idx)),
        user_drop: None,
    }
}

/// Mint a deterministic logical cleanup identity for a captured environment.
///
/// Reuses `scc::mint_drop_operation_sym` so recursive types and closures share
/// one stable identity space. Helper naming and body emission are projection
/// decisions. Distinct pool `Idx` values receive distinct identities even when
/// their capture shapes are structurally identical.
#[must_use]
pub fn mint_closure_drop_operation(closure_idx: Idx) -> FnSym {
    mint_drop_operation_sym(closure_idx)
}

#[cfg(test)]
mod tests;
