//! AOT tests for enum representation optimization.
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
/// `TagEncoding` is wired in (niche/tagless codegen consumers), remove `#[ignore]`.
#[test]
#[ignore = "BUG-07-038: blocked by codegen consumer migration to niche/tagless TagEncoding"]
fn test_tagless_single_variant_enum_aot() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/repr/tagless_single_variant.ori"));
    assert_eq!(exit_code, 0, "tagless enum AOT failed: {stderr}");
    assert_eq!(stdout.trim(), "42");
}
