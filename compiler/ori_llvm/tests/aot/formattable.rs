//! AOT Formattable Trait Codegen Tests (§3.16)
//!
//! End-to-end tests verifying that format spec expressions (`{value:spec}`)
//! generate correct native code through the LLVM backend. Each test compiles
//! Ori source to a native binary, runs it, and checks exit code 0.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// =============================================================================
// Integer Formatting
// =============================================================================

#[test]
fn test_aot_format_int_hex() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_int_hex.ori"),
        "format_int_hex",
    );
}

#[test]
fn test_aot_format_int_binary() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_int_binary.ori"),
        "format_int_binary",
    );
}

#[test]
fn test_aot_format_int_octal() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_int_octal.ori"),
        "format_int_octal",
    );
}

#[test]
fn test_aot_format_int_sign() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_int_sign.ori"),
        "format_int_sign",
    );
}

#[test]
fn test_aot_format_int_zero_pad() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_int_zero_pad.ori"),
        "format_int_zero_pad",
    );
}

#[test]
fn test_aot_format_int_width_align() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_int_width_align.ori"),
        "format_int_width_align",
    );
}

// =============================================================================
// Float Formatting
// =============================================================================

#[test]
fn test_aot_format_float_fixed() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_float_fixed.ori"),
        "format_float_fixed",
    );
}

#[test]
fn test_aot_format_float_precision() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_float_precision.ori"),
        "format_float_precision",
    );
}

#[test]
fn test_aot_format_float_percent() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_float_percent.ori"),
        "format_float_percent",
    );
}

#[test]
fn test_aot_format_float_sign() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_float_sign.ori"),
        "format_float_sign",
    );
}

// =============================================================================
// String Formatting
// =============================================================================

#[test]
fn test_aot_format_str_width() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_str_width.ori"),
        "format_str_width",
    );
}

#[test]
fn test_aot_format_str_fill() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_str_fill.ori"),
        "format_str_fill",
    );
}

#[test]
fn test_aot_format_str_precision() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_str_precision.ori"),
        "format_str_precision",
    );
}

// =============================================================================
// Bool and Char Formatting
// =============================================================================

#[test]
fn test_aot_format_bool_width() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_bool_width.ori"),
        "format_bool_width",
    );
}

#[test]
fn test_aot_format_char_width() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_char_width.ori"),
        "format_char_width",
    );
}

// =============================================================================
// Custom Fill Characters
// =============================================================================

#[test]
fn test_aot_format_fill_center() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_fill_center.ori"),
        "format_fill_center",
    );
}

#[test]
fn test_aot_format_negative_hex() {
    assert_aot_success(
        include_str!("fixtures/formattable/aot_format_negative_hex.ori"),
        "format_negative_hex",
    );
}
