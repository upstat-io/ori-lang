//! Cross-module imported-generic monomorphization AOT matrix — four type
//! cells (int, str, [int], struct) covering `ori build` routing through
//! `import_sig_by_name` in `collect_mono_functions` plus the merged-pool
//! re-interning consumer in the multi-file build pipeline.
//!
//! Each matrix test pins Ok behavior AND a regression guard: reverting the
//! `import_sig_by_name` lookup branch re-surfaces `E5001 unresolved
//! function` at LLVM verification.
//! `test_unimported_generic_still_fails_cleanly` pins the inverse: a generic
//! absent from `import_sigs` fails with a clean type-check diagnostic — not
//! a crash, not a silent miscompilation.
//! Complements the JIT-path spec test
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
/// (`{i64, i64, ptr}`, 24 bytes, passed indirectly as it exceeds the
/// 16-byte direct-passing threshold). Any
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
/// Pins `first(identity([10, 20, 30])) == 10` through imported-mono dispatch
/// and the regression guard: reverting the `import_sig_by_name` lookup
/// re-surfaces `E5001` for both functions. The `[int]` element pins the
/// 24-byte `{ len, cap, data }` fat-pointer layout through the imported
/// generic body (indirect passing above the 16-byte direct threshold).
#[test]
#[ignore = "BUG-04-230: AOT multi-file build emits duplicate _ori_Error_ctor across translation units (linker multiple definition); prelude-derived Error ctor lacks emit-once/linkonce_odr"]
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
/// where `Point` is defined in the host module.
///
/// Pins field projection through the mono'd `identity` plus a 2-element
/// `pair` result; reverting the `import_sig_by_name` lookup produces `E5001`,
/// and reverting merged-pool re-interning produces `Tag::Var` codegen errors
/// (`Point`'s `Idx` not re-interned). `Point` (`{ x: int, y: int }`) is 16
/// bytes — exactly at the direct/indirect ABI passing boundary.
#[test]
#[ignore = "BUG-04-230: AOT multi-file build emits duplicate _ori_Error_ctor across translation units (linker multiple definition); prelude-derived Error ctor lacks emit-once/linkonce_odr"]
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

/// A generic NOT in `import_sigs` (the host omits the `use` statement) MUST
/// fail at type-check with a clean unresolved-identifier diagnostic — not a
/// crash, not a silent miscompilation.
///
/// Proves the `import_sig_by_name` lookup is the boundary: without the
/// import the failure is diagnostic; with it (the matrix tests) the mono
/// instance is registered and codegen succeeds. Asserts a non-zero exit,
/// non-empty stderr, and a structural unresolved-name diagnostic.
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
