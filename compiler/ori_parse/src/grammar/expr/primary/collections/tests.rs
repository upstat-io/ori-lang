//! Lambda-body newline-significance parse pins.

/// Parse source and return the module.
fn parse_module(source: &str) -> crate::ParseOutput {
    let interner = ori_ir::StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parser = crate::Parser::new(&tokens, &interner);
    parser.parse_module()
}

/// Regression: BUG-07-314. grammar.ebnf `lambda_tail = type "=" expression |
/// expression` is newline-silent, so an inferred-return lambda whose body is
/// pushed to the next line after `->` must parse.
#[test]
fn test_inferred_lambda_body_after_arrow_newline_parses_clean() {
    let output = parse_module("@f () -> int = {\n    let g = a ->\n        a + 1;\n    g(2)\n};");
    assert!(
        output.errors.is_empty(),
        "inferred-lambda body after an `->` newline should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
}

/// Regression: BUG-07-314. A typed lambda with an explicit return type whose
/// body follows the `=` on the next line must parse, mirroring the correct
/// function-body sibling (`expect(Eq); skip_newlines(); parse_expr()`).
#[test]
fn test_typed_lambda_body_after_eq_newline_parses_clean() {
    let output = parse_module(
        "@f () -> int = {\n    let g = (a: int) -> int =\n        a + 1;\n    g(a: 2)\n};",
    );
    assert!(
        output.errors.is_empty(),
        "typed-lambda body after a `=` newline should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
}

/// Regression: BUG-07-314. The three untyped/inferred
/// parenthesized-lambda body sites (`() -> body`, `(a, b) -> body` tuple,
/// `(a) -> body` single) each parse `parse_expr()` after `->` with no
/// preceding `skip_newlines()`; each must accept a newline-broken body.
/// Distinct from P5 (bare shorthand `a -> body`, postfix path) and P6 (typed).
#[test]
fn test_untyped_paren_lambda_bodies_after_arrow_newline_parse_clean() {
    let cases = [
        (
            "empty-params",
            "@f () -> int = {\n    let g = () ->\n        1;\n    g()\n};",
        ),
        (
            "tuple-params",
            "@f () -> int = {\n    let g = (a, b) ->\n        a + b;\n    g(1, 2)\n};",
        ),
        (
            "single-param",
            "@f () -> int = {\n    let g = (a) ->\n        a;\n    g(1)\n};",
        ),
    ];
    for (label, src) in cases {
        let output = parse_module(src);
        assert!(
            output.errors.is_empty(),
            "untyped paren-lambda body ({label}) after `->` newline should parse; errors: {:?}",
            output.errors
        );
    }
}
