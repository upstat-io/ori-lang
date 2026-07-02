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
fn test_prefix_without_tests_keyword_is_regular_function() {
    // After removing the legacy `test_` prefix convention, a function named
    // `test_something` without `tests` keyword is a regular function.
    let output = parse_module("@test_something () -> void = ();");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(
        output.module.functions.len(),
        1,
        "test_ prefix without `tests` keyword should be a regular function"
    );
    assert_eq!(
        output.module.tests.len(),
        0,
        "should not be classified as a test"
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

// Semicolon enforcement tests

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

// Contract parsing tests

#[test]
fn test_pre_contract_parses_clean() {
    let output = parse_module("@f (x: int) -> int pre(x > 0) = x;");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
    let func = &output.module.functions[0];
    assert_eq!(func.pre_contracts.len(), 1);
    assert!(func.pre_contracts[0].message.is_none());
    assert!(func.post_contracts.is_empty());
}

#[test]
fn test_pre_contract_with_message() {
    let output = parse_module(r#"@f (x: int) -> int pre(x > 0 | "x must be positive") = x;"#);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert_eq!(func.pre_contracts.len(), 1);
    assert!(func.pre_contracts[0].message.is_some());
}

#[test]
fn test_post_contract_parses_clean() {
    let output = parse_module("@f (x: int) -> int post(r -> r >= 0) = x;");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert!(func.pre_contracts.is_empty());
    assert_eq!(func.post_contracts.len(), 1);
    assert_eq!(func.post_contracts[0].params.len(), 1);
    assert!(func.post_contracts[0].message.is_none());
}

#[test]
fn test_post_contract_tuple_params() {
    let output = parse_module("@f (x: int) -> (int, int) post((a, b) -> a + b == x) = (x, 0);");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert_eq!(func.post_contracts.len(), 1);
    assert_eq!(func.post_contracts[0].params.len(), 2);
}

#[test]
fn test_post_contract_with_message() {
    let output =
        parse_module(r#"@f (x: int) -> int post(r -> r > 0 | "result must be positive") = x;"#);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert_eq!(func.post_contracts.len(), 1);
    assert!(func.post_contracts[0].message.is_some());
}

#[test]
fn test_multiple_pre_contracts() {
    let output = parse_module(
        r#"@f (a: int, b: int) -> int pre(a > 0 | "a positive") pre(b > 0 | "b positive") = a + b;"#,
    );
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert_eq!(func.pre_contracts.len(), 2);
}

#[test]
fn test_pre_and_post_contracts() {
    let output =
        parse_module("@f (a: int, b: int) -> int pre(b != 0) post(r -> r * b <= a) = a div b;");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert_eq!(func.pre_contracts.len(), 1);
    assert_eq!(func.post_contracts.len(), 1);
}

#[test]
fn test_contracts_with_newlines() {
    let source = "\
@divide (a: int, b: int) -> int
    pre(b != 0)
    post(r -> r * b <= a)
= a div b;";
    let output = parse_module(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert_eq!(func.pre_contracts.len(), 1);
    assert_eq!(func.post_contracts.len(), 1);
}

#[test]
fn test_contracts_with_guard_and_where() {
    let source = "@f <T> (x: T) -> T where T: Eq if x != x pre(true) = x;";
    let output = parse_module(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert!(!func.where_clauses.is_empty());
    assert!(func.guard.is_some());
    assert_eq!(func.pre_contracts.len(), 1);
}

#[test]
fn test_function_without_contracts_parses_clean() {
    // Regression: functions without contracts should still parse cleanly
    let output = parse_module("@f (x: int) -> int = x;");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let func = &output.module.functions[0];
    assert!(func.pre_contracts.is_empty());
    assert!(func.post_contracts.is_empty());
}

#[test]
fn test_pre_used_as_identifier_elsewhere() {
    // `pre` is not a keyword — it can be used as a variable name in the body
    let output = parse_module("@f () -> int = { let pre = 42; pre };");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
    assert!(output.module.functions[0].pre_contracts.is_empty());
}

/// Regression: BUG-07-314. grammar.ebnf `params` is newline-silent, so a
/// function whose params are stacked one-per-line (the formatter's multi-line
/// shape) must parse. Before the fix `parse_params` used `.no_newlines()` and
/// this failed with E1002.
#[test]
fn test_multiline_params_function_parses_clean() {
    let output =
        parse_module("@process_data (\n    input_data: str,\n    count: int,\n) -> int = count;");
    assert!(
        output.errors.is_empty(),
        "multi-line function params should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
    assert_eq!(output.module.functions[0].params.len, 2);
}

/// Regression: BUG-07-314. Typed-lambda params reuse `parse_params`, so a
/// lambda with stacked params must parse for the same reason as a function.
#[test]
fn test_multiline_params_typed_lambda_parses_clean() {
    let output = parse_module("@f () -> int = {\n    let g = (\n        a: int,\n        b: int,\n    ) -> int = a + b;\n    g(a: 1, b: 2)\n};");
    assert!(
        output.errors.is_empty(),
        "multi-line typed-lambda params should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
}

/// Regression: BUG-07-314 (Plan-TPR codex-F1). A clause-based function with a
/// literal-pattern param stacked one-per-line must parse — the literal-pattern
/// path also flows through `parse_params`.
#[test]
fn test_multiline_params_clause_literal_parses_clean() {
    let output = parse_module("@f (\n    0: int,\n) -> int = 1;");
    assert!(
        output.errors.is_empty(),
        "multi-line clause/literal params should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
}

/// Regression: BUG-07-314 (Plan-TPR codex-F1). A default-valued param stacked
/// one-per-line must parse — the default-value path flows through `parse_params`.
#[test]
fn test_multiline_params_default_value_parses_clean() {
    let output = parse_module("@g (\n    x: int = 42,\n) -> int = x;");
    assert!(
        output.errors.is_empty(),
        "multi-line default-value params should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.functions.len(), 1);
    assert_eq!(output.module.functions[0].params.len, 1);
}

/// Regression: BUG-07-314 (Plan-TPR-R2 codex-F2). A test declaration with
/// params stacked one-per-line must parse — the test-body parse is the 5th
/// shared `parse_params` call site (`function/mod.rs:192`).
#[test]
fn test_multiline_params_test_declaration_parses_clean() {
    let output = parse_module("@t tests @f (\n    a: int,\n) -> void = ();");
    assert!(
        output.errors.is_empty(),
        "multi-line test-declaration params should parse; errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.tests.len(), 1);
}
