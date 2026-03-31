//! AOT Derive Trait Codegen Tests
//!
//! End-to-end tests verifying that `#[derive(...)]` generates correct native code
//! through the LLVM backend. Each test compiles Ori source to a native binary,
//! runs it, and checks the exit code (0 = success).
//!
//! Covers roadmap Section 3.5: Derive Traits (Eq, Clone, Hashable, Printable).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use ori_ir::DerivedTrait;

use crate::util::assert_aot_success;

// --- Cross-crate sync enforcement (Section 05.1, Test 5) ---

#[test]
fn all_derived_traits_have_codegen() {
    // Known gaps — traits with documented reasons for missing LLVM codegen.
    // Adding a trait here requires a comment explaining why codegen is deferred.
    // Removing a trait means codegen was implemented — update the count below.
    let known_gaps: &[DerivedTrait] = &[];

    // Guard: pinned trait count forces this test to be reviewed when a new
    // DerivedTrait variant is added. Update this constant, then either
    // implement LLVM codegen or add the trait to known_gaps above.
    assert_eq!(
        DerivedTrait::COUNT,
        7,
        "DerivedTrait::COUNT changed! Update this test: either implement \
         LLVM codegen for the new trait or add it to known_gaps with a reason."
    );

    // Verify known_gaps entries are valid (no stale entries after variant removal)
    for gap in known_gaps {
        assert!(
            DerivedTrait::ALL.contains(gap),
            "Stale known_gap: {gap:?} is no longer in DerivedTrait::ALL"
        );
    }

    // Every trait in ALL must be either in known_gaps or expected to have codegen.
    // The pinned count above is the real guard; this documents intent.
    let should_have_codegen: Vec<_> = DerivedTrait::ALL
        .iter()
        .filter(|t| !known_gaps.contains(t))
        .collect();

    assert_eq!(
        should_have_codegen.len(),
        7,
        "Traits expected to have LLVM codegen changed: {should_have_codegen:?}. \
         Update this count after implementing codegen or adding to known_gaps."
    );
}

// 3.5.1: Derive Eq

#[test]
fn test_aot_derive_eq_basic() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_eq_basic.ori"),
        "derive_eq_basic",
    );
}

#[test]
fn test_aot_derive_eq_with_strings() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_eq_with_strings.ori"),
        "derive_eq_with_strings",
    );
}

#[test]
fn test_aot_derive_eq_mixed_types() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_eq_mixed_types.ori"),
        "derive_eq_mixed_types",
    );
}

#[test]
fn test_aot_derive_eq_single_field() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_eq_single_field.ori"),
        "derive_eq_single_field",
    );
}

// 3.5.2: Derive Clone

#[test]
fn test_aot_derive_clone_basic() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_clone_basic.ori"),
        "derive_clone_basic",
    );
}

#[test]
fn test_aot_derive_clone_large_struct() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_clone_large_struct.ori"),
        "derive_clone_large_struct",
    );
}

// 3.5.3: Derive Hashable

#[test]
fn test_aot_derive_hash_equal_values() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_hash_equal_values.ori"),
        "derive_hash_equal_values",
    );
}

#[test]
fn test_aot_derive_hash_different_values() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_hash_different_values.ori"),
        "derive_hash_different_values",
    );
}

// 3.5.4: Derive Printable

#[test]
fn test_aot_derive_printable_basic() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_printable_basic.ori"),
        "derive_printable_basic",
    );
}

// 3.5.5: Derive Default

#[test]
fn test_aot_derive_default_basic() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_default_basic.ori"),
        "derive_default_basic",
    );
}

#[test]
fn test_aot_derive_default_mixed_types() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_default_mixed_types.ori"),
        "derive_default_mixed_types",
    );
}

#[test]
fn test_aot_derive_default_eq_integration() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_default_eq_integration.ori"),
        "derive_default_eq_integration",
    );
}

#[test]
fn test_aot_derive_default_str_field() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_default_str_field.ori"),
        "derive_default_str_field",
    );
}

#[test]
fn test_aot_derive_default_nested() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_default_nested.ori"),
        "derive_default_nested",
    );
}

// 3.7: Clone trait on primitives (built-in identity clone)

#[test]
fn test_aot_clone_int() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_int.ori"),
        "clone_int",
    );
}

#[test]
fn test_aot_clone_float() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_float.ori"),
        "clone_float",
    );
}

#[test]
fn test_aot_clone_bool() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_bool.ori"),
        "clone_bool",
    );
}

#[test]
fn test_aot_clone_str() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_str.ori"),
        "clone_str",
    );
}

// 3.7: Clone on collections

#[test]
fn test_aot_clone_list_int() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_list_int.ori"),
        "clone_list_int",
    );
}

#[test]
fn test_aot_clone_list_empty() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_list_empty.ori"),
        "clone_list_empty",
    );
}

// 3.7: Clone on Option

#[test]
fn test_aot_clone_option_some() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_option_some.ori"),
        "clone_option_some",
    );
}

#[test]
fn test_aot_clone_option_none() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_option_none.ori"),
        "clone_option_none",
    );
}

// 3.7: Clone on Result

