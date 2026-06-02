//! Polymorphic-Lambda + Imported-Generic AOT Tests
//!
//! Cross-module imported generic monomorphization matrix —
//! covers the four type cells (int, str, [int], struct) for `ori build`
//! routing through `import_sig_by_name` in `collect_mono_functions`
//! plus the merged-pool re-interning consumer in
//! `multi.rs::compile_single_module`.
//!
//! ## Pre-fix failure mode
//!
//! Before merged-pool re-interning + the imported-mono carrier promotion,
//! `ori build` failed with `E5001 unresolved function '<name>' in
//! apply/invoke — missing mono instance` at LLVM verification because
//! `collect_mono_functions` silently skipped imported generic instances
//! (their `fn_name` was not in the host module's `function_sigs` /
//! `impl_sigs` tables; `import_sigs` was not yet threaded through).
//!
//! Before merged-pool re-interning, the additional failure mode for the polymorphic-lambda
//! tests (`test_poly_lambda_with_imported_assert_eq_*`) layered on top:
//! `ori_llvm::codegen::type_info::store::get_impl()` emitted "unresolved
//! type variable at codegen" when the poly-lambda return-position
//! `Tag::Var(Unbound)` leaked past typeck's PC-2 validator and reached
//! codegen still carrying `Tag::Var(Generalized)` — sibling pass
//! `default_unbound_vars_from_polylambda_returns` cures that leak before
//! `validate_body_types` runs.
//!
//! ## Post-fix shape
//!
//! `collect_mono_functions` consults `import_sig_by_name` for top-level
//! call sites whose `instance.receiver_type.is_none()`; the merged-pool
//! re-interning consumer assembles an `imported_mono_fns: Vec<ImportedMonoFn>`
//! from retained per-module `TypeCheckResult.imported` metadata + per-module
//! `CanonResult` slots; `arc_lowering` specializes each imported mono locally
//! against the SOURCE module's `CanonResult` via `lower_to_arc(mangled_name,
//! sig, source_body_name, source_canon, ...)`. The body-import path
//! emits the body in the host module through `declare_mono_functions` —
//! no `declare_imported_mono` ghost exists.
//!
//! ## Asserted invariant
//!
//! Each test asserts both Ok behavior (correct result via assertions) AND
//! a regression guard: reverting the `import_sig_by_name` lookup branch in `collect_mono_functions`
//! re-surfaces `E5001 unresolved function`, failing `assert_aot_success`
//! with exit code `-1`. The matrix is the regression guard.
//!
//! ## Clean-failure mode
//!
//! `test_unimported_generic_still_fails_cleanly` confirms the failure
//! mode for a generic not present in `import_sigs` is a CLEAN diagnostic
//! (compile-time type error) — not a silent miscompilation, not a crash.
//!
//! These AOT tests complement the JIT-path spec test at
//! `tests/spec/expressions/poly_lambda_with_imported_generic.ori`.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{
    assert_aot_success, assert_multifile_aot_success, compile_multifile_and_run_capture,
};

// Polymorphic-lambda matrix — single-file host with stdlib `assert_eq`
// (existing cases). Imported generic = `assert_eq<T>` from `std.testing`.

/// Polymorphic identity lambda defined in the host module, imported
/// `assert_eq<int>` called with the lambda's monomorphized result. The
/// `@main` wrapper returns 0 on success; `assert_eq` panics on mismatch,
/// producing a non-zero exit status. Compilation failure →
/// `assert_aot_success` fails.
#[test]
fn test_poly_lambda_with_imported_assert_eq_int() {
    assert_aot_success(
        include_str!("fixtures/poly_lambda_mono/poly_lambda_with_imported_assert_eq_int.ori"),
        "poly_lambda_imported_assert_eq_int",
    );
}

/// Same shape as the int variant but pinned at `str` — fat-pointer ABI
/// (`{i64, i64, ptr}`, 24 bytes, indirect passing per `CG:AB-1`). Any
/// fix that regresses RC management on the assertion-failure message
/// concat/`debug()` path surfaces here rather than in the int cell.
#[test]
fn test_poly_lambda_with_imported_assert_eq_str() {
    assert_aot_success(
        include_str!("fixtures/poly_lambda_mono/poly_lambda_with_imported_assert_eq_str.ori"),
        "poly_lambda_imported_assert_eq_str",
    );
}

// Imported-generic matrix — multi-file host calling a custom generic from
// `fixtures/imported_generics/generics.ori`. Exercises the merged-pool
// re-interning consumer in `multi.rs::compile_single_module` end-to-end.

const IMPORTED_GENERICS_HELPER: &str = include_str!("fixtures/imported_generics/generics.ori");

