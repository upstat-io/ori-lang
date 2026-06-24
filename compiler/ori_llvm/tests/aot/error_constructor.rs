//! The builtin `Error` struct constructor lowers to a direct `Construct`
//! (not `PartialApply @Error` + `ApplyIndirect`), so `Error(msg)` constructs
//! and runs under AOT instead of calling through a null fn ptr (SIGSEGV).
//! Covers L8 (AOT) + L10 (leak) via `assert_aot_success`.

use crate::util::assert_aot_success;

/// Construct `Error` across every call position the typeck `(str) -> Error`
/// function-typing produces: direct let-bind, inline function-arg, inline
/// tail-return, and inline-into-`Err`. Pre-fix every inline position SIGSEGVs
/// (exit 139) at the unresolved `@Error` partial-apply.
#[test]
fn test_error_constructor_all_positions() {
    assert_aot_success(
        include_str!("fixtures/error_constructor/all_positions.ori"),
        "error_constructor_all_positions",
    );
}

/// `Error` bound as a first-class function VALUE (`let f = Error`) then called
/// indirectly — the bare reference lowers to `PartialApply @Error`; the
/// synthesized `@Error` constructor function resolves it (pre-fix: exit 139).
#[test]
fn test_error_constructor_first_class_value() {
    assert_aot_success(
        include_str!("fixtures/error_constructor/first_class_value.ori"),
        "error_constructor_first_class_value",
    );
}
