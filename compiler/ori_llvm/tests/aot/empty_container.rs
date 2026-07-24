//! AOT tests for empty-container correctness.
//!
//! Pins the runtime half of the empty-container fix: a program built around
//! the original empty-list repro compiles via the full AOT pipeline and exits
//! with code 0. The typeck half lives in
//! `compiler/ori_types/src/check/bodies/tests.rs` — see
//! `empty_list_with_push_and_len_typechecks_without_errors_end_to_end`.
//!
//! Dual-execution parity between the interpreter and AOT paths is verified
//! separately by `diagnostics/dual-exec-verify.sh` against the same repro
//! program (empty-container typeck phase-contract repro).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Annotated empty list with `push` + `len` compiles clean and exits 0
/// through the AOT backend.
///
/// Confirms the intended fix path works: when the user supplies
/// `let ages: [int] = []`, inference resolves the element type at the
/// declaration site (no reliance on push's constraint or end-of-body
/// defaulting) and codegen sees a fully-resolved `[int]` payload.
///
/// Pairs with
/// `empty_list_with_push_and_len_typechecks_without_errors_end_to_end` in
/// `ori_types/src/check/bodies/tests.rs` — that test pins the typeck
/// behavior; this one pins the runtime behavior.
#[test]
fn annotated_empty_list_with_push_and_len_compiles_and_exits_zero() {
    let src = r#"
        @main () -> int = {
            let ages: [int] = [];
            ages = ages.push(value: 10);
            if ages.len() == 1 then 0 else 1
        }
    "#;
    assert_aot_success(src, "annotated_empty_list_with_push_and_len");
}

/// Unannotated form — the original surface repro — now compiles
/// clean and exits 0 via AOT.
///
/// After the end-of-body defaulting pre-pass + push's element-type constraint
/// via unification, the unannotated `let ages = []` resolves to `[int]` before
/// typeck exit. Pinning this as a separate AOT test guards against a future
/// regression where either inference narrows differently or the defaulting pass
/// changes scope — the original repro MUST keep working.
#[test]
fn unannotated_empty_list_with_push_and_len_compiles_and_exits_zero() {
    let src = r#"
        @main () -> int = {
            let ages = [];
            ages = ages.push(value: 10);
            if ages.len() == 1 then 0 else 1
        }
    "#;
    assert_aot_success(src, "unannotated_empty_list_with_push_and_len");
}
