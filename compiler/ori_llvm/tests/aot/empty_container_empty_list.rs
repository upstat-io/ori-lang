//! AOT tests for empty-container matrix coverage (Section 05.3).
//!
//! Pairs with the spec tests in
//! `compiler_repo/tests/spec/types/collections/empty_list/` — this module
//! exercises the same matrix cells via the full AOT pipeline. Distinct
//! from `empty_container.rs` which owns the §03.5 original-repro fix.
//!
//! Dual-execution parity is verified separately by
//! `diagnostics/dual-exec-verify.sh` (plan §05.3.4).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Annotated empty `[bool]` with `push` + `is_empty` compiles clean and
/// exits 0 through the AOT backend.
///
/// Matrix cell: `B=bool` × `C=push+is_empty`. After push, `is_empty()` must
/// be false; a non-zero exit would indicate either an inference regression
/// (element type unresolved) or a codegen regression on the `[bool]`
/// narrowed-element path.
#[test]
fn aot_empty_list_bool_push_is_empty_compiles_and_exits_zero() {
    let src = r#"
        @main () -> int = {
            let xs: [bool] = [];
            xs = xs.push(value: true);
            if xs.is_empty() then 1 else 0
        }
    "#;
    assert_aot_success(src, "aot_empty_list_bool_push_is_empty");
}

/// Annotated empty `[str]` with `push` + `iter().count()` compiles clean
/// and exits 0 through the AOT backend.
///
/// Matrix cell: `B=str` × `C=push+iter`. Exercises the iterator pipeline over
/// an element type that requires full fat-pointer element stride (24 bytes),
/// ensuring the narrowed-pointer sext widening path is NOT engaged for
/// non-integer elements and that iterator drop cleans up remaining `str`
/// values.
#[test]
fn aot_empty_list_str_push_iter_compiles_and_exits_zero() {
    let src = r#"
        @main () -> int = {
            let xs: [str] = [];
            xs = xs.push(value: "a");
            let $count = xs.iter().count();
            if count == 1 then 0 else 1
        }
    "#;
    assert_aot_success(src, "aot_empty_list_str_push_iter");
}

/// Annotated empty `[Option<int>]` with `push` + `len` compiles clean and
/// exits 0 through the AOT backend.
///
/// Matrix cell: `B=Option<int>` × `C=push+len`. Exercises a sum-type payload
/// element. Tests that niche/tagged representation of `Option<int>` is
/// preserved through the empty-list inference path and element storage.
#[test]
fn aot_empty_list_option_push_compiles_and_exits_zero() {
    let src = r#"
        @main () -> int = {
            let xs: [Option<int>] = [];
            xs = xs.push(value: Some(42));
            if xs.len() == 1 then 0 else 1
        }
    "#;
    assert_aot_success(src, "aot_empty_list_option_push");
}

/// Annotated empty `[[int]]` with `push` + `len` compiles clean and exits
/// 0 through the AOT backend.
///
/// Matrix cell: `C=nested`. Exercises nested-container RC: the outer list
/// owns inner `[int]` values, each carrying its own RC header. On drop,
/// the outer `elem_dec_fn` must release each inner list's backing buffer.
/// A leak here would trip `ORI_CHECK_LEAKS=1` in `assert_aot_success`.
#[test]
fn aot_empty_list_nested_annotated_compiles_and_exits_zero() {
    let src = r#"
        @main () -> int = {
            let xs: [[int]] = [];
            xs = xs.push(value: [1, 2, 3]);
            if xs.len() == 1 then 0 else 1
        }
    "#;
    assert_aot_success(src, "aot_empty_list_nested_annotated");
}
