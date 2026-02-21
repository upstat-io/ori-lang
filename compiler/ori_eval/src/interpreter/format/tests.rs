//! Tests for format spec evaluation functions.

#![allow(
    clippy::approx_constant,
    reason = "3.14 is a test value, not an approximation of PI"
)]

use ori_ir::format_spec::{Align, FormatType, ParsedFormatSpec, Sign};

use super::*;

fn spec() -> ParsedFormatSpec {
    ParsedFormatSpec::EMPTY
}

// Integer formatting

#[test]
fn int_decimal() {
    assert_eq!(format_int(42, &spec()), "42");
}

#[test]
fn int_negative() {
    assert_eq!(format_int(-42, &spec()), "-42");
}

#[test]
fn int_zero() {
    assert_eq!(format_int(0, &spec()), "0");
}

#[test]
fn int_binary() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Binary),
        ..spec()
    };
    assert_eq!(format_int(42, &s), "101010");
}

#[test]
fn int_binary_alternate() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Binary),
        alternate: true,
        ..spec()
    };
    assert_eq!(format_int(42, &s), "0b101010");
}

#[test]
fn int_octal() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Octal),
        ..spec()
    };
    assert_eq!(format_int(42, &s), "52");
}

#[test]
fn int_octal_alternate() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Octal),
        alternate: true,
        ..spec()
    };
    assert_eq!(format_int(42, &s), "0o52");
}

#[test]
fn int_hex() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Hex),
        ..spec()
    };
    assert_eq!(format_int(255, &s), "ff");
}

#[test]
fn int_hex_upper() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::HexUpper),
        ..spec()
    };
    assert_eq!(format_int(255, &s), "FF");
}

#[test]
fn int_hex_alternate() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Hex),
        alternate: true,
        ..spec()
    };
    assert_eq!(format_int(255, &s), "0xff");
}

#[test]
fn int_sign_plus() {
    let s = ParsedFormatSpec {
        sign: Some(Sign::Plus),
        ..spec()
    };
    assert_eq!(format_int(42, &s), "+42");
}

#[test]
fn int_sign_space() {
    let s = ParsedFormatSpec {
        sign: Some(Sign::Space),
        ..spec()
    };
    assert_eq!(format_int(42, &s), " 42");
}

#[test]
fn int_zero_pad() {
    let s = ParsedFormatSpec {
        zero_pad: true,
        width: Some(8),
        ..spec()
    };
    assert_eq!(format_int(42, &s), "00000042");
}

#[test]
fn int_zero_pad_negative() {
    let s = ParsedFormatSpec {
        zero_pad: true,
        width: Some(8),
        ..spec()
    };
    assert_eq!(format_int(-42, &s), "-0000042");
}

#[test]
fn int_zero_pad_hex() {
    let s = ParsedFormatSpec {
        zero_pad: true,
        width: Some(8),
        format_type: Some(FormatType::Hex),
        ..spec()
    };
    assert_eq!(format_int(255, &s), "000000ff");
}

#[test]
fn int_width_right_align() {
    let s = ParsedFormatSpec {
        width: Some(10),
        align: Some(Align::Right),
        ..spec()
    };
    assert_eq!(format_int(42, &s), "        42");
}

// Float formatting

#[test]
fn float_default() {
    assert_eq!(format_float(3.14, &spec()), "3.14");
}

#[test]
fn float_precision() {
    let s = ParsedFormatSpec {
        precision: Some(2),
        ..spec()
    };
    assert_eq!(format_float(3.14159, &s), "3.14");
}

#[test]
fn float_fixed() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Fixed),
        precision: Some(2),
        ..spec()
    };
    assert_eq!(format_float(3.14159, &s), "3.14");
}

#[test]
fn float_fixed_default_precision() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Fixed),
        ..spec()
    };
    assert_eq!(format_float(3.14, &s), "3.140000");
}

#[test]
fn float_scientific() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Exp),
        precision: Some(4),
        ..spec()
    };
    let result = format_float(1234.5, &s);
    assert_eq!(result, "1.2345e3");
}

