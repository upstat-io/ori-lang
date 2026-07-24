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
/// Tagless single-variant codegen is active. This cell pins its payload-only
/// construction, projection, match, and drop path independently from the
/// still-gated multi-variant niche encoding.
#[test]
fn test_tagless_single_variant_enum_aot() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/repr/tagless_single_variant.ori"));
    assert_eq!(exit_code, 0, "tagless enum AOT failed: {stderr}");
    assert_eq!(stdout.trim(), "42");
}
