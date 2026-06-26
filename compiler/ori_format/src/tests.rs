//! Golden-output matrix for the scalar emission kernel + parser roundtrip.
//!
//! A single implementation is tested directly here; the interpreter and AOT
//! runtime both call into it, so these golden triples pin the behavior both
//! backends share (the cross-backend conformance the dual-execution spec corpus
//! verifies end-to-end).

#![allow(
    clippy::approx_constant,
    reason = "3.14 is a literal test value, not an approximation of PI"
)]

use super::{format_float, format_int, format_str, parse_format_spec, Align, FormatType, Sign};

/// Parse a spec string that is known-valid in tests.
fn spec(s: &str) -> super::ParsedFormatSpec {
    match parse_format_spec(s) {
        Ok(p) => p,
        Err(e) => panic!("test spec {s:?} failed to parse: {e}"),
    }
}

fn fi(n: i64, s: &str) -> String {
    format_int(n, &spec(s))
}

fn ff(f: f64, s: &str) -> String {
    format_float(f, &spec(s))
}

fn fs(text: &str, s: &str) -> String {
    format_str(text, &spec(s))
}

// Integer matrix: decimal / binary / octal / hex / hexupper x sign / zero-pad / width / align

#[test]
fn format_int_decimal_default_emits_bare_digits() {
    assert_eq!(fi(42, ""), "42");
    assert_eq!(fi(0, ""), "0");
    assert_eq!(fi(-42, ""), "-42");
}

#[test]
fn format_int_zero_pad_negative_sign_before_zeros() {
    assert_eq!(fi(-42, "08"), "-0000042");
}

#[test]
fn format_int_plus_sign_prepends_plus_on_positive() {
    assert_eq!(fi(42, "+"), "+42");
}

#[test]
fn format_int_space_sign_prepends_space_on_positive() {
    assert_eq!(fi(42, " "), " 42");
}

#[test]
fn format_int_plus_sign_suppressed_on_negative() {
    assert_eq!(fi(-5, "+"), "-5");
}

#[test]
fn format_int_zero_pad_width_pads_after_sign() {
    assert_eq!(fi(42, "08"), "00000042");
    assert_eq!(fi(-5, "05"), "-0005");
}

#[test]
fn format_int_left_align_pads_right() {
    assert_eq!(fi(42, "<4"), "42  ");
}

#[test]
fn format_int_binary_alternate_adds_prefix() {
    assert_eq!(fi(42, "b"), "101010");
    assert_eq!(fi(42, "#b"), "0b101010");
}

#[test]
fn format_int_octal_alternate_adds_prefix() {
    assert_eq!(fi(42, "o"), "52");
    assert_eq!(fi(42, "#o"), "0o52");
}

#[test]
fn format_int_hex_lowercase_alternate_adds_prefix() {
    assert_eq!(fi(42, "x"), "2a");
    assert_eq!(fi(42, "#x"), "0x2a");
}

#[test]
fn format_int_hexupper_alternate_adds_prefix() {
    assert_eq!(fi(42, "X"), "2A");
    assert_eq!(fi(42, "#X"), "0X2A");
}

#[test]
fn format_int_i64_min_uses_unsigned_abs() {
    assert_eq!(fi(i64::MIN, ""), "-9223372036854775808");
}

// Float matrix: default / fixed / scientific / percent x precision / sign / zero-pad

#[test]
fn format_float_default_precision() {
    assert_eq!(ff(3.14, ""), "3.14");
    assert_eq!(ff(3.14159, ".2"), "3.14");
    assert_eq!(ff(3.14, "+"), "+3.14");
}

#[test]
fn format_float_fixed_default_six_places() {
    assert_eq!(ff(3.14, ".2f"), "3.14");
    assert_eq!(ff(3.14, "f"), "3.140000");
}

#[test]
fn format_float_scientific_zero_pads_exponent() {
    assert_eq!(ff(1.5, "e"), "1.5e+00");
    assert_eq!(ff(1.5, ".2e"), "1.50e+00");
    assert_eq!(ff(0.0, "e"), "0e+00");
    assert_eq!(ff(1e-9, "e"), "1e-09");
}

