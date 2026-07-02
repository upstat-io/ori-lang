//! Trait-method multi-line params parse pin.

/// Parse source and return the module.
fn parse_module(source: &str) -> crate::ParseOutput {
    let interner = ori_ir::StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parser = crate::Parser::new(&tokens, &interner);
    parser.parse_module()
}

/// Regression: BUG-07-314. A trait-method signature with
/// params stacked one-per-line must parse — trait methods are one of
/// `parse_params`' shared call sites.
#[test]
fn test_multiline_params_trait_method_parses_clean() {
    let output =
        parse_module("trait Foo {\n    @m (\n        a: int,\n        b: int,\n    ) -> int\n}");
    assert!(
        output.errors.is_empty(),
        "multi-line trait-method params should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.traits.len(), 1);
}
