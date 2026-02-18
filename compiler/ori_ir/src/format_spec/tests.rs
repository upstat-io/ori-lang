//! Tests for the format specification parser.

#![allow(
    clippy::unwrap_used,
    reason = "test code uses unwrap for concise assertions"
)]

use super::*;

// Empty spec

#[test]
fn empty_spec() {
    let spec = parse_format_spec("").unwrap();
    assert_eq!(spec, ParsedFormatSpec::EMPTY);
}

// Alignment

#[test]
fn align_left() {
    let spec = parse_format_spec("<").unwrap();
    assert_eq!(spec.align, Some(Align::Left));
    assert_eq!(spec.fill, None);
}

#[test]
fn align_right() {
    let spec = parse_format_spec(">").unwrap();
    assert_eq!(spec.align, Some(Align::Right));
}

#[test]
fn align_center() {
    let spec = parse_format_spec("^").unwrap();
    assert_eq!(spec.align, Some(Align::Center));
}

#[test]
fn fill_and_align() {
    let spec = parse_format_spec("*^").unwrap();
    assert_eq!(spec.fill, Some('*'));
    assert_eq!(spec.align, Some(Align::Center));
}

#[test]
fn fill_zero_and_align() {
    let spec = parse_format_spec("0>").unwrap();
    assert_eq!(spec.fill, Some('0'));
    assert_eq!(spec.align, Some(Align::Right));
}

#[test]
fn fill_dash_and_align() {
    let spec = parse_format_spec("-<").unwrap();
    assert_eq!(spec.fill, Some('-'));
    assert_eq!(spec.align, Some(Align::Left));
}

// Sign

#[test]
fn sign_plus() {
    let spec = parse_format_spec("+").unwrap();
    assert_eq!(spec.sign, Some(Sign::Plus));
}

#[test]
fn sign_minus() {
    let spec = parse_format_spec("-").unwrap();
    assert_eq!(spec.sign, Some(Sign::Minus));
}

#[test]
fn sign_space() {
    let spec = parse_format_spec(" ").unwrap();
    assert_eq!(spec.sign, Some(Sign::Space));
}

// Alternate form

#[test]
fn alternate() {
    let spec = parse_format_spec("#").unwrap();
    assert!(spec.alternate);
}

// Zero-pad

#[test]
fn zero_pad_with_width() {
    let spec = parse_format_spec("08").unwrap();
    assert!(spec.zero_pad);
    assert_eq!(spec.width, Some(8));
}

#[test]
fn zero_pad_alone() {
    let spec = parse_format_spec("0").unwrap();
    assert!(spec.zero_pad);
    assert_eq!(spec.width, None);
}

#[test]
fn zero_pad_with_type() {
    let spec = parse_format_spec("0x").unwrap();
    assert!(spec.zero_pad);
    assert_eq!(spec.format_type, Some(FormatType::Hex));
}

// Width

#[test]
fn width_only() {
    let spec = parse_format_spec("10").unwrap();
    assert_eq!(spec.width, Some(10));
}

#[test]
fn width_large() {
    let spec = parse_format_spec("100").unwrap();
    assert_eq!(spec.width, Some(100));
}

// Precision

#[test]
fn precision_only() {
    let spec = parse_format_spec(".2").unwrap();
    assert_eq!(spec.precision, Some(2));
}

#[test]
fn precision_zero() {
    let spec = parse_format_spec(".0").unwrap();
    assert_eq!(spec.precision, Some(0));
}

#[test]
fn precision_dot_alone() {
    let spec = parse_format_spec(".").unwrap();
    assert_eq!(spec.precision, Some(0));
}

#[test]
fn width_and_precision() {
    let spec = parse_format_spec("10.2").unwrap();
    assert_eq!(spec.width, Some(10));
    assert_eq!(spec.precision, Some(2));
}

// Format types

#[test]
fn type_binary() {
    let spec = parse_format_spec("b").unwrap();
    assert_eq!(spec.format_type, Some(FormatType::Binary));
}

#[test]
fn type_octal() {
    let spec = parse_format_spec("o").unwrap();
    assert_eq!(spec.format_type, Some(FormatType::Octal));
}

#[test]
fn type_hex() {
    let spec = parse_format_spec("x").unwrap();
    assert_eq!(spec.format_type, Some(FormatType::Hex));
}

