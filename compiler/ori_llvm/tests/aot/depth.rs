//! Depth & Complexity AOT Tests
//!
//! Tests that push the AOT pipeline on **complexity**: many match arms, deep
//! control flow nesting, long error-propagation chains, complex closure patterns,
//! and multi-derive struct combinations. These verify LLVM codegen correctness
//! for non-trivial program structures.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Match with many arms ───

#[test]
fn test_depth_match_20_arms() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_match_20_arms.ori"),
        "depth_match_20_arms",
    );
}

#[test]
fn test_depth_match_in_loop() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_match_in_loop.ori"),
        "depth_match_in_loop",
    );
}

// ─── Nested control flow depth ───

#[test]
fn test_depth_nested_if_5_levels() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_nested_if_5_levels.ori"),
        "depth_nested_if_5",
    );
}

#[test]
fn test_depth_nested_loops_break_from_inner() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_nested_loops_break_from_inner.ori"),
        "depth_nested_loops_break",
    );
}

#[test]
fn test_depth_nested_loop_continue_and_break() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_nested_loop_continue_and_break.ori"),
        "depth_nested_continue_break",
    );
}

#[test]
fn test_depth_match_inside_loop_inside_match() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_match_inside_loop_inside_match.ori"),
        "depth_match_loop_match",
    );
}

#[test]
fn test_depth_break_value_nested() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_break_value_nested.ori"),
        "depth_break_value_nested",
    );
}

#[test]
fn test_depth_complex_for_guard() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_complex_for_guard.ori"),
        "depth_complex_for_guard",
    );
}

// ─── Deep ? chains (error propagation) ───

#[test]
fn test_depth_try_chain_5_levels() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_try_chain_5_levels.ori"),
        "depth_try_chain_5",
    );
}

#[test]
fn test_depth_try_chain_early_fail() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_try_chain_early_fail.ori"),
        "depth_try_early_fail",
    );
}

#[test]
fn test_depth_try_option_chain() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_try_option_chain.ori"),
        "depth_try_option_chain",
    );
}

// ─── unwrap_or with computed defaults ───

#[test]
fn test_depth_unwrap_or_complex() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_unwrap_or_complex.ori"),
        "depth_unwrap_or",
    );
}

// ─── Complex closure patterns ───

#[test]
fn test_depth_closure_capturing_struct() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_closure_capturing_struct.ori"),
        "depth_closure_capture_struct",
    );
}

#[test]
fn test_depth_closure_capturing_string() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_closure_capturing_string.ori"),
        "depth_closure_capture_string",
    );
}

#[test]
fn test_depth_closure_capturing_multiple_strings() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_closure_capturing_multiple_strings.ori"),
        "depth_closure_capture_multi_string",
    );
}

#[test]
fn test_depth_closure_passed_through_3_functions() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_closure_passed_through_3_functions.ori"),
        "depth_closure_3_levels",
    );
}

#[test]
fn test_depth_closure_mixed_captures() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_closure_mixed_captures.ori"),
        "depth_closure_mixed_captures",
    );
}

#[test]
fn test_depth_multiple_closures_same_scope() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_multiple_closures_same_scope.ori"),
        "depth_multiple_closures",
    );
}

// ─── Multi-derive combinations ───

#[test]
fn test_depth_derive_eq_comparable_hashable() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_derive_eq_comparable_hashable.ori"),
        "depth_derive_triple",
    );
}

#[test]
fn test_depth_derive_5_traits() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_derive_5_traits.ori"),
        "depth_derive_5_traits",
    );
}

#[test]
fn test_depth_derive_eq_on_struct_with_many_types() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_derive_eq_on_struct_with_many_types.ori"),
        "depth_derive_eq_many_types",
    );
}

#[test]
fn test_depth_derive_comparable_string_fields() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_derive_comparable_string_fields.ori"),
        "depth_derive_comparable_strings",
    );
}

#[test]
fn test_depth_derive_hashable_consistency() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_derive_hashable_consistency.ori"),
        "depth_derive_hashable_contract",
    );
}

// ─── Nested Option/Result types ───

#[test]
fn test_depth_nested_option() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_nested_option.ori"),
        "depth_nested_option",
    );
}

#[test]
fn test_depth_result_with_option_payload() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_result_with_option_payload.ori"),
        "depth_result_option_payload",
    );
}

// ─── Complex expressions combining multiple features ───

#[test]
fn test_depth_combined_match_closure_result() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_combined_match_closure_result.ori"),
        "depth_combined_match_closure_result",
    );
}

#[test]
fn test_depth_combined_struct_iter_match() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_combined_struct_iter_match.ori"),
        "depth_combined_struct_iter_match",
    );
}

#[test]
fn test_depth_combined_recursion_with_match() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_combined_recursion_with_match.ori"),
        "depth_recursion_match",
    );
}

#[test]
fn test_depth_combined_all_features() {
    assert_aot_success(
        include_str!("fixtures/depth/depth_combined_all_features.ori"),
        "depth_combined_all_features",
    );
}
