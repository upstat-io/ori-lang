//! Sum-type newline-significance + grammar-conformance parse pins.

use ori_diagnostic::ErrorCode;
use ori_ir::TypeDeclKind;

/// Parse source and return the module.
fn parse_module(source: &str) -> crate::ParseOutput {
    let interner = ori_ir::StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parser = crate::Parser::new(&tokens, &interner);
    parser.parse_module()
}

/// Regression: BUG-07-314. grammar.ebnf `sum_body = variant { "|" variant }`
/// is newline-silent, so a sum type with a newline after `=` and each
/// continuation variant on its own line (no leading pipe) must parse.
#[test]
fn test_multiline_sum_no_leading_pipe_parses_clean() {
    let output = parse_module("type Event =\n    Click(x: int)\n    | KeyPress(key: char);");
    assert!(
        output.errors.is_empty(),
        "multi-line sum (no leading pipe) should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    match &output.module.types[0].kind {
        TypeDeclKind::Sum(variants) => assert_eq!(variants.len(), 2),
        other => panic!("expected Sum with 2 variants, got {other:?}"),
    }
}

/// Regression: BUG-07-314. A sum continuation variant on its own line puts a
/// newline before the `|`; the `while check(Pipe)` loop must skip it.
#[test]
fn test_multiline_sum_continuation_on_own_line_parses_clean() {
    let output = parse_module("type Event = Click(x: int)\n    | KeyPress(key: char);");
    assert!(
        output.errors.is_empty(),
        "sum continuation on its own line should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    match &output.module.types[0].kind {
        TypeDeclKind::Sum(variants) => assert_eq!(variants.len(), 2),
        other => panic!("expected Sum with 2 variants, got {other:?}"),
    }
}

/// Regression: BUG-07-314. A BARE first variant
/// (no fields) on its own line before its continuation pipe forces the
/// sum-vs-newtype disambiguation checks (`check(&Pipe)` / `check(&LParen)`) to
/// run at a `Newline`; without a `skip_newlines()` there the type mis-parses as
/// a newtype and the continuation `| KeyPress` surfaces an error. Distinct from
/// P3/P4 whose first variant carries fields.
#[test]
fn test_multiline_sum_bare_first_variant_parses_clean() {
    let output = parse_module("type Event =\n    Click\n    | KeyPress(key: char);");
    assert!(
        output.errors.is_empty(),
        "bare first sum variant on its own line should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.types.len(), 1);
    match &output.module.types[0].kind {
        TypeDeclKind::Sum(variants) => assert_eq!(variants.len(), 2),
        other => panic!("expected Sum with 2 variants, got {other:?}"),
    }
}

/// Negative pin: BUG-07-314. grammar.ebnf `sum_body = variant { "|" variant }`
/// makes `|` a SEPARATOR only, never a prefix on the first variant. The parser
/// must REJECT a leading pipe even inline — adding newline-skips must not make
/// the parser over-permissively accept a leading pipe. E1001 at the leading `|`.
#[test]
fn test_leading_pipe_on_first_variant_rejected() {
    let output = parse_module("type Event = | Click(x: int) | KeyPress(key: char);");
    assert!(
        !output.errors.is_empty(),
        "leading pipe on the first variant must be rejected"
    );
    assert!(
        output.errors.iter().any(|e| e.code() == ErrorCode::E1001),
        "expected E1001 for leading pipe; errors: {:?}",
        output.errors
    );
}
