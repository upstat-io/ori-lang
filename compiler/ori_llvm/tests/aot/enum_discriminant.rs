//! AOT tests for §07.1 discriminant narrowing.
//!
//! Verifies that enum tags are emitted as narrowed integer types in LLVM IR
//! (i8 instead of i64 for ≤256 variants) and that behavioral correctness
//! is preserved.

use crate::util::{assert_aot_success, compile_and_run_capture, compile_to_llvm_ir};

/// Semantic pin: all-unit enum LLVM type is `{ i8 }` (not `{ i64 }`).
#[test]
fn test_all_unit_enum_tag_is_i8() {
    let ir = compile_to_llvm_ir(include_str!(
        "fixtures/enum_discriminant/all_unit_enum_tag_is_i8.ori"
    ))
    .expect("compilation failed");

    // The Color enum type should contain i8 (narrowed tag), not i64
    assert!(
        ir.contains("type { i8 }"),
        "All-unit enum should use i8 tag, not i64. IR snippet:\n{}",
        ir.lines()
            .filter(|l: &&str| l.contains("Color") || l.contains("type {"))
            .take(5)
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Negative pin: must NOT contain i64 tag for this enum
    assert!(
        !ir.contains("%Color = type { i64 }"),
        "All-unit enum should NOT use i64 tag"
    );
}

/// Documents: Option/Result keep i64 tags for runtime (`ori_rt`) compatibility.
/// User-defined enums get i8 tags via `resolve_enum()`.
/// Narrowing Option/Result tags requires coordinated `ori_rt` update.
#[test]
fn test_option_keeps_i64_tag_for_runtime_compat() {
    let ir = compile_to_llvm_ir(include_str!(
        "fixtures/enum_discriminant/option_keeps_i64_tag_for_runtime_compat.ori"
    ))
    .expect("compilation failed");

    // Option<str> uses {i64, {i64, i64, ptr}} — i64 tag for runtime compatibility
    // The runtime (ori_list_first, ori_map_get, etc.) writes i64 tags to sret ptrs
    assert!(
        ir.contains("{ i64,"),
        "Option should use i64 tag for runtime compatibility. IR snippet:\n{}",
        ir.lines()
            .filter(|l: &&str| l.contains("sret") || l.contains("{ i64"))
            .take(5)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Behavioral correctness: all-unit enum match produces correct values.
#[test]
fn test_all_unit_enum_match_correctness() {
    assert_aot_success(
        include_str!("fixtures/enum_discriminant/all_unit_enum_match_correctness.ori"),
        "all_unit_enum_match",
    );
}

/// Behavioral correctness: Option match with narrowed tag.
#[test]
fn test_option_match_narrowed_tag() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/enum_discriminant/option_match_narrowed_tag.ori"
    ));
    assert_eq!(exit_code, 0, "option match failed: {stderr}");
    assert_eq!(stdout.trim(), "42,-1");
}

/// Behavioral correctness: Result match with narrowed tag.
#[test]
fn test_result_match_narrowed_tag() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/enum_discriminant/result_match_narrowed_tag.ori"
    ));
    assert_eq!(exit_code, 0, "result match failed: {stderr}");
    assert_eq!(stdout.trim(), "100,4");
}

/// Behavioral + leak check: enum with RC payload and narrowed tag.
#[test]
fn test_enum_rc_payload_narrowed_tag() {
    assert_aot_success(
        include_str!("fixtures/enum_discriminant/enum_rc_payload_narrowed_tag.ori"),
        "enum_rc_payload_narrowed",
    );
}
