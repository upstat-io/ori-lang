use ori_ir::StringInterner;

/// Parse source and return the module.
fn parse_module(source: &str) -> crate::ParseOutput {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parser = crate::Parser::new(&tokens, &interner);
    parser.parse_module()
}

#[test]
fn test_attached_single_target() {
    // Regression guard: @t tests @add () -> void = ()
    let output = parse_module("@t tests @add () -> void = ();");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.tests.len(), 1);
    assert_eq!(output.module.tests[0].targets.len(), 1);
}

#[test]
fn test_attached_multi_target() {
    // Multi-target: @t tests @a tests @b () -> void = ()
    let output = parse_module("@t tests @a tests @b () -> void = ();");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.tests.len(), 1);
    assert_eq!(output.module.tests[0].targets.len(), 2);
}

#[test]
fn test_floating_with_underscore() {
    // Floating test: @t tests _ () -> void = ()
    let output = parse_module("@t tests _ () -> void = ();");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.tests.len(), 1);
    assert!(
        output.module.tests[0].targets.is_empty(),
        "Floating test should have empty targets"
    );
}

#[test]
fn test_floating_by_name_prefix() {
    // Regression guard: test_ prefix detection without `tests` keyword
    let output = parse_module("@test_something () -> void = ();");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.tests.len(), 1);
    assert!(
        output.module.tests[0].targets.is_empty(),
        "test_ prefix test should have empty targets"
    );
}

#[test]
fn test_regular_function_not_test() {
    // Regression guard: regular function is not a test
    let output = parse_module("@add (a: int, b: int) -> int = a + b;");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
    assert_eq!(output.module.tests.len(), 0);
}

// --- Semicolon enforcement tests ---

#[test]
fn test_expression_body_requires_semicolon() {
    // Expression body without `;` should produce an error
    let output = parse_module("@f () -> int = 42");
    assert_eq!(
        output.errors.len(),
        1,
        "Expected 1 error for missing `;`, got: {:?}",
        output.errors
    );
    assert_eq!(output.errors[0].code(), ori_diagnostic::ErrorCode::E1016);
    // Function should still be parsed (error recovery)
    assert_eq!(output.module.functions.len(), 1);
}

#[test]
fn test_expression_body_with_semicolon_parses_cleanly() {
    // Expression body with `;` should parse without errors
    let output = parse_module("@f () -> int = 42;");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
}

#[test]
fn test_block_body_without_semicolon_parses_cleanly() {
    // Block body ending with `}` should NOT require `;`
    let output = parse_module("@f () -> int = { 42 }");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
}

#[test]
fn test_block_body_with_optional_semicolon() {
    // Block body with optional `;` should also parse cleanly
    let output = parse_module("@f () -> int = { 42 };");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
}
