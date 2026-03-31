//! Operator Edge Case & Expression AOT Tests
//!
//! Tests covering operator semantics that basic conformance tests miss:
//! integer boundaries, negative division/modulo, float specials, boolean
//! short-circuit semantics, empty string operations, complex precedence
//! chains, and unary operator combinations.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

// ─── Integer boundary values ───

#[test]
fn test_op_large_integer_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/operators/op_large_integer_arithmetic.ori"),
        "op_large_int",
    );
}

#[test]
fn test_op_large_integer_multiplication() {
    assert_aot_success(
        include_str!("fixtures/operators/op_large_integer_multiplication.ori"),
        "op_large_mul",
    );
}

#[test]
fn test_op_negative_large_integer() {
    assert_aot_success(
        include_str!("fixtures/operators/op_negative_large_integer.ori"),
        "op_negative_large",
    );
}

#[test]
fn test_op_zero_boundary() {
    assert_aot_success(
        include_str!("fixtures/operators/op_zero_boundary.ori"),
        "op_zero_boundary",
    );
}

// ─── Division edge cases ───

#[test]
fn test_op_division_truncation_positive() {
    assert_aot_success(
        include_str!("fixtures/operators/op_division_truncation_positive.ori"),
        "op_div_truncation_pos",
    );
}

#[test]
fn test_op_division_negative_dividend() {
    assert_aot_success(
        include_str!("fixtures/operators/op_division_negative_dividend.ori"),
        "op_div_neg_dividend",
    );
}

#[test]
fn test_op_division_negative_divisor() {
    assert_aot_success(
        include_str!("fixtures/operators/op_division_negative_divisor.ori"),
        "op_div_neg_divisor",
    );
}

#[test]
fn test_op_division_both_negative() {
    assert_aot_success(
        include_str!("fixtures/operators/op_division_both_negative.ori"),
        "op_div_both_neg",
    );
}

#[test]
fn test_op_division_exact() {
    assert_aot_success(
        include_str!("fixtures/operators/op_division_exact.ori"),
        "op_div_exact",
    );
}

// ─── Modulo edge cases ───

#[test]
fn test_op_modulo_negative_dividend() {
    assert_aot_success(
        include_str!("fixtures/operators/op_modulo_negative_dividend.ori"),
        "op_mod_neg_dividend",
    );
}

#[test]
fn test_op_modulo_negative_divisor() {
    assert_aot_success(
        include_str!("fixtures/operators/op_modulo_negative_divisor.ori"),
        "op_mod_neg_divisor",
    );
}

#[test]
fn test_op_modulo_both_negative() {
    assert_aot_success(
        include_str!("fixtures/operators/op_modulo_both_negative.ori"),
        "op_mod_both_neg",
    );
}

#[test]
fn test_op_modulo_zero_remainder() {
    assert_aot_success(
        include_str!("fixtures/operators/op_modulo_zero_remainder.ori"),
        "op_mod_zero_remainder",
    );
}

// ─── Negation combinations ───

#[test]
fn test_op_double_boolean_negation() {
    assert_aot_success(
        include_str!("fixtures/operators/op_double_boolean_negation.ori"),
        "op_double_bool_neg",
    );
}

#[test]
fn test_op_triple_negation() {
    assert_aot_success(
        include_str!("fixtures/operators/op_triple_negation.ori"),
        "op_triple_negation",
    );
}

// ─── Complex precedence chains ───

#[test]
fn test_op_mixed_precedence_chain() {
    assert_aot_success(
        include_str!("fixtures/operators/op_mixed_precedence_chain.ori"),
        "op_mixed_precedence",
    );
}

#[test]
fn test_op_complex_expression() {
    assert_aot_success(
        include_str!("fixtures/operators/op_complex_expression.ori"),
        "op_complex_expr",
    );
}

#[test]
fn test_op_bitwise_mixed_precedence() {
    assert_aot_success(
        include_str!("fixtures/operators/op_bitwise_mixed_precedence.ori"),
        "op_bitwise_precedence",
    );
}

#[test]
fn test_op_comparison_in_boolean_chain() {
    assert_aot_success(
        include_str!("fixtures/operators/op_comparison_in_boolean_chain.ori"),
        "op_comparison_bool_chain",
    );
}

// ─── Float edge cases ───

#[test]
fn test_op_float_negative_zero() {
    assert_aot_success(
        include_str!("fixtures/operators/op_float_negative_zero.ori"),
        "op_float_neg_zero",
    );
}

#[test]
fn test_op_float_very_small() {
    assert_aot_success(
        include_str!("fixtures/operators/op_float_very_small.ori"),
        "op_float_very_small",
    );
}

#[test]
fn test_op_float_very_large() {
    assert_aot_success(
        include_str!("fixtures/operators/op_float_very_large.ori"),
        "op_float_very_large",
    );
}

#[test]
fn test_op_float_precision() {
    assert_aot_success(
        include_str!("fixtures/operators/op_float_precision.ori"),
        "op_float_precision",
    );
}

#[test]
fn test_op_float_division() {
    assert_aot_success(
        include_str!("fixtures/operators/op_float_division.ori"),
        "op_float_division",
    );
}

// ─── Empty string operations ───

#[test]
fn test_op_empty_string_equality() {
    assert_aot_success(
        include_str!("fixtures/operators/op_empty_string_equality.ori"),
        "op_empty_str_eq",
    );
}

#[test]
fn test_op_empty_string_concat_left() {
    assert_aot_success(
        include_str!("fixtures/operators/op_empty_string_concat_left.ori"),
        "op_empty_str_concat_left",
    );
}

