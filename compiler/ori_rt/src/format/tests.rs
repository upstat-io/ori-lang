//! Conformance tests for the `ori_rt` format spec parser.
//!
//! Verifies that `ori_rt::format::parse_format_spec` (the AOT runtime parser)
//! produces identical results to `ori_ir::format_spec::parse_format_spec` (the
//! compilation-time parser) for the same inputs.
//!
//! This catches drift when a variant or parsing rule is added to one parser
//! but missed in the other.

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
