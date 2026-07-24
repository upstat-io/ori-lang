//! Protocol-builtin `Apply` in a generic function body — AOT regression pins.
//!
//! A protocol builtin (`__cast`/`__index`/`__iter_next`/`__collect_set`)
//! emitted inside a generic-`<T>`-bodied function is intercepted at codegen
//! (`ProtocolBuiltin::from_name` + per-variant inline emission); it never
//! reaches the user-function mono-dispatch path, so it cannot surface an
//! E5001 "unresolved function in apply — missing mono instance?". Each fixture
//! is a generic body exercising one protocol builtin; `assert_aot_success`
//! also leak-checks (exit 2 = leak).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "consistency with sibling AOT fixture modules"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

/// Regression: `int as float` cast inside a generic `<T>` body. The `__cast`
/// protocol-builtin Apply resolves via codegen interception, no E5001.
#[test]
fn test_cast_int_to_float_in_generic() {
    assert_aot_success(
        include_str!("fixtures/builtin_apply/cast_int_to_float_in_generic.ori"),
        "cast_int_to_float_in_generic",
    );
}

/// Regression: `float as int` cast inside a generic `<T>` body. The `__cast`
/// protocol-builtin Apply resolves via codegen interception, no E5001.
#[test]
fn test_cast_float_to_int_in_generic() {
    assert_aot_success(
        include_str!("fixtures/builtin_apply/cast_float_to_int_in_generic.ori"),
        "cast_float_to_int_in_generic",
    );
}

/// Regression: `int as byte` cast inside a generic `<T>` body. The `__cast`
/// protocol-builtin Apply resolves via codegen interception, no E5001.
#[test]
fn test_cast_int_to_byte_in_generic() {
    assert_aot_success(
        include_str!("fixtures/builtin_apply/cast_int_to_byte_in_generic.ori"),
        "cast_int_to_byte_in_generic",
    );
}

/// Regression: a `str.len()`-derived `as int` cast inside a generic `<T>`
/// body resolves its protocol-builtin `__cast` Apply mono via codegen
/// interception, no E5001.
#[test]
fn test_cast_str_len_in_generic() {
    assert_aot_success(
        include_str!("fixtures/builtin_apply/cast_str_len_in_generic.ori"),
        "cast_str_len_in_generic",
    );
}

/// Regression: `xs[0]` list indexing inside a generic `<T>` body. The
/// `__index` protocol-builtin Apply resolves via codegen interception, no
/// E5001.
#[test]
fn test_index_list_in_generic() {
    assert_aot_success(
        include_str!("fixtures/builtin_apply/index_list_in_generic.ori"),
        "index_list_in_generic",
    );
}

/// Regression: an `.iter().fold(...)` chain inside a generic `<T>` body. The
/// `__iter_next` protocol-builtin Apply resolves via codegen interception, no
/// E5001.
#[test]
fn test_iter_next_in_generic() {
    assert_aot_success(
        include_str!("fixtures/builtin_apply/iter_next_in_generic.ori"),
        "iter_next_in_generic",
    );
}

/// Regression: an `.iter().collect()` into `Set<int>` inside a generic `<T>`
/// body. The `__collect_set` protocol-builtin Apply resolves via codegen
/// interception, no E5001.
#[test]
fn test_collect_set_in_generic() {
    assert_aot_success(
        include_str!("fixtures/builtin_apply/collect_set_in_generic.ori"),
        "collect_set_in_generic",
    );
}

/// Negative pin: a genuinely invalid protocol-builtin use in a generic `<T>`
/// body — indexing a non-indexable `int` (`__index`) — is rejected at TYPE
/// CHECK with E2025, NOT at codegen as an E5001 "unresolved function __index
/// in apply" crash. Clamps that the builtin-Apply codegen-interception path
/// never masks a real type error: invalid programs fail early (E2xxx), valid
/// ones build clean (the seven positive pins above).
#[test]
fn test_index_non_indexable_in_generic_rejects_at_typeck_not_codegen() {
    let source = include_str!("fixtures/builtin_apply/index_non_indexable_in_generic_invalid.ori");
    let (exit_code, _stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(
        exit_code, -1,
        "expected compile failure, got exit {exit_code}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("E2025"),
        "expected typeck E2025 (non-indexable), got:\n{stderr}"
    );
    assert!(
        !stderr.contains("E5001"),
        "must reject at typeck, NOT crash at codegen with E5001:\n{stderr}"
    );
}
