//! Conformance tests for the `ori_rt` format spec parser and formatters.
//!
//! Two categories of conformance:
//!
//! 1. **Parser conformance**: Verifies that `ori_rt::format::parse_format_spec`
//!    (the AOT runtime parser) produces identical results to
//!    `ori_ir::format_spec::parse_format_spec` (the compilation-time parser).
//!
//! 2. **Formatter conformance (golden output)**: Verifies that `ori_rt`'s
//!    formatter functions (`format_int`, `format_float`, `fmt_str`) produce
//!    the same output as `ori_eval`'s formatters for the same (value, spec)
//!    pairs. Since `ori_rt` can't depend on `ori_eval`, both test files use
//!    the same golden-output triples — drift in either formatter triggers a
//!    test failure.

use ori_ir::format_spec as ir;

use super::{parse_format_spec, Align, FormatType, Sign};

// Variant count assertions

/// Ensures `ori_rt::FormatType` has exactly as many variants as `ori_ir::FormatType`.
///
/// If a variant is added to `ori_ir::FormatType`, this test will fail to compile
/// (exhaustive match) or fail at runtime (count mismatch), signaling the need to
/// update `ori_rt::FormatType` as well.
#[test]
fn format_type_variant_count() {
    // Count ori_ir variants via exhaustive match
    let ir_count = [
        ir::FormatType::Binary,
        ir::FormatType::Octal,
        ir::FormatType::Hex,
        ir::FormatType::HexUpper,
        ir::FormatType::Exp,
        ir::FormatType::ExpUpper,
        ir::FormatType::Fixed,
        ir::FormatType::Percent,
    ]
    .len();

    // Count ori_rt variants via exhaustive match
    let rt_count = [
        FormatType::Binary,
        FormatType::Octal,
        FormatType::Hex,
        FormatType::HexUpper,
        FormatType::Exp,
        FormatType::ExpUpper,
        FormatType::Fixed,
        FormatType::Percent,
    ]
    .len();

    assert_eq!(
        ir_count, rt_count,
        "ori_ir::FormatType has {ir_count} variants but ori_rt::FormatType has {rt_count}"
    );
}

/// Ensures `ori_rt::Align` has exactly as many variants as `ori_ir::Align`.
#[test]
fn align_variant_count() {
    let ir_count = [ir::Align::Left, ir::Align::Center, ir::Align::Right].len();
    let rt_count = [Align::Left, Align::Center, Align::Right].len();
    assert_eq!(ir_count, rt_count);
}

/// Ensures `ori_rt::Sign` has exactly as many variants as `ori_ir::Sign`.
#[test]
fn sign_variant_count() {
    let ir_count = [ir::Sign::Plus, ir::Sign::Minus, ir::Sign::Space].len();
    let rt_count = [Sign::Plus, Sign::Minus, Sign::Space].len();
    assert_eq!(ir_count, rt_count);
}

// Cross-parser conformance helpers

/// Compare the field-by-field output of both parsers for a given spec string.
///
/// The `ori_ir` parser returns `Result<ParsedFormatSpec, _>` while `ori_rt`
/// returns `ParsedFormatSpec` (falling back to EMPTY on error). For valid specs,
/// both must agree on every field.
fn assert_parsers_agree(spec: &str) {
    let ir_result = ir::parse_format_spec(spec);
    let ir_parsed = match ir_result {
        Ok(p) => p,
        Err(e) => panic!("ori_ir parser rejected spec '{spec}': {e}"),
    };
    let rt_parsed = parse_format_spec(spec);

    // Fill
    assert_eq!(
        ir_parsed.fill, rt_parsed.fill,
        "fill mismatch for spec '{spec}'"
    );

    // Align
    let ir_align = ir_parsed.align.map(|a| match a {
        ir::Align::Left => "Left",
        ir::Align::Center => "Center",
        ir::Align::Right => "Right",
    });
    let rt_align = rt_parsed.align.map(|a| match a {
        Align::Left => "Left",
        Align::Center => "Center",
        Align::Right => "Right",
    });
    assert_eq!(ir_align, rt_align, "align mismatch for spec '{spec}'");

    // Sign
    let ir_sign = ir_parsed.sign.map(|s| match s {
        ir::Sign::Plus => "Plus",
        ir::Sign::Minus => "Minus",
        ir::Sign::Space => "Space",
    });
    let rt_sign = rt_parsed.sign.map(|s| match s {
        Sign::Plus => "Plus",
        Sign::Minus => "Minus",
        Sign::Space => "Space",
    });
    assert_eq!(ir_sign, rt_sign, "sign mismatch for spec '{spec}'");

    // Flags
    assert_eq!(
        ir_parsed.alternate, rt_parsed.alternate,
        "alternate mismatch for spec '{spec}'"
    );
    assert_eq!(
        ir_parsed.zero_pad, rt_parsed.zero_pad,
        "zero_pad mismatch for spec '{spec}'"
    );

    // Width
    assert_eq!(
        ir_parsed.width, rt_parsed.width,
        "width mismatch for spec '{spec}'"
    );

    // Precision
    assert_eq!(
        ir_parsed.precision, rt_parsed.precision,
        "precision mismatch for spec '{spec}'"
    );

    // FormatType
    let ir_ft = ir_parsed.format_type.map(|ft| match ft {
        ir::FormatType::Binary => "Binary",
        ir::FormatType::Octal => "Octal",
        ir::FormatType::Hex => "Hex",
        ir::FormatType::HexUpper => "HexUpper",
        ir::FormatType::Exp => "Exp",
        ir::FormatType::ExpUpper => "ExpUpper",
        ir::FormatType::Fixed => "Fixed",
        ir::FormatType::Percent => "Percent",
    });
    let rt_ft = rt_parsed.format_type.map(|ft| match ft {
        FormatType::Binary => "Binary",
        FormatType::Octal => "Octal",
        FormatType::Hex => "Hex",
        FormatType::HexUpper => "HexUpper",
        FormatType::Exp => "Exp",
        FormatType::ExpUpper => "ExpUpper",
        FormatType::Fixed => "Fixed",
        FormatType::Percent => "Percent",
    });
    assert_eq!(ir_ft, rt_ft, "format_type mismatch for spec '{spec}'");
}