#[test]
fn test_op_empty_string_concat_right() {
    assert_aot_success(
        include_str!("fixtures/operators/op_empty_string_concat_right.ori"),
        "op_empty_str_concat_right",
    );
}

#[test]
fn test_op_empty_string_concat_both() {
    assert_aot_success(
        include_str!("fixtures/operators/op_empty_string_concat_both.ori"),
        "op_empty_str_concat_both",
    );
}

#[test]
fn test_op_empty_string_not_equal() {
    assert_aot_success(
        include_str!("fixtures/operators/op_empty_string_not_equal.ori"),
        "op_empty_str_ne",
    );
}

// ─── Boolean short-circuit semantics ───

#[test]
fn test_op_and_short_circuit() {
    // If && short-circuits, the second condition is never evaluated
    // We test indirectly: false && (complex expression) should still be fast
    assert_aot_success(
        include_str!("fixtures/operators/op_and_short_circuit.ori"),
        "op_and_short_circuit",
    );
}

#[test]
fn test_op_or_short_circuit() {
    assert_aot_success(
        include_str!("fixtures/operators/op_or_short_circuit.ori"),
        "op_or_short_circuit",
    );
}

#[test]
fn test_op_short_circuit_chain() {
    assert_aot_success(
        include_str!("fixtures/operators/op_short_circuit_chain.ori"),
        "op_short_circuit_chain",
    );
}

// ─── Operator on different types ───

#[test]
fn test_op_char_equality() {
    assert_aot_success(
        include_str!("fixtures/operators/op_char_equality.ori"),
        "op_char_eq",
    );
}

#[test]
fn test_op_byte_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/operators/op_byte_arithmetic.ori"),
        "op_byte_arith",
    );
}

#[test]
fn test_op_duration_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/operators/op_duration_arithmetic.ori"),
        "op_duration_arith",
    );
}

#[test]
fn test_op_size_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/operators/op_size_arithmetic.ori"),
        "op_size_arith",
    );
}

// ─── Integer overflow (checked arithmetic) ───

#[test]
fn test_op_overflow_add_max_plus_one() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/operators/op_overflow_add_max_plus_one.ori"
    ));
    assert_ne!(exit_code, 0, "addition overflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_add_min_minus_one() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/operators/op_overflow_add_min_minus_one.ori"
    ));
    assert_ne!(exit_code, 0, "addition underflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_sub_min_minus_one() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/operators/op_overflow_sub_min_minus_one.ori"
    ));
    assert_ne!(exit_code, 0, "subtraction underflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_mul_large_values() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/operators/op_overflow_mul_large_values.ori"
    ));
    assert_ne!(exit_code, 0, "multiplication overflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_mul_min_times_neg_one() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/operators/op_overflow_mul_min_times_neg_one.ori"
    ));
    assert_ne!(exit_code, 0, "INT_MIN * -1 overflow should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_no_overflow_near_boundary() {
    // INT_MAX - 1 + 1 = INT_MAX (no overflow)
    assert_aot_success(
        include_str!("fixtures/operators/op_no_overflow_near_boundary.ori"),
        "op_no_overflow_add",
    );
}

#[test]
fn test_op_no_overflow_sub_near_boundary() {
    // INT_MIN + 1 - 1 = INT_MIN (no overflow)
    assert_aot_success(
        include_str!("fixtures/operators/op_no_overflow_sub_near_boundary.ori"),
        "op_no_overflow_sub",
    );
}

#[test]
fn test_op_no_overflow_mul_near_boundary() {
    // Multiplication that's large but doesn't overflow
    assert_aot_success(
        include_str!("fixtures/operators/op_no_overflow_mul_near_boundary.ori"),
        "op_no_overflow_mul",
    );
}

#[test]
fn test_op_overflow_in_expression() {
    // Overflow inside a complex expression
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/operators/op_overflow_in_expression.ori"
    ));
    assert_ne!(exit_code, 0, "overflow in complex expression should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_overflow_in_function() {
    // Overflow in a called function
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/operators/op_overflow_in_function.ori"
    ));
    assert_ne!(exit_code, 0, "overflow in function should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

// --- Unary negation overflow ---

#[test]
fn test_op_overflow_neg_min() {
    // Unary negation of INT_MIN (-2^63) overflows: result would be 2^63 > INT_MAX
    let (exit_code, _stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/operators/op_overflow_neg_min.ori"));
    assert_ne!(exit_code, 0, "INT_MIN unary negation should panic");
    assert!(
        stderr.contains("overflow") || stderr.contains("panic"),
        "stderr should mention overflow or panic, got: {stderr}"
    );
}

#[test]
fn test_op_no_overflow_neg_zero() {
    // -0 = 0, no overflow
    assert_aot_success(
        include_str!("fixtures/operators/op_no_overflow_neg_zero.ori"),
        "op_no_overflow_neg_zero",
    );
}

#[test]
fn test_op_no_overflow_neg_normal() {
    // -42 = -42, no overflow
    assert_aot_success(
        include_str!("fixtures/operators/op_no_overflow_neg_normal.ori"),
        "op_no_overflow_neg_normal",
    );
}

#[test]
fn test_op_no_overflow_neg_max() {
    // -INT_MAX = INT_MIN + 1, no overflow (largest valid negation)
    assert_aot_success(
        include_str!("fixtures/operators/op_no_overflow_neg_max.ori"),
        "op_no_overflow_neg_max",
    );
}
