//! AOT tests for §07.2 enum representation optimization.
//!
//! Verifies that tagless (single-variant) and niche-encoded enums
//! produce correct behavior through the full AOT pipeline.

use crate::util::compile_and_run_capture;

/// Tagless single-variant enum flows through full AOT pipeline.
///
/// Covers: `canonical_enum` → `EnumTag::None` → `resolve_enum_tagless` →
/// LLVM named struct with payload only → construction → match → output.
///
/// Blocked: codegen consumers (`variant_construction`, `instr_dispatch`,
/// `drop_enum`) still hardcode tag access via GEP index 0. Once
/// `TagEncoding` is wired in (repr-opt §07.2 Phase B/C), remove `#[ignore]`.
#[test]
#[ignore = "blocked by codegen consumer migration (repr-opt §07.2 Phase B/C)"]
fn test_tagless_single_variant_enum_aot() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/repr/tagless_single_variant.ori"));
    assert_eq!(exit_code, 0, "tagless enum AOT failed: {stderr}");
    assert_eq!(stdout.trim(), "42");
}