#[test]
fn type_hex_upper() {
    let spec = parse_format_spec("X").unwrap();
    assert_eq!(spec.format_type, Some(FormatType::HexUpper));
}

#[test]
fn type_exp() {
    let spec = parse_format_spec("e").unwrap();
    assert_eq!(spec.format_type, Some(FormatType::Exp));
}

#[test]
fn type_exp_upper() {
    let spec = parse_format_spec("E").unwrap();
    assert_eq!(spec.format_type, Some(FormatType::ExpUpper));
}

#[test]
fn type_fixed() {
    let spec = parse_format_spec("f").unwrap();
    assert_eq!(spec.format_type, Some(FormatType::Fixed));
}

#[test]
fn type_percent() {
    let spec = parse_format_spec("%").unwrap();
    assert_eq!(spec.format_type, Some(FormatType::Percent));
}

// Combined specs

#[test]
fn full_spec() {
    let spec = parse_format_spec("*^+#020.5f").unwrap();
    assert_eq!(spec.fill, Some('*'));
    assert_eq!(spec.align, Some(Align::Center));
    assert_eq!(spec.sign, Some(Sign::Plus));
    assert!(spec.alternate);
    assert!(spec.zero_pad);
    assert_eq!(spec.width, Some(20));
    assert_eq!(spec.precision, Some(5));
    assert_eq!(spec.format_type, Some(FormatType::Fixed));
}

#[test]
fn hex_with_alternate_and_width() {
    let spec = parse_format_spec("#08x").unwrap();
    assert!(spec.alternate);
    assert!(spec.zero_pad);
    assert_eq!(spec.width, Some(8));
    assert_eq!(spec.format_type, Some(FormatType::Hex));
}

#[test]
fn right_align_with_width() {
    let spec = parse_format_spec(">10").unwrap();
    assert_eq!(spec.align, Some(Align::Right));
    assert_eq!(spec.width, Some(10));
}

#[test]
fn fill_align_width_precision() {
    let spec = parse_format_spec("*>10.2").unwrap();
    assert_eq!(spec.fill, Some('*'));
    assert_eq!(spec.align, Some(Align::Right));
    assert_eq!(spec.width, Some(10));
    assert_eq!(spec.precision, Some(2));
}

#[test]
fn zero_pad_hex() {
    let spec = parse_format_spec("08x").unwrap();
    assert!(spec.zero_pad);
    assert_eq!(spec.width, Some(8));
    assert_eq!(spec.format_type, Some(FormatType::Hex));
}

#[test]
fn precision_with_type() {
    let spec = parse_format_spec(".2f").unwrap();
    assert_eq!(spec.precision, Some(2));
    assert_eq!(spec.format_type, Some(FormatType::Fixed));
}

#[test]
fn scientific_with_precision() {
    let spec = parse_format_spec(".6e").unwrap();
    assert_eq!(spec.precision, Some(6));
    assert_eq!(spec.format_type, Some(FormatType::Exp));
}

#[test]
fn percentage_with_precision() {
    let spec = parse_format_spec(".0%").unwrap();
    assert_eq!(spec.precision, Some(0));
    assert_eq!(spec.format_type, Some(FormatType::Percent));
}

// Error cases

#[test]
fn unknown_type() {
    let err = parse_format_spec("z").unwrap_err();
    assert_eq!(err, FormatSpecError::UnknownType('z'));
}

#[test]
fn trailing_characters() {
    let err = parse_format_spec("10xz").unwrap_err();
    assert_eq!(err, FormatSpecError::TrailingCharacters("z".to_string()));
}

// FormatType queries

#[test]
fn integer_only_types() {
    assert!(FormatType::Binary.is_integer_only());
    assert!(FormatType::Octal.is_integer_only());
    assert!(FormatType::Hex.is_integer_only());
    assert!(FormatType::HexUpper.is_integer_only());
    assert!(!FormatType::Exp.is_integer_only());
    assert!(!FormatType::Fixed.is_integer_only());
}

#[test]
fn float_only_types() {
    assert!(FormatType::Exp.is_float_only());
    assert!(FormatType::ExpUpper.is_float_only());
    assert!(FormatType::Fixed.is_float_only());
    assert!(FormatType::Percent.is_float_only());
    assert!(!FormatType::Binary.is_float_only());
    assert!(!FormatType::Hex.is_float_only());
}
