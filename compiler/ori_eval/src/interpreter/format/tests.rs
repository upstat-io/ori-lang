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