#[test]
fn float_scientific_upper() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::ExpUpper),
        precision: Some(4),
        ..spec()
    };
    let result = format_float(1234.5, &s);
    assert_eq!(result, "1.2345E3");
}

#[test]
fn float_percent() {
    let s = ParsedFormatSpec {
        format_type: Some(FormatType::Percent),
        precision: Some(0),
        ..spec()
    };
    assert_eq!(format_float(0.75, &s), "75%");
}

#[test]
fn float_sign_plus() {
    let s = ParsedFormatSpec {
        sign: Some(Sign::Plus),
        ..spec()
    };
    assert_eq!(format_float(3.14, &s), "+3.14");
}

#[test]
fn float_zero_pad() {
    let s = ParsedFormatSpec {
        zero_pad: true,
        width: Some(10),
        precision: Some(2),
        ..spec()
    };
    assert_eq!(format_float(3.14, &s), "0000003.14");
}

// String formatting

#[test]
fn str_no_format() {
    assert_eq!(format_str("hello", &spec()), "hello");
}

#[test]
fn str_width_left() {
    let s = ParsedFormatSpec {
        width: Some(10),
        align: Some(Align::Left),
        ..spec()
    };
    assert_eq!(format_str("hello", &s), "hello     ");
}

#[test]
fn str_width_right() {
    let s = ParsedFormatSpec {
        width: Some(10),
        align: Some(Align::Right),
        ..spec()
    };
    assert_eq!(format_str("hello", &s), "     hello");
}

#[test]
fn str_width_center() {
    let s = ParsedFormatSpec {
        width: Some(10),
        align: Some(Align::Center),
        ..spec()
    };
    assert_eq!(format_str("hello", &s), "  hello   ");
}

#[test]
fn str_fill_and_center() {
    let s = ParsedFormatSpec {
        fill: Some('*'),
        width: Some(10),
        align: Some(Align::Center),
        ..spec()
    };
    assert_eq!(format_str("hello", &s), "**hello***");
}

#[test]
fn str_precision_truncation() {
    let s = ParsedFormatSpec {
        precision: Some(3),
        ..spec()
    };
    assert_eq!(format_str("hello", &s), "hel");
}

#[test]
fn str_precision_no_truncation() {
    let s = ParsedFormatSpec {
        precision: Some(10),
        ..spec()
    };
    assert_eq!(format_str("hello", &s), "hello");
}

// Apply alignment edge cases

#[test]
fn alignment_no_width() {
    assert_eq!(apply_alignment("hello", &spec()), "hello");
}

#[test]
fn alignment_width_smaller_than_content() {
    let s = ParsedFormatSpec {
        width: Some(3),
        ..spec()
    };
    assert_eq!(apply_alignment("hello", &s), "hello");
}

// Cross-formatter conformance tests (golden output)
//
// These tests define (input, spec_string, expected_output) triples that MUST
// produce identical results in both `ori_eval` (tree-walking evaluator) and
// `ori_rt` (AOT runtime). The same triples appear in
// `ori_rt/src/format/tests.rs` — if either formatter drifts, its golden tests
// fail.

/// Helper: parse spec string, format an int, compare against expected.
fn assert_int_formats_to(n: i64, spec_str: &str, expected: &str) {
    let parsed = ori_ir::format_spec::parse_format_spec(spec_str)
        .unwrap_or_else(|e| panic!("parse_format_spec(\"{spec_str}\") failed: {e}"));
    let result = format_int(n, &parsed);
    assert_eq!(
        result, expected,
        "ori_eval format_int({n}, \"{spec_str}\") = \"{result}\", expected \"{expected}\""
    );
}

/// Helper: parse spec string, format a float, compare against expected.
fn assert_float_formats_to(f: f64, spec_str: &str, expected: &str) {
    let parsed = ori_ir::format_spec::parse_format_spec(spec_str)
        .unwrap_or_else(|e| panic!("parse_format_spec(\"{spec_str}\") failed: {e}"));
    let result = format_float(f, &parsed);
    assert_eq!(
        result, expected,
        "ori_eval format_float({f}, \"{spec_str}\") = \"{result}\", expected \"{expected}\""
    );
}

