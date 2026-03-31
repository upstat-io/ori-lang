//! Pattern Matching Extension AOT Tests
//!
//! Tests for advanced pattern matching features beyond basic literal/wildcard:
//! or-patterns, guard clauses, tuple patterns, struct patterns, and binding
//! patterns in match expressions.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Or-patterns in match ───

#[test]
fn test_pattern_or_int_literals() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_or_int_literals.ori"),
        "pattern_or_int",
    );
}

#[test]
fn test_pattern_or_char_literals() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_or_char_literals.ori"),
        "pattern_or_char",
    );
}

#[test]
fn test_pattern_or_bool() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_or_bool.ori"),
        "pattern_or_bool",
    );
}

#[test]
fn test_pattern_or_in_loop() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_or_in_loop.ori"),
        "pattern_or_in_loop",
    );
}

// ─── Guard clauses ───

#[test]
fn test_pattern_guard_basic() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_guard_basic.ori"),
        "pattern_guard_basic",
    );
}

#[test]
fn test_pattern_guard_with_binding() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_guard_with_binding.ori"),
        "pattern_guard_binding",
    );
}

#[test]
fn test_pattern_guard_complex_condition() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_guard_complex_condition.ori"),
        "pattern_guard_complex",
    );
}

#[test]
fn test_pattern_guard_in_loop() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_guard_in_loop.ori"),
        "pattern_guard_loop",
    );
}

// ─── Tuple patterns in match ───

#[test]
fn test_pattern_tuple_basic() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_tuple_basic.ori"),
        "pattern_tuple_basic",
    );
}

#[test]
fn test_pattern_tuple_second_arm() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_tuple_second_arm.ori"),
        "pattern_tuple_second",
    );
}

#[test]
fn test_pattern_tuple_wildcard_fallthrough() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_tuple_wildcard_fallthrough.ori"),
        "pattern_tuple_wildcard",
    );
}

#[test]
fn test_pattern_tuple_3_elements() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_tuple_3_elements.ori"),
        "pattern_tuple_3elem",
    );
}

#[test]
fn test_pattern_tuple_all_wildcards() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_tuple_all_wildcards.ori"),
        "pattern_tuple_all_wild",
    );
}

#[test]
fn test_pattern_tuple_from_function() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_tuple_from_function.ori"),
        "pattern_tuple_from_fn",
    );
}

// ─── Binding patterns ───

#[test]
fn test_pattern_binding_capture() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_binding_capture.ori"),
        "pattern_binding_capture",
    );
}

#[test]
fn test_pattern_binding_with_literal_arms() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_binding_with_literal_arms.ori"),
        "pattern_binding_mixed",
    );
}

// ─── Combined patterns ───

#[test]
fn test_pattern_guard_with_tuple() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_guard_with_tuple.ori"),
        "pattern_guard_tuple",
    );
}

#[test]
fn test_pattern_match_on_result_tag() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_match_on_result_tag.ori"),
        "pattern_result_dispatch",
    );
}

#[test]
fn test_pattern_nested_match() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_nested_match.ori"),
        "pattern_nested_match",
    );
}

#[test]
fn test_pattern_match_expression_in_function() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_match_expression_in_function.ori"),
        "pattern_fizzbuzz",
    );
}

// ─── Match exhaustiveness ───

#[test]
fn test_pattern_match_all_bool_cases() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_match_all_bool_cases.ori"),
        "pattern_exhaust_bool",
    );
}

#[test]
fn test_pattern_match_many_char_literals() {
    assert_aot_success(
        include_str!("fixtures/patterns/pattern_match_many_char_literals.ori"),
        "pattern_many_chars",
    );
}
