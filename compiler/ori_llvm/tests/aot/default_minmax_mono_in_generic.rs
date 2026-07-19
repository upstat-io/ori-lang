//! Derived `Default` + prelude `min`/`max` monomorphization in a generic body —
//! AOT regression pins.
//!
//! `min`/`max` are prelude free functions (`min(left:, right:)` /
//! `max(left:, right:)`), monomorphized through the standard generic-function
//! dispatch path; `#derive(Default)` synthesizes a `Type.default()` associated
//! function and, for default parameter values, a `_ori_<fn>_default` helper.
//! Each fixture exercises one of those surfaces inside a generic-`<T>` body and
//! must build + run clean under AOT; `assert_aot_success` also leak-checks
//! (exit 2 = leak).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "consistency with sibling AOT fixture modules"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

/// Regression: `min`/`max` on `int` inside a generic `<T>` body monomorphize
/// via the standard generic-function dispatch, no missing-mono instance.
#[test]
fn test_min_max_int_in_generic() {
    assert_aot_success(
        include_str!("fixtures/default_minmax_mono/min_max_int_in_generic.ori"),
        "min_max_int_in_generic",
    );
}

/// Regression: `min`/`max` on `float` inside a generic `<T>` body monomorphize
/// cleanly; the float comparison path emits no missing-mono instance.
#[test]
fn test_min_max_float_in_generic() {
    assert_aot_success(
        include_str!("fixtures/default_minmax_mono/min_max_float_in_generic.ori"),
        "min_max_float_in_generic",
    );
}

/// Regression: `min`/`max` on `str` (a heap value) inside a generic `<T>` body
/// monomorphize cleanly and leave RC balanced (no leak / double-free).
#[test]
fn test_min_max_str_in_generic() {
    assert_aot_success(
        include_str!("fixtures/default_minmax_mono/min_max_str_in_generic.ori"),
        "min_max_str_in_generic",
    );
}

/// Regression: nested `min(min(..), ..)` / `max(max(..), ..)` inside a generic
/// `<T>` body — chained mono of the same builtin resolves at every call site.
#[test]
fn test_nested_min_max_in_generic() {
    assert_aot_success(
        include_str!("fixtures/default_minmax_mono/nested_min_max_in_generic.ori"),
        "nested_min_max_in_generic",
    );
}

/// Regression: a `#derive(Default)` struct built via `Type.default()` inside a
/// generic `<T>` body and threaded through a second generic `<T>` consumer —
/// the synthesized `_ori_<Type>$A$default` associated fn monomorphizes clean.
#[test]
fn test_default_struct_in_generic() {
    assert_aot_success(
        include_str!("fixtures/default_minmax_mono/default_struct_in_generic.ori"),
        "default_struct_in_generic",
    );
}

/// Regression: a `#derive(Default)` nested struct (struct-with-struct-field)
/// default-constructed — locks the `insert_value` / `const_zero` aggregate
/// layout for the nested `StructValue` so the verifier accepts the aggregate.
#[test]
fn test_default_nested_struct() {
    assert_aot_success(
        include_str!("fixtures/default_minmax_mono/default_nested_struct.ori"),
        "default_nested_struct",
    );
}

/// Regression: a default-parameter-value call (`@scaled (n, factor: int = 10)`)
/// exercised with and without the default so the synthesized `_ori_<fn>_default`
/// helper is emitted AND called under AOT — the arg-count-correct helper passes
/// the LLVM verifier.
#[test]
fn test_default_param_helper() {
    assert_aot_success(
        include_str!("fixtures/default_minmax_mono/default_param_helper.ori"),
        "default_param_helper",
    );
}

/// Negative pin: `#derive(Default)` on a struct with a non-`Default` field
/// (a sum type) is rejected at TYPE CHECK with E2032, NOT at codegen as an
/// `insert_value`-on-non-struct / missing-mono / verifier crash. Clamps that
/// the Default-derive path fails early (E2xxx) on invalid input and never
/// threads a malformed aggregate into LLVM emission.
#[test]
fn test_default_non_default_field_rejects_at_typeck_not_codegen() {
    let source = include_str!("fixtures/default_minmax_mono/default_non_default_field_invalid.ori");
    let (exit_code, _stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(
        exit_code, -1,
        "expected compile failure, got exit {exit_code}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("E2032"),
        "expected typeck E2032 (non-Default field), got:\n{stderr}"
    );
    assert!(
        !stderr.contains("E5001"),
        "must reject at typeck, NOT crash at codegen with E5001:\n{stderr}"
    );
}
