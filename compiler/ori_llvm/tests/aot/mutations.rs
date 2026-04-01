//! Variable Mutation & Reassignment AOT Tests
//!
//! Tests for variable reassignment semantics in various contexts:
//! simple reassignment, loops, match arms, conditionals, and
//! accumulator patterns.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Simple reassignment ───

#[test]
fn test_mut_simple_reassign() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_simple_reassign.ori"),
        "mut_simple",
    );
}

#[test]
fn test_mut_reassign_multiple() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_reassign_multiple.ori"),
        "mut_multiple",
    );
}

#[test]
fn test_mut_reassign_self_reference() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_reassign_self_reference.ori"),
        "mut_self_ref",
    );
}

// ─── Loop accumulator patterns ───

#[test]
fn test_mut_loop_counter() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_loop_counter.ori"),
        "mut_loop_counter",
    );
}

#[test]
fn test_mut_loop_accumulator() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_loop_accumulator.ori"),
        "mut_loop_accum",
    );
}

#[test]
fn test_mut_loop_product() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_loop_product.ori"),
        "mut_loop_product",
    );
}

#[test]
fn test_mut_loop_conditional_accumulator() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_loop_conditional_accumulator.ori"),
        "mut_loop_cond_accum",
    );
}

// ─── Loop with break ───

#[test]
fn test_mut_loop_break() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_loop_break.ori"),
        "mut_loop_break",
    );
}

#[test]
fn test_mut_while_pattern() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_while_pattern.ori"),
        "mut_while_pattern",
    );
}

// ─── Reassignment in conditionals ───

#[test]
fn test_mut_if_reassign() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_if_reassign.ori"),
        "mut_if_reassign",
    );
}

#[test]
fn test_mut_if_else_reassign() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_if_else_reassign.ori"),
        "mut_if_else_reassign",
    );
}

// ─── Reassignment with different types ───

#[test]
fn test_mut_reassign_string() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_reassign_string.ori"),
        "mut_reassign_string",
    );
}

#[test]
fn test_mut_reassign_bool() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_reassign_bool.ori"),
        "mut_reassign_bool",
    );
}

// ─── Multiple variables ───

#[test]
fn test_mut_swap_values() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_swap_values.ori"),
        "mut_swap",
    );
}

#[test]
fn test_mut_fibonacci() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_fibonacci.ori"),
        "mut_fibonacci",
    );
}

#[test]
fn test_mut_min_max_tracking() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_min_max_tracking.ori"),
        "mut_min_max",
    );
}

// ─── Nested loops with mutation ───

#[test]
fn test_mut_nested_loop() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_nested_loop.ori"),
        "mut_nested_loop",
    );
}

#[test]
fn test_mut_outer_from_inner() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_outer_from_inner.ori"),
        "mut_outer_from_inner",
    );
}

// ─── String building ───

#[test]
fn test_mut_string_builder() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_string_builder.ori"),
        "mut_string_builder",
    );
}

// ─── Reassignment with function calls ───

#[test]
fn test_mut_function_result() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_function_result.ori"),
        "mut_function_result",
    );
}

#[test]
fn test_mut_accumulate_function() {
    assert_aot_success(
        include_str!("fixtures/mutations/mut_accumulate_function.ori"),
        "mut_accum_func",
    );
}
