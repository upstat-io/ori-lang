//! Tests for [`super::super::TypeInfoStore`].
//!
//! §08.3b.1 Cell J — `Tag::BoundVar` at codegen is a defensive ICE signal.
//!
//! Per, scheme bodies carry `Tag::BoundVar(var_id)` leaves
//! post-generalization. Substitution to concrete types happens UPSTREAM in
//! `lower_function_can` via `type_subst` threaded from
//! `MonoFunction.body_type_map` (see `function_compiler/nounwind/prepare.rs:133`).
//! By codegen time, a correctly-substituted mono body has zero `Tag::BoundVar`
//! leaves. If one reaches `compute_type_info_inner`, an upstream layer
//! missed a leaf — the store surfaces this as `TypeInfo::Error` rather than
//! masking it, per AIMS Invariant 2 (no masking of upstream
//! incorrectness).
//!
//! Plan anchor: §08.3b.1 Cell J — ICE-signal contract.

use ori_types::{Pool, Tag};

use super::super::info::TypeInfo;
use super::super::TypeInfoStore;

/// Cell J — `Tag::BoundVar` reaching codegen MUST surface as
/// `TypeInfo::Error` (defensive ICE signal).
///
/// Per AIMS Invariant 2 — no masking of upstream
/// incorrectness. Substitution of scheme-body `Tag::BoundVar` leaves to
/// concrete types is an upstream responsibility (ARC lowering's
/// `type_subst` + typeck end-of-body normalization). When a `Tag::BoundVar`
/// reaches `compute_type_info_inner`, the upstream pipeline missed a leaf
/// and the store returns `TypeInfo::Error` to surface the bug, not a
/// fallback-to-int default.
///
/// Plan anchor: §08.3b.1 Cell J.
#[test]
fn type_info_store_bound_var_surfaces_as_error_ice_signal() {
    let mut pool = Pool::new();
    let bound_var = pool.bound_var(0);
    assert_eq!(
        pool.tag(bound_var),
        Tag::BoundVar,
        "pool setup: bound_var(0) must produce a Tag::BoundVar leaf"
    );

    let store = TypeInfoStore::new(&pool);
    let info = store.get(bound_var);
    assert!(
        matches!(info, TypeInfo::Error),
        " — Tag::BoundVar at codegen MUST \
         surface as TypeInfo::Error (defensive ICE signal; upstream \
         type_subst missed a leaf). Got {info:?}"
    );
}
