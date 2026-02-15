use super::*;
use crate::parse;
use ori_ir::StringInterner;

fn parse_with_errors(source: &str) -> (crate::ParseOutput, StringInterner) {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let result = parse(&tokens, &interner);
    (result, interner)
}

#[test]
fn test_parsed_attrs_token_capture() {
    // Create a parser and parse attributes directly to verify token capture
    let interner = StringInterner::new();
    let source = r#"#skip("reason") #compile_fail("error")"#;
    let tokens = ori_lexer::lex(source, &interner);
    let mut parser = crate::Parser::new(&tokens, &interner);
    let mut errors = Vec::new();

    let attrs = parser.parse_attributes(&mut errors);

    // Should have captured tokens
    assert!(attrs.has_tokens(), "Expected tokens to be captured");
    assert!(!attrs.token_range.is_empty());

    // Verify we can access the captured tokens
    let captured = tokens.get_range(attrs.token_range);
    assert!(captured.len() >= 2, "Should capture multiple tokens");

    // First token should be # (Hash)
    assert!(
        matches!(captured[0].kind, TokenKind::Hash),
        "First captured token should be #"
    );
}

#[test]
fn test_parsed_attrs_no_tokens_when_no_attributes() {
    let interner = StringInterner::new();
    let source = r"def foo() -> int = 42";
    let tokens = ori_lexer::lex(source, &interner);
    let mut parser = crate::Parser::new(&tokens, &interner);
    let mut errors = Vec::new();

    let attrs = parser.parse_attributes(&mut errors);

    // Should NOT have captured tokens (no attributes)
    assert!(
        !attrs.has_tokens(),
        "Should not capture tokens when no attributes"
    );
    assert!(attrs.token_range.is_empty());
}

#[test]
fn test_parse_skip_attribute() {
    let (result, _interner) = parse_with_errors(
        r#"
#[skip("not implemented")]
@test_example () -> void = print(msg: "test")
"#,
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert_eq!(result.module.tests.len(), 1);
    let test = &result.module.tests[0];
    assert!(test.skip_reason.is_some());
}

#[test]
fn test_parse_compile_fail_attribute() {
    let (result, _interner) = parse_with_errors(
        r#"
#[compile_fail("type error")]
@test_should_fail () -> void = print(msg: "test")
"#,
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert_eq!(result.module.tests.len(), 1);
    let test = &result.module.tests[0];
    assert!(test.is_compile_fail());
    assert_eq!(test.expected_errors.len(), 1);
}

#[test]
fn test_parse_fail_attribute() {
    let (result, _interner) = parse_with_errors(
        r#"
#[fail("assertion failed")]
@test_expect_failure () -> void = panic(msg: "expected failure")
"#,
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert_eq!(result.module.tests.len(), 1);
    let test = &result.module.tests[0];
    assert!(test.fail_expected.is_some());
}

#[test]
fn test_parse_derive_attribute() {
    let (result, _interner) = parse_with_errors(
        r#"
#[derive(Eq, Clone)]
@test_with_derive () -> void = print(msg: "test")
"#,
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

#[test]
fn test_parse_unknown_attribute() {
    let (result, _interner) = parse_with_errors(
        r#"
#[unknown("value")]
@test_unknown () -> void = print(msg: "test")
"#,
    );

    // Should have an error for unknown attribute
    assert!(result.has_errors());
    assert!(result
        .errors
        .iter()
        .any(|e| e.message.contains("unknown attribute")));
}

#[test]
fn test_parse_attribute_missing_paren() {
    let (result, _interner) = parse_with_errors(
        r"
#[skip]
@test_bad () -> void = assert(cond: true)
",
    );

    // Should have an error for missing (
    assert!(result.has_errors());
}

#[test]
fn test_parse_attribute_missing_string() {
    let (result, _interner) = parse_with_errors(
        r"
#[skip()]
@test_bad () -> void = assert(cond: true)
",
    );

    // Should have an error for missing string argument
    assert!(result.has_errors());
}

#[test]
fn test_parse_multiple_attributes() {
    // Multiple attributes on same item isn't typical but parser should handle
    let (result, _interner) = parse_with_errors(
        r#"
#[skip("reason")]
#[fail("expected")]
@test_multi () -> void = print(msg: "test")
"#,
    );

    // Last attribute wins for each field
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// Tests for new bracket-less syntax per grammar.ebnf

#[test]
fn test_parse_skip_attribute_no_brackets() {
    let (result, _interner) = parse_with_errors(
        r#"
#skip("not implemented")
@test_example () -> void = print(msg: "test")
"#,
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert_eq!(result.module.tests.len(), 1);
    let test = &result.module.tests[0];
    assert!(test.skip_reason.is_some());
}

#[test]
fn test_parse_compile_fail_attribute_no_brackets() {
    let (result, _interner) = parse_with_errors(
        r#"
#compile_fail("type error")
@test_should_fail () -> void = print(msg: "test")
"#,
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert_eq!(result.module.tests.len(), 1);
    let test = &result.module.tests[0];
    assert!(test.is_compile_fail());
    assert_eq!(test.expected_errors.len(), 1);
}

#[test]
fn test_parse_fail_attribute_no_brackets() {
    let (result, _interner) = parse_with_errors(
        r#"
#fail("assertion failed")
@test_expect_failure () -> void = panic(msg: "expected failure")
"#,
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert_eq!(result.module.tests.len(), 1);
    let test = &result.module.tests[0];
    assert!(test.fail_expected.is_some());
}

#[test]
fn test_parse_derive_attribute_no_brackets() {
    let (result, _interner) = parse_with_errors(
        r"
#derive(Eq, Clone)
type Point = { x: int, y: int }
",
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

#[test]
fn test_parse_compile_fail_extended_no_brackets() {
    let (result, _interner) = parse_with_errors(
        r#"
#compile_fail(message: "type mismatch", code: "E2001")
@test_extended () -> void = print(msg: "test")
"#,
    );

    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert_eq!(result.module.tests.len(), 1);
    let test = &result.module.tests[0];
    assert!(test.is_compile_fail());
    assert_eq!(test.expected_errors.len(), 1);
}

// File-level attribute tests

#[test]
fn test_file_attr_target_parses() {
    let (result, _) = parse_with_errors("#!target(os: \"linux\")\n@main () -> void = ()");
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert!(result.module.file_attr.is_some());
}

#[test]
fn test_file_attr_cfg_parses() {
    let (result, _) = parse_with_errors("#!cfg(debug)\n@main () -> void = ()");
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert!(result.module.file_attr.is_some());
}

#[test]
fn test_file_attr_none_when_absent() {
    let (result, _) = parse_with_errors("@main () -> void = ()");
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert!(result.module.file_attr.is_none());
}

#[test]
fn test_file_attr_does_not_consume_item_attr() {
    let (result, _) = parse_with_errors("#skip(\"reason\")\n@test_foo () -> void = ()");
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    assert!(
        result.module.file_attr.is_none(),
        "item-level #skip should not be consumed as file attribute"
    );
}

#[test]
fn test_file_attr_invalid_kind_reports_error() {
    let (result, _) = parse_with_errors("#!derive(Eq)\n@main () -> void = ()");
    assert!(result.has_errors());
    assert!(result
        .errors
        .iter()
        .any(|e| e.message.contains("not valid as a file-level attribute")));
}