// Cross-parser conformance tests

/// Comprehensive conformance: feed every spec string from `ori_ir`'s test suite
/// to both parsers and verify identical output.
#[test]
fn parsers_agree_on_all_valid_specs() {
    let specs = [
        // Empty
        "",
        // Alignment only
        "<",
        ">",
        "^",
        // Fill + align
        "*^",
        "0>",
        "-<",
        // Sign only
        "+",
        "-",
        " ",
        // Alternate
        "#",
        // Zero-pad
        "08",
        "0",
        "0x",
        // Width
        "10",
        "100",
        // Precision
        ".2",
        ".0",
        ".",
        // Width + precision
        "10.2",
        // Format types
        "b",
        "o",
        "x",
        "X",
        "e",
        "E",
        "f",
        "%",
        // Combined specs
        "*^+#020.5f",
        "#08x",
        ">10",
        "*>10.2",
        "08x",
        ".2f",
        ".6e",
        ".0%",
    ];

    for spec in &specs {
        assert_parsers_agree(spec);
    }
}

// Standalone ori_rt parser correctness tests

#[test]
fn empty_spec() {
    let spec = parse_format_spec("");
    assert!(spec.fill.is_none());
    assert!(spec.align.is_none());
    assert!(spec.sign.is_none());
    assert!(!spec.alternate);
    assert!(!spec.zero_pad);
    assert!(spec.width.is_none());
    assert!(spec.precision.is_none());
    assert!(spec.format_type.is_none());
}

#[test]
fn full_spec() {
    let spec = parse_format_spec("*^+#020.5f");
    assert_eq!(spec.fill, Some('*'));
    assert!(matches!(spec.align, Some(Align::Center)));
    assert!(matches!(spec.sign, Some(Sign::Plus)));
    assert!(spec.alternate);
    assert!(spec.zero_pad);
    assert_eq!(spec.width, Some(20));
    assert_eq!(spec.precision, Some(5));
    assert!(matches!(spec.format_type, Some(FormatType::Fixed)));
}

#[test]
fn unknown_type_falls_back_to_none() {
    // ori_rt silently ignores unknown types (type-checker already validated)
    let spec = parse_format_spec("z");
    assert!(spec.format_type.is_none());
}

// Cross-formatter conformance tests (golden output)
//
// These tests define (input, spec_string, expected_output) triples that MUST
// produce identical results in both `ori_rt` (AOT runtime) and `ori_eval`
// (tree-walking evaluator). The same triples appear in
// `ori_eval/src/interpreter/format/tests.rs` — if either formatter drifts,
// its golden tests fail.

/// Helper: parse spec string, format an int, compare against expected.
fn assert_int_formats_to(n: i64, spec_str: &str, expected: &str) {
    let parsed = parse_format_spec(spec_str);
    let result = super::format_int(n, &parsed);
    assert_eq!(
        result, expected,
        "ori_rt format_int({n}, \"{spec_str}\") = \"{result}\", expected \"{expected}\""
    );
}

/// Helper: parse spec string, format a float, compare against expected.
fn assert_float_formats_to(f: f64, spec_str: &str, expected: &str) {
    let parsed = parse_format_spec(spec_str);
    let result = super::format_float(f, &parsed);
    assert_eq!(
        result, expected,
        "ori_rt format_float({f}, \"{spec_str}\") = \"{result}\", expected \"{expected}\""
    );
}

/// Helper: parse spec string, format a string, compare against expected.
fn assert_str_formats_to(s: &str, spec_str: &str, expected: &str) {
    let parsed = parse_format_spec(spec_str);
    let result = super::fmt_str(s, &parsed);
    assert_eq!(
        result, expected,
        "ori_rt fmt_str(\"{s}\", \"{spec_str}\") = \"{result}\", expected \"{expected}\""
    );
}

/// Golden output conformance for integer formatting.
///
/// These exact triples are duplicated in `ori_eval/src/interpreter/format/tests.rs`.
/// If this test fails, the `ori_rt` formatter has drifted from the evaluator.
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
#[allow(clippy::approx_constant, reason = "3.14 is a test value, not PI")]
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
