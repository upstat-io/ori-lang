//! Polymorphic-Lambda + Imported-Generic AOT Tests
//!
//! TDD matrix for `plans/empty-container-typeck-phase-contract §08.2`,
//! absorbing BUG-04-042. Every test in this file is expected to FAIL before
//! §08.3 lands and PASS after. The failure mode is compile-time:
//! `ori_llvm::codegen::type_info::store::get_impl()` emits "unresolved type
//! variable at codegen" when a polymorphic-lambda return position leaks past
//! typeck's PC-2 validator and reaches codegen still carrying
//! `Tag::Var(Generalized)`.
//!
//! Root cause (per §08.1.R Hypothesis (d)):
//! `default_unbound_vars_from_empty_literals` in
//! `compiler/ori_types/src/infer/mod.rs` is scope-by-empty-literal, NOT
//! scope-by-lambda-return. Poly-lambda returns whose `Tag::Var(Unbound)`
//! never sees a concrete-type constraint generalize to
//! `Tag::Var(Generalized)`, which `validate_body_types` exempts per
//! `types.md §SC-1` shipped divergence (load-bearing for let-polymorphism).
//! The surviving `Tag::Var` reaches `TypeInfoStore::get_impl()` where
//! `pool.resolve_fully()` cannot chase non-`Link` `VarStates`.
//!
//! Fix (target §08.3): sibling pass
//! `default_unbound_vars_from_polylambda_returns` that defaults poly-lambda
//! return-position `Unbound` vars to `Idx::NEVER` before
//! `validate_body_types` runs — preserving the `Generalized`/`Rigid`
//! exemptions that let-polymorphism relies on.
//!
//! These AOT tests complement the JIT-path spec test at
//! `tests/spec/expressions/poly_lambda_with_imported_generic.ori` — the
//! pillar-5 deliverable per the parent plan's overview requires BOTH paths
//! green before §08 closes.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Polymorphic identity lambda defined in the host module, imported
/// `assert_eq<int>` called with the lambda's monomorphized result.
///
/// This is the exact shape of the BUG-04-042 failing case. The `@main`
/// wrapper returns 0 on success; `assert_eq` panics on mismatch, producing
/// a non-zero exit status. Compilation failure → `assert_aot_success` fails.
#[test]
#[ignore = "BUG-04-AOT-MONO"]
fn test_poly_lambda_with_imported_assert_eq_int() {
    assert_aot_success(
        include_str!("fixtures/poly_lambda_mono/poly_lambda_with_imported_assert_eq_int.ori"),
        "poly_lambda_imported_assert_eq_int",
    );
}

/// Same shape as the int variant but pinned at `str` — fat-pointer ABI
/// (`{i64, i64, ptr}`, 24 bytes, indirect passing per `CG:AB-1`). Any
/// §08.3 fix that regresses RC management on the assertion-failure message
/// concat/`debug()` path surfaces here rather than in the int cell.
#[test]
#[ignore = "BUG-04-AOT-MONO"]
fn test_poly_lambda_with_imported_assert_eq_str() {
    assert_aot_success(
        include_str!("fixtures/poly_lambda_mono/poly_lambda_with_imported_assert_eq_str.ori"),
        "poly_lambda_imported_assert_eq_str",
    );
}