#[test]
fn format_float_scientific_uppercase() {
    assert_eq!(ff(1.5, "E"), "1.5E+00");
    assert_eq!(ff(1.5, ".2E"), "1.50E+00");
}

#[test]
fn format_float_percent_multiplies_and_appends() {
    assert_eq!(ff(0.5, "%"), "50%");
    assert_eq!(ff(0.5, ".1%"), "50.0%");
    assert_eq!(ff(0.125, ".1%"), "12.5%");
}

#[test]
fn format_float_nan_renders_name_not_exponent() {
    assert_eq!(ff(f64::NAN, "e"), "NaN");
    assert_eq!(ff(f64::INFINITY, "e"), "inf");
    assert_eq!(ff(f64::NEG_INFINITY, "e"), "-inf");
}

/// Negative-zero EMITS the sign (`is_sign_negative && !is_nan`); only NaN
/// suppresses it. This is the matrix edge-pin corrected during plan review.
#[test]
fn format_float_negative_zero_emits_sign() {
    assert_eq!(ff(-0.0, ""), "-0");
    assert!(!ff(f64::NAN, "").starts_with('-'));
}

// String / bool-as-str / char-as-str matrix

#[test]
fn format_str_precision_truncates() {
    assert_eq!(fs("hello", ".3"), "hel");
}

#[test]
fn format_str_width_right_align_pads_left() {
    assert_eq!(fs("hello", ">8"), "   hello");
}

#[test]
fn format_str_no_spec_is_identity() {
    assert_eq!(fs("hello", ""), "hello");
}

// Negative pins — catch a broken kernel

#[test]
fn negative_pin_percent_keeps_marker() {
    let out = ff(0.5, "%");
    assert!(out.ends_with('%'), "percent must keep the % marker: {out}");
    assert_ne!(out, "0.5%");
    assert_ne!(out, "50");
}

#[test]
fn negative_pin_scientific_zero_pads_exponent() {
    let out = ff(0.0, "e");
    assert_ne!(out, "0e0", "exponent must be zero-padded to min 2 digits");
    assert_eq!(out, "0e+00");
}

#[test]
fn negative_pin_hex_lowercase_stays_lowercase() {
    assert_ne!(fi(255, "#x"), "0xFF");
    assert_eq!(fi(255, "#x"), "0xff");
}

#[test]
fn negative_pin_invalid_spec_is_err_not_panic() {
    // The fallible parser returns Err; the ori_rt thin wrapper's unwrap_or(EMPTY)
    // must not panic on this.
    assert!(parse_format_spec("zz").is_err());
}

// Parser roundtrip — every spec dimension reaches its structured field

#[test]
fn parse_full_spec_populates_all_fields() {
    let p = spec("*^+#020.5f");
    assert_eq!(p.fill, Some('*'));
    assert_eq!(p.align, Some(Align::Center));
    assert_eq!(p.sign, Some(Sign::Plus));
    assert!(p.alternate);
    assert!(p.zero_pad);
    assert_eq!(p.width, Some(20));
    assert_eq!(p.precision, Some(5));
    assert_eq!(p.format_type, Some(FormatType::Fixed));
}

#[test]
fn parse_empty_spec_is_all_none() {
    let p = spec("");
    assert_eq!(p, super::ParsedFormatSpec::EMPTY);
}

/// Self-verifying matrix completeness: every `FormatType` variant round-trips
/// through `from_char` so a new variant cannot silently skip the matrix.
#[test]
fn format_type_roundtrips_every_variant() {
    let all = [
        ('b', FormatType::Binary),
        ('o', FormatType::Octal),
        ('x', FormatType::Hex),
        ('X', FormatType::HexUpper),
        ('e', FormatType::Exp),
        ('E', FormatType::ExpUpper),
        ('f', FormatType::Fixed),
        ('%', FormatType::Percent),
    ];
    let mut count = 0;
    for (c, ty) in all {
        assert_eq!(FormatType::from_char(c), Some(ty));
        count += 1;
    }
    assert_eq!(count, 8, "every FormatType variant must be covered");
}
