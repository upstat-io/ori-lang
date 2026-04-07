//! AOT tests for zero-sized enum payload mismatch.
//!
//! Verifies that enum variants with void/unit payloads are correctly
//! handled in LLVM codegen — void fields should contribute 0 bytes
//! to enum payload size, not 8 bytes (the i64 placeholder).

use crate::util::{compile_and_run, compile_and_run_capture, compile_to_llvm_ir};

/// Exact failing case from `A(u: void) | B` should compile and run.
#[test]
fn test_void_payload_enum_compiles() {
    let exit_code = compile_and_run(include_str!(
        "fixtures/enum_zero_payload/void_payload_enum_compiles.ori"
    ));
    assert_eq!(exit_code, 0, "void-payload enum should compile and run");
}

/// Match on void-payload variant should produce correct result.
#[test]
fn test_void_payload_enum_match() {
    let (exit_code, stdout, _) = compile_and_run_capture(include_str!(
        "fixtures/enum_zero_payload/void_payload_enum_match.ori"
    ));
    assert_eq!(exit_code, 0, "compilation should succeed");
    assert_eq!(
        stdout.trim(),
        "a\nb",
        "match should produce correct results"
    );
}

/// Mixed enum: real payload alongside void payload.
#[test]
fn test_mixed_payload_with_void() {
    let (exit_code, stdout, _) = compile_and_run_capture(
        r#"
type Mixed = Real(x: int) | Empty(u: void) | Plain;

@describe (m: Mixed) -> str = match m {
    Real(x:) -> `real({x})`,
    Empty(u:) -> "empty",
    Plain -> "plain",
}

@main () -> void = {
    print(msg: describe(m: Real(x: 42)));
    print(msg: describe(m: Empty(u: ())));
    print(msg: describe(m: Plain));
}
"#,
    );
    assert_eq!(exit_code, 0, "compilation should succeed");
    assert_eq!(
        stdout.trim(),
        "real(42)\nempty\nplain",
        "mixed enum with void payload should work correctly"
    );
}

/// Semantic pin: void-payload enum should have the same LLVM layout
/// as an all-unit enum (tag only, no payload array).
#[test]
fn test_void_payload_enum_layout_is_tag_only() {
    let ir = compile_to_llvm_ir(include_str!(
        "fixtures/enum_zero_payload/void_payload_enum_layout_is_tag_only.ori"
    ))
    .expect("compilation should succeed");

    // Void-payload enum should be tag-only `{ i8 }`, same as all-unit enum.
    // If void fields incorrectly contribute 8 bytes, the layout would be
    // `{ i8, [1 x i64] }` — which is wrong.
    assert!(
        ir.contains("type { i8 }"),
        "Void-payload enum should use tag-only layout {{ i8 }}, not {{ i8, [1 x i64] }}.\n\
         IR types:\n{}",
        ir.lines()
            .filter(|l: &&str| l.contains("WithVoid") || l.contains("type {"))
            .take(10)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// All variants with void payloads should be equivalent to all-unit enum.
#[test]
fn test_all_void_payload_variants() {
    let exit_code = compile_and_run(include_str!(
        "fixtures/enum_zero_payload/all_void_payload_variants.ori"
    ));
    assert_eq!(exit_code, 0, "all-void-payload enum should work correctly");
}

// Derived traits (Eq, Comparable, Hashable) on enums with
// zero-sized payload fields crash in LLVM codegen because the derive
// paths count void fields as occupied i64 slots, drifting offsets.

/// Exact repro from derived Eq on enum with void field.
#[test]
fn test_derive_eq_void_payload() {
    let (exit_code, stdout, _) = compile_and_run_capture(include_str!(
        "fixtures/enum_zero_payload/derive_eq_void_payload.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "derive Eq on void-payload enum should compile"
    );
    assert_eq!(
        stdout.trim(),
        "eq\nne\nne\neq",
        "derive Eq should compare non-void fields correctly"
    );
}

/// Derived Eq on all-void-payload enum (tag-only layout, no payload array).
#[test]
fn test_derive_eq_all_void_payload() {
    let (exit_code, stdout, _) = compile_and_run_capture(include_str!(
        "fixtures/enum_zero_payload/derive_eq_all_void_payload.ori"
    ));
    assert_eq!(exit_code, 0, "derive Eq on all-void enum should compile");
    assert_eq!(
        stdout.trim(),
        "eq\nne\nne\neq",
        "all-void Eq should compare by tag only"
    );
}

/// Derived Comparable on enum with void field.
#[test]
fn test_derive_comparable_void_payload() {
    let (exit_code, stdout, _) = compile_and_run_capture(include_str!(
        "fixtures/enum_zero_payload/derive_comparable_void_payload.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "derive Comparable on void-payload enum should compile"
    );
    assert_eq!(
        stdout.trim(),
        "lt\nge\neq",
        "Comparable should compare non-void fields correctly"
    );
}

/// Derived Hashable on enum with void field.
#[test]
fn test_derive_hashable_void_payload() {
    let (exit_code, stdout, _) = compile_and_run_capture(include_str!(
        "fixtures/enum_zero_payload/derive_hashable_void_payload.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "derive Hashable on void-payload enum should compile"
    );
    assert_eq!(
        stdout.trim(),
        "same\ndiff\ndiff",
        "Hashable should hash non-void fields only"
    );
}

/// Mixed: void field between non-void fields — offset must be correct.
#[test]
fn test_derive_eq_void_between_fields() {
    let (exit_code, stdout, _) = compile_and_run_capture(include_str!(
        "fixtures/enum_zero_payload/derive_eq_void_between_fields.ori"
    ));
    assert_eq!(exit_code, 0, "void between non-void fields should compile");
    assert_eq!(
        stdout.trim(),
        "eq\nne",
        "Eq should skip void gap and compare both int fields"
    );
}

/// Semantic pin: derived Eq on void-payload enum must behave
/// identically to an all-unit enum.
#[test]
fn test_derive_eq_semantic_pin_void_vs_unit() {
    let (exit_code, stdout, _) = compile_and_run_capture(include_str!(
        "fixtures/enum_zero_payload/derive_eq_semantic_pin_void_vs_unit.ori"
    ));
    assert_eq!(exit_code, 0);
    assert_eq!(
        stdout.trim(),
        "eq\nne\neq\nne",
        "void-payload Eq must match all-unit Eq behavior"
    );
}