/// Helper: parse spec string, format a string, compare against expected.
fn assert_str_formats_to(s: &str, spec_str: &str, expected: &str) {
    let parsed = ori_ir::format_spec::parse_format_spec(spec_str)
        .unwrap_or_else(|e| panic!("parse_format_spec(\"{spec_str}\") failed: {e}"));
    let result = format_str(s, &parsed);
    assert_eq!(
        result, expected,
        "ori_eval format_str(\"{s}\", \"{spec_str}\") = \"{result}\", expected \"{expected}\""
    );
}

/// Golden output conformance for integer formatting.
///
/// These exact triples are duplicated in `ori_rt/src/format/tests.rs`.
/// If this test fails, the `ori_eval` formatter has drifted from the runtime.
#[test]
fn golden_int_conformance() {
    // Decimal
    assert_int_formats_to(42, "", "42");
    assert_int_formats_to(-42, "", "-42");
    assert_int_formats_to(0, "", "0");
    // Binary
    assert_int_formats_to(42, "b", "101010");
    assert_int_formats_to(42, "#b", "0b101010");
    // Octal
    assert_int_formats_to(42, "o", "52");
    assert_int_formats_to(42, "#o", "0o52");
    // Hex
    assert_int_formats_to(255, "x", "ff");
    assert_int_formats_to(255, "X", "FF");
    assert_int_formats_to(255, "#x", "0xff");
    // Sign
    assert_int_formats_to(42, "+", "+42");
    assert_int_formats_to(42, " ", " 42");
    // Zero-pad
    assert_int_formats_to(42, "08", "00000042");
    assert_int_formats_to(-42, "08", "-0000042");
    assert_int_formats_to(255, "08x", "000000ff");
    // Width + alignment
    assert_int_formats_to(42, ">10", "        42");
    assert_int_formats_to(42, "<10", "42        ");
    assert_int_formats_to(42, "^10", "    42    ");
    // Fill + alignment
    assert_int_formats_to(42, "*>10", "********42");
    assert_int_formats_to(42, "*^10", "****42****");
}

/// Golden output conformance for float formatting.
#[test]
fn golden_float_conformance() {
    // Default
    assert_float_formats_to(3.14, "", "3.14");
    // Precision
    assert_float_formats_to(3.14159, ".2", "3.14");
    // Fixed
    assert_float_formats_to(3.14159, ".2f", "3.14");
    assert_float_formats_to(3.14, "f", "3.140000");
    // Scientific
    assert_float_formats_to(1234.5, ".4e", "1.2345e3");
    assert_float_formats_to(1234.5, ".4E", "1.2345E3");
    // Percent
    assert_float_formats_to(0.75, ".0%", "75%");
    // Sign
    assert_float_formats_to(3.14, "+", "+3.14");
    // Zero-pad
    assert_float_formats_to(3.14, "010.2", "0000003.14");
    // Negative
    assert_float_formats_to(-3.14, "", "-3.14");
}

/// Golden output conformance for string formatting.
#[test]
fn golden_str_conformance() {
    // No format
    assert_str_formats_to("hello", "", "hello");
    // Width + alignment
    assert_str_formats_to("hello", "<10", "hello     ");
    assert_str_formats_to("hello", ">10", "     hello");
    assert_str_formats_to("hello", "^10", "  hello   ");
    // Fill + alignment
    assert_str_formats_to("hello", "*^10", "**hello***");
    // Precision truncation
    assert_str_formats_to("hello", ".3", "hel");
    assert_str_formats_to("hello", ".10", "hello");
}

/// Golden output conformance for bool (formatted as string).
#[test]
fn golden_bool_conformance() {
    assert_str_formats_to("true", ">10", "      true");
    assert_str_formats_to("false", "<10", "false     ");
}

/// Golden output conformance for char (formatted as string).
#[test]
fn golden_char_conformance() {
    assert_str_formats_to("A", ">5", "    A");
    assert_str_formats_to("A", "*^5", "**A**");
}
