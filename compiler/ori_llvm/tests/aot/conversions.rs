//! Type Conversion AOT Tests
//!
//! Tests for primitive type conversion methods: `to_int`, `to_float`, `f`,
//! `byte`, `abs`, `to_str`, `into`, and chained conversions. Verifies that
//! AOT codegen produces correct LLVM cast instructions (sitofp, fptosi,
//! trunc, zext, sext).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── int.to_float / int.f ───

#[test]
fn test_conv_int_to_float() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_float.ori"),
        "conv_int_to_float",
    );
}

#[test]
fn test_conv_int_f_shorthand() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_f_shorthand.ori"),
        "conv_int_f",
    );
}

#[test]
fn test_conv_int_to_float_negative() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_float_negative.ori"),
        "conv_int_to_float_neg",
    );
}

#[test]
fn test_conv_int_to_float_zero() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_float_zero.ori"),
        "conv_int_to_float_zero",
    );
}

#[test]
fn test_conv_int_to_float_large() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_float_large.ori"),
        "conv_int_to_float_large",
    );
}

// ─── float.to_int ───

#[test]
fn test_conv_float_to_int() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_to_int.ori"),
        "conv_float_to_int",
    );
}

#[test]
fn test_conv_float_to_int_truncates() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_to_int_truncates.ori"),
        "conv_float_to_int_trunc",
    );
}

#[test]
fn test_conv_float_to_int_negative_truncates() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_to_int_negative_truncates.ori"),
        "conv_float_to_int_neg_trunc",
    );
}

#[test]
fn test_conv_float_to_int_zero() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_to_int_zero.ori"),
        "conv_float_to_int_zero",
    );
}

#[test]
fn test_conv_float_to_int_negative_zero() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_to_int_negative_zero.ori"),
        "conv_float_to_int_neg_zero",
    );
}

// ─── int.into (int -> float) ───

#[test]
fn test_conv_int_into_float() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_into_float.ori"),
        "conv_int_into_float",
    );
}

// ─── bool.to_int ───

#[test]
fn test_conv_bool_to_int_true() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_bool_to_int_true.ori"),
        "conv_bool_to_int_true",
    );
}

#[test]
fn test_conv_bool_to_int_false() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_bool_to_int_false.ori"),
        "conv_bool_to_int_false",
    );
}

// ─── char.to_int ───

#[test]
fn test_conv_char_to_int_ascii() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_char_to_int_ascii.ori"),
        "conv_char_to_int_ascii",
    );
}

#[test]
fn test_conv_char_to_int_zero() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_char_to_int_zero.ori"),
        "conv_char_to_int_zero",
    );
}

#[test]
fn test_conv_char_to_int_space() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_char_to_int_space.ori"),
        "conv_char_to_int_space",
    );
}

// ─── byte.to_int ───

#[test]
fn test_conv_byte_to_int() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_byte_to_int.ori"),
        "conv_byte_to_int",
    );
}

#[test]
fn test_conv_byte_to_int_zero() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_byte_to_int_zero.ori"),
        "conv_byte_to_int_zero",
    );
}

#[test]
fn test_conv_byte_to_int_max() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_byte_to_int_max.ori"),
        "conv_byte_to_int_max",
    );
}

// ─── int.byte ───

#[test]
fn test_conv_int_to_byte() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_byte.ori"),
        "conv_int_to_byte",
    );
}

#[test]
fn test_conv_int_to_byte_truncates() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_byte_truncates.ori"),
        "conv_int_to_byte_trunc",
    );
}

#[test]
fn test_conv_int_to_byte_max() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_byte_max.ori"),
        "conv_int_to_byte_max",
    );
}

// ─── abs ───

#[test]
fn test_conv_int_abs_positive() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_abs_positive.ori"),
        "conv_int_abs_pos",
    );
}

#[test]
fn test_conv_int_abs_negative() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_abs_negative.ori"),
        "conv_int_abs_neg",
    );
}

#[test]
fn test_conv_int_abs_zero() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_abs_zero.ori"),
        "conv_int_abs_zero",
    );
}

#[test]
fn test_conv_float_abs_positive() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_abs_positive.ori"),
        "conv_float_abs_pos",
    );
}

#[test]
fn test_conv_float_abs_negative() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_abs_negative.ori"),
        "conv_float_abs_neg",
    );
}

#[test]
fn test_conv_float_abs_zero() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_abs_zero.ori"),
        "conv_float_abs_zero",
    );
}

// ─── to_str ───

#[test]
fn test_conv_int_to_str() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_str.ori"),
        "conv_int_to_str",
    );
}

#[test]
fn test_conv_int_to_str_negative() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_str_negative.ori"),
        "conv_int_to_str_neg",
    );
}

#[test]
fn test_conv_int_to_str_zero() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_str_zero.ori"),
        "conv_int_to_str_zero",
    );
}

#[test]
fn test_conv_float_to_str() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_float_to_str.ori"),
        "conv_float_to_str",
    );
}

#[test]
fn test_conv_bool_to_str_true() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_bool_to_str_true.ori"),
        "conv_bool_to_str_true",
    );
}

#[test]
fn test_conv_bool_to_str_false() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_bool_to_str_false.ori"),
        "conv_bool_to_str_false",
    );
}

// ─── Ordering.to_int ───

#[test]
fn test_conv_ordering_to_int() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_ordering_to_int.ori"),
        "conv_ordering_to_int",
    );
}

// ─── Chained conversions ───

#[test]
fn test_conv_int_to_float_to_int() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_float_to_int.ori"),
        "conv_roundtrip_int_float",
    );
}

#[test]
fn test_conv_int_to_byte_to_int() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_int_to_byte_to_int.ori"),
        "conv_roundtrip_int_byte",
    );
}

#[test]
fn test_conv_bool_to_int_to_float() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_bool_to_int_to_float.ori"),
        "conv_chain_bool_int_float",
    );
}

#[test]
fn test_conv_char_to_int_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_char_to_int_arithmetic.ori"),
        "conv_char_to_int_arith",
    );
}

// ─── Conversion in expressions ───

#[test]
fn test_conv_in_comparison() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_in_comparison.ori"),
        "conv_in_comparison",
    );
}

#[test]
fn test_conv_in_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_in_arithmetic.ori"),
        "conv_in_arithmetic",
    );
}

#[test]
fn test_conv_abs_in_expression() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_abs_in_expression.ori"),
        "conv_abs_in_expr",
    );
}

#[test]
fn test_conv_to_str_concat() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_to_str_concat.ori"),
        "conv_to_str_concat",
    );
}

#[test]
fn test_conv_multiple_to_str() {
    assert_aot_success(
        include_str!("fixtures/conversions/conv_multiple_to_str.ori"),
        "conv_multiple_to_str",
    );
}
