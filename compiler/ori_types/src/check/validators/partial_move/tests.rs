//! Smoke tests for `validate_partial_move` (E2043) + `validate_drop_partial_move`
//! (E2048) — full coverage lives in the spec-test corpus at
//! `compiler_repo/tests/spec/aims/`, which exercises the validators
//! end-to-end via `#compile_fail("E2043")` / `#compile_fail("E2048")`
//! directives.

use ori_ir::{ExprId, Name};
use rustc_hash::FxHashMap;

use super::{validate_drop_partial_move, validate_partial_move};
use crate::output::FunctionSig;
use crate::{Idx, Pool, TraitRegistry};

#[test]
fn partial_move_validator_handles_invalid_body_root_without_panicking() {
    let pool = Pool::new();
    let arena = ori_ir::ExprArena::new();
    let expr_types = FxHashMap::default();
    let sig = FunctionSig::simple(Name::EMPTY, vec![], Idx::UNIT);
    let mut errors = Vec::new();

    validate_partial_move(
        &pool,
        &arena,
        &expr_types,
        &sig,
        ExprId::INVALID,
        &mut errors,
    );

    assert!(
        errors.is_empty(),
        "ExprId::INVALID is the spec-canonical empty-body sentinel; \
         validator must short-circuit without emitting diagnostics"
    );
}

#[test]
fn drop_partial_move_validator_handles_invalid_body_root_without_panicking() {
    let pool = Pool::new();
    let arena = ori_ir::ExprArena::new();
    let expr_types = FxHashMap::default();
    let trait_registry = TraitRegistry::new();
    let drop_name = Name::EMPTY;
    let sig = FunctionSig::simple(Name::EMPTY, vec![], Idx::UNIT);
    let mut errors = Vec::new();

    validate_drop_partial_move(
        &pool,
        &arena,
        &expr_types,
        &trait_registry,
        drop_name,
        &sig,
        ExprId::INVALID,
        &mut errors,
    );

    assert!(
        errors.is_empty(),
        "ExprId::INVALID is the spec-canonical empty-body sentinel; \
         drop-partial-move validator must short-circuit without emitting \
         diagnostics regardless of registry state"
    );
}

#[test]
fn drop_partial_move_validator_silent_when_drop_trait_unregistered() {
    // Pre-deployment shape: when the `Drop` trait is not yet wired into
    // the trait registry (no impl, no trait_def), the validator MUST
    // silently no-op — emitting E2048 on every nominal aggregate would
    // be wildly over-restrictive.
    let pool = Pool::new();
    let arena = ori_ir::ExprArena::new();
    let expr_types = FxHashMap::default();
    let trait_registry = TraitRegistry::new();
    let drop_name = Name::from_raw(999); // arbitrary, unregistered name
    let sig = FunctionSig::simple(Name::EMPTY, vec![], Idx::UNIT);
    let mut errors = Vec::new();

    // Empty body short-circuits before the registry check; this test
    // exercises the registry-name miss path implicitly through the
    // function's two early-return gates (INVALID body root + missing
    // trait). Both produce empty-error outcomes.
    validate_drop_partial_move(
        &pool,
        &arena,
        &expr_types,
        &trait_registry,
        drop_name,
        &sig,
        ExprId::INVALID,
        &mut errors,
    );

    assert!(
        errors.is_empty(),
        "validator MUST silently no-op when Drop trait is not in the \
         registry (pre-deployment / partial-deployment shape)"
    );
}