#[test]
fn test_aot_clone_result_ok() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_result_ok.ori"),
        "clone_result_ok",
    );
}

#[test]
fn test_aot_clone_result_err() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_result_err.ori"),
        "clone_result_err",
    );
}

// 3.7: Clone on tuples

#[test]
fn test_aot_clone_tuple_pair() {
    // Tuple destructuring not yet implemented in AOT codegen,
    // so we verify clone compiles and the value is usable.
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_tuple_pair.ori"),
        "clone_tuple_pair",
    );
}

#[test]
fn test_aot_clone_tuple_triple() {
    // Tuple destructuring not yet implemented in AOT codegen,
    // so we verify clone compiles and the value is usable.
    assert_aot_success(
        include_str!("fixtures/derives/aot_clone_tuple_triple.ori"),
        "clone_tuple_triple",
    );
}

// 3.14: Derive Comparable

#[test]
fn test_aot_derive_comparable_basic() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_comparable_basic.ori"),
        "derive_comparable_basic",
    );
}

#[test]
fn test_aot_derive_comparable_first_field_wins() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_comparable_first_field_wins.ori"),
        "derive_comparable_first_field",
    );
}

#[test]
fn test_aot_derive_comparable_with_strings() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_comparable_with_strings.ori"),
        "derive_comparable_strings",
    );
}

#[test]
fn test_aot_derive_comparable_single_field() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_comparable_single_field.ori"),
        "derive_comparable_single_field",
    );
}

// 3.5.6: Multiple derives on one type

#[test]
fn test_aot_derive_multiple_traits() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_multiple_traits.ori"),
        "derive_multiple_traits",
    );
}

// =========================================================================
// 3.14: Derive hash edge cases (hygiene fixes)
// =========================================================================

// Derive Hashable with float fields: ±0.0 must produce same hash

#[test]
fn test_aot_derive_hash_float_neg_zero() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_hash_float_neg_zero.ori"),
        "derive_hash_float_neg_zero",
    );
}

// Derive Hashable with str fields: different strings must hash differently

#[test]
fn test_aot_derive_hash_str_content() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_hash_str_content.ori"),
        "derive_hash_str_content",
    );
}

// Derive Hashable with byte field: values ≥ 128 must use unsigned extension

#[test]
fn test_aot_derive_hash_byte_field() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_hash_byte_field.ori"),
        "derive_hash_byte_field",
    );
}

// =========================================================================
// C3: Derive Eq on payload sum types (code journey 11)
// =========================================================================

#[test]
fn test_aot_derive_eq_payload_sum_type() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_eq_payload_sum_type.ori"),
        "derive_eq_payload_sum",
    );
}

#[test]
fn test_aot_derive_eq_mixed_variant_comparison() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_eq_mixed_variant_comparison.ori"),
        "derive_eq_mixed_variant",
    );
}

#[test]
fn test_aot_derive_eq_single_payload_variant() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_eq_single_payload_variant.ori"),
        "derive_eq_single_payload",
    );
}

#[test]
fn test_aot_journey_11_derived_eq() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_journey_11_derived_eq.ori"),
        "journey_11_derived_eq",
    );
}

// ---- TPR-04-021: Debug derive str field leak fix ----
//
// The `emit_field_to_string` Debug/Str path creates `"\"" + val + "\""`
// via two concats. The inner concat result (`quoted`) must be RC-decremented
// after being consumed by the outer concat. Additionally, the `emit_str_rc_dec`
// helper must pass `ori_str_drop_buffer` (not null) as the drop function, so
// `ori_rc_dec` can free the buffer when RC reaches 0.
//
// These tests form a matrix: {short (SSO) str, long (heap) str} x
// {single str field, multiple str fields, mixed str+int fields}.

/// Semantic pin: long str field (heap-backed intermediate) — the core
/// regression case. Without the fix, `ori_str_concat`'s in-place append
/// reuses the buffer, and the intermediate's RC reaches 0 inside the
/// derive function with no drop function to free it.
#[test]
fn test_aot_derive_debug_str_field_no_leak() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_debug_str_field_no_leak.ori"),
        "derive_debug_str_field_no_leak",
    );
}

/// Short str field (SSO) — the intermediate stays SSO, so no heap
/// allocation to leak. Verifies the fix doesn't break the SSO path.
#[test]
fn test_aot_derive_debug_short_str_no_leak() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_debug_short_str_no_leak.ori"),
        "derive_debug_short_str_no_leak",
    );
}

/// Multiple str fields — each field independently creates a quoted
/// intermediate. All must be properly freed.
#[test]
fn test_aot_derive_debug_multi_str_no_leak() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_debug_multi_str_no_leak.ori"),
        "derive_debug_multi_str_no_leak",
    );
}

/// Mixed str + int fields — the str field goes through the Debug quoting
/// path while the int field goes through the sext/format path. Both must
/// clean up properly without interfering with each other.
#[test]
fn test_aot_derive_debug_mixed_str_int_no_leak() {
    assert_aot_success(
        include_str!("fixtures/derives/aot_derive_debug_mixed_str_int_no_leak.ori"),
        "derive_debug_mixed_str_int_no_leak",
    );
}