/// `[int]` matrix cell — imported `identity<[int]>` + `first<int>` from a
/// custom helper module.
///
/// Pins:
/// - Ok behavior: `first(identity([10, 20, 30])) == 10` (passes when mono
///   dispatch succeeds through the `import_sig_by_name` lookup).
/// - Regression guard: reverting the `import_sig_by_name` lookup branch re-surfaces `E5001 unresolved function`
///   for `identity` AND `first` — `assert_multifile_aot_success` panics
///   with the captured stderr.
///
/// The `[int]` element type pins fat-pointer ABI (`CG:TR-4`) through the
/// imported generic body, exercising the indirect-passing path per
/// `CG:AB-1`.
#[test]
fn test_imported_generic_fn_list_int() {
    assert_multifile_aot_success(
        &[
            (
                "host.ori",
                include_str!("fixtures/imported_generics/host_list_int.ori"),
            ),
            ("generics.ori", IMPORTED_GENERICS_HELPER),
        ],
        "imported_generic_fn_list_int",
    );
}

/// User-struct matrix cell — imported `identity<Point>` + `pair<Point>`
/// from a custom helper module where `Point` is defined in the host.
///
/// Pins:
/// - Ok behavior: struct-field projection through the mono'd `identity`
///   returns the original field values; `pair` returns a 2-element list
///   with both points intact.
/// - Regression guard: reverting the `import_sig_by_name` lookup branch produces `E5001` for both `identity`
///   AND `pair`; reverting the merged-pool re-interning produces
///   `Tag::Var` codegen errors because `Point`'s `Idx` is not re-interned
///   into the merged pool.
///
/// `Point` (`{ x: int, y: int }`) is 16 bytes — sits at the
/// direct/indirect ABI boundary per `CG:AB-1`, threading user-type Idx
/// through the imported generic body. The struct dimension distinguishes
/// scalar / fat-pointer / struct ABI cases without requiring stdlib
/// involvement.
#[test]
fn test_imported_generic_fn_struct() {
    assert_multifile_aot_success(
        &[
            (
                "host.ori",
                include_str!("fixtures/imported_generics/host_struct.ori"),
            ),
            ("generics.ori", IMPORTED_GENERICS_HELPER),
        ],
        "imported_generic_fn_struct",
    );
}

// Clean-failure mode — generic referenced without import produces a CLEAN
// diagnostic, not a silent miscompilation or crash.

/// A generic that is NOT in `import_sigs` (because the host omits the
/// `use` statement) MUST fail at type-check with a clean unresolved
/// identifier diagnostic — NOT a silent miscompilation, NOT a crash.
///
/// This is the clean-failure regression guard that proves the `import_sig_by_name`
/// lookup is the boundary: when the boundary is not crossed (no import),
/// the failure mode is diagnostic. When it IS crossed (import present),
/// the mono instance is registered and codegen succeeds. The asymmetry
/// is the spec; this test asserts one side, the matrix tests assert
/// the other.
///
/// Asserts:
/// - Compilation fails (exit code `-1` from `compile_multifile_and_run_capture`).
/// - Stderr is non-empty (a diagnostic was emitted).
/// - The diagnostic is structural (an unresolved-name / undefined-function
///   error), not a panic / segfault / silent zero-exit.
#[test]
fn test_unimported_generic_still_fails_cleanly() {
    // Host module references `identity` and `first` without importing them
    // from `./generics`. The helper module IS present in the temp dir but
    // is not in the host's `use` set — so the type checker's import
    // resolution never sees them, `import_sigs` does not include them,
    // and the type checker raises an unresolved identifier diagnostic.
    let host = r#"
use std.testing { assert_eq }

@main () -> int = {
    let $xs = [10, 20, 30];
    let $copy = identity(x: xs);
    assert_eq(actual: first(items: copy), expected: 10);
    0
}
"#;

    let (exit_code, stdout, stderr) = compile_multifile_and_run_capture(&[
        ("host.ori", host),
        ("generics.ori", IMPORTED_GENERICS_HELPER),
    ]);

    // Compilation MUST fail (-1 = ori build returned non-zero). A zero
    // exit here would mean a silent miscompilation — production codegen
    // emitted a binary referencing a function it never declared, and
    // either the binary segfaulted at runtime or produced garbage. Either
    // is worse than the diagnostic path.
    assert_eq!(
        exit_code, -1,
        "unimported generic compiled cleanly (silent miscompilation); \
         stdout: {stdout}; stderr: {stderr}"
    );

    // Stderr MUST carry a diagnostic. An empty stderr with -1 exit
    // suggests the compiler crashed without emitting a message — also
    // worse than the diagnostic path.
    assert!(
        !stderr.is_empty(),
        "compilation failed but no diagnostic was emitted; \
         compile-time silent failure is worse than runtime miscompilation"
    );

    // Stderr MUST describe an unresolved identifier (or similar
    // structural failure). The exact error code is checked loosely
    // because the type checker's exact wording may evolve — the
    // load-bearing fact is the failure is DIAGNOSTIC vs CRASH.
    let stderr_lower = stderr.to_lowercase();
    assert!(
        stderr_lower.contains("identity")
            || stderr_lower.contains("first")
            || stderr_lower.contains("unresolved")
            || stderr_lower.contains("undefined")
            || stderr_lower.contains("not found")
            || stderr_lower.contains("e2"),
        "diagnostic did not mention the unresolved generic; got: {stderr}"
    );
}
