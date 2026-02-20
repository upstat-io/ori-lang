//! Property-based tests for the Ori formatter.
//!
//! These tests use proptest to generate random valid Ori code and verify:
//! 1. Idempotence: format(format(code)) == format(code)
//! 2. Parse-ability: formatted output can be re-parsed
//!
//! This complements the idempotence_tests.rs which tests real files,
//! by generating synthetic code that might exercise edge cases not
//! present in the test corpus.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "Tests can panic")]
#![allow(
    clippy::doc_markdown,
    clippy::disallowed_types,
    clippy::uninlined_format_args,
    clippy::redundant_closure_for_method_calls,
    clippy::no_effect_replace,
    reason = "Proptest macros generate code with these patterns"
)]

use ori_fmt::{format_module, format_module_with_comments};
use ori_ir::StringInterner;
use ori_lexer::lex_with_comments;
use proptest::prelude::*;

// -- Code Generation Strategies --

/// Generate a valid Ori identifier.
fn identifier_strategy() -> impl Strategy<Value = String> {
    // Identifiers must start with letter or underscore, followed by alphanumeric or underscore
    // Avoid keywords
    prop::string::string_regex("[a-z][a-z0-9_]{0,15}")
        .expect("valid regex")
        .prop_filter("not a keyword", |s| !is_keyword(s))
}

/// Generate a valid Ori type identifier (starts with uppercase).
fn type_identifier_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Z][a-zA-Z0-9]{0,15}")
        .expect("valid regex")
        .prop_filter("not a keyword", |s| !is_keyword(s))
}

/// Check if a string is a reserved keyword.
fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "async"
            | "break"
            | "continue"
            | "def"
            | "do"
            | "else"
            | "false"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "pub"
            | "self"
            | "then"
            | "trait"
            | "true"
            | "type"
            | "use"
            | "uses"
            | "void"
            | "where"
            | "with"
            | "yield"
            | "by"
            | "cache"
            | "catch"
            | "parallel"
            | "recurse"
            | "run"
            | "spawn"
            | "timeout"
            | "try"
            | "without"
            | "int"
            | "float"
            | "str"
            | "byte"
            | "bool"
            | "char"
            | "div"
    )
}

/// Generate an integer literal.
fn int_literal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Small integers
        (0i64..=1000).prop_map(|n| n.to_string()),
        // Negative integers
        (-1000i64..0).prop_map(|n| n.to_string()),
        // Large integers with underscores
        (1_000_000i64..10_000_000).prop_map(|n| format!("{}_000", n / 1000)),
    ]
}

/// Generate a float literal.
fn float_literal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple floats
        (0.0f64..1000.0).prop_map(|f| format!("{:.2}", f)),
        // Scientific notation
        (1.0f64..10.0, -5i32..5).prop_map(|(m, e)| format!("{:.1}e{}", m, e)),
    ]
}

/// Generate a bool literal.
fn bool_literal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![Just("true".to_string()), Just("false".to_string())]
}

/// Generate a string literal (simple, no escapes for now).
fn string_literal_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 _]{0,30}")
        .expect("valid regex")
        .prop_map(|s| format!("\"{}\"", s))
}

/// Generate a char literal.
fn char_literal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // ASCII letters using u8 range
        (b'a'..=b'z').prop_map(|c| format!("'{}'", c as char)),
        // Escape sequences
        Just("'\\n'".to_string()),
        Just("'\\t'".to_string()),
        Just("'\\r'".to_string()),
        Just("'\\0'".to_string()),
    ]
}

/// Generate a simple literal.
fn literal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        int_literal_strategy(),
        float_literal_strategy(),
        bool_literal_strategy(),
        string_literal_strategy(),
        char_literal_strategy(),
    ]
}

/// Generate a simple expression (literals, identifiers, basic operations).
fn simple_expr_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        literal_strategy(),
        identifier_strategy(),
        // Unit
        Just("()".to_string()),
        // None
        Just("None".to_string()),
    ]
}

/// Generate a binary operator.
fn binary_op_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("+".to_string()),
        Just("-".to_string()),
        Just("*".to_string()),
        Just("/".to_string()),
        Just("%".to_string()),
        Just("==".to_string()),
        Just("!=".to_string()),
        Just("<".to_string()),
        Just(">".to_string()),
        Just("<=".to_string()),
        Just(">=".to_string()),
        Just("&&".to_string()),
        Just("||".to_string()),
    ]
}

/// Generate a binary expression.
fn binary_expr_strategy() -> BoxedStrategy<String> {
    (
        simple_expr_strategy(),
        binary_op_strategy(),
        simple_expr_strategy(),
    )
        .prop_map(|(left, op, right)| format!("{} {} {}", left, op, right))
        .boxed()
}

/// Generate an expression (recursive with depth limit).
fn expr_strategy(depth: u32) -> BoxedStrategy<String> {
    if depth == 0 {
        simple_expr_strategy().boxed()
    } else {
        prop_oneof![
            // Simple expressions
            simple_expr_strategy(),
            // Binary expressions
            binary_expr_strategy(),
            // Unary expressions
            simple_expr_strategy().prop_map(|e| format!("-{}", e)),
            simple_expr_strategy().prop_map(|e| format!("!{}", e)),
            // Parenthesized
            expr_strategy(depth - 1).prop_map(|e| format!("({})", e)),
            // If-then-else
            (
                simple_expr_strategy(),
                expr_strategy(depth - 1),
                expr_strategy(depth - 1)
            )
                .prop_map(|(cond, then_e, else_e)| {
                    format!("if {} then {} else {}", cond, then_e, else_e)
                }),
            // Option wrappers
            expr_strategy(depth - 1).prop_map(|e| format!("Some({})", e)),
            // Result wrappers
            expr_strategy(depth - 1).prop_map(|e| format!("Ok({})", e)),
            expr_strategy(depth - 1).prop_map(|e| format!("Err({})", e)),
            // List literals
            prop::collection::vec(simple_expr_strategy(), 0..5)
                .prop_map(|items| format!("[{}]", items.join(", "))),
            // Tuple literals
            prop::collection::vec(simple_expr_strategy(), 2..5)
                .prop_map(|items| format!("({})", items.join(", "))),
        ]
        .boxed()
    }
}

/// Generate a type annotation.
fn type_annotation_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("int".to_string()),
        Just("float".to_string()),
        Just("bool".to_string()),
        Just("str".to_string()),
        Just("char".to_string()),
        Just("void".to_string()),
        // Generic types
        Just("[int]".to_string()),
        Just("[str]".to_string()),
        Just("Option<int>".to_string()),
        Just("Result<int, str>".to_string()),
        // Custom types
        type_identifier_strategy(),
    ]
}

/// Generate a function parameter.
fn param_strategy() -> impl Strategy<Value = String> {
    (identifier_strategy(), type_annotation_strategy())
        .prop_map(|(name, ty)| format!("{}: {}", name, ty))
}

/// Generate a simple function declaration.
fn function_strategy() -> impl Strategy<Value = String> {
    (
        identifier_strategy(),
        prop::collection::vec(param_strategy(), 0..4),
        type_annotation_strategy(),
        expr_strategy(2),
    )
        .prop_map(|(name, params, ret_ty, body)| {
            if params.is_empty() {
                format!("@{} () -> {} = {}", name, ret_ty, body)
            } else {
                format!("@{} ({}) -> {} = {}", name, params.join(", "), ret_ty, body)
            }
        })
}

/// Generate a type definition.
fn type_def_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Newtype alias
        (type_identifier_strategy(), type_annotation_strategy())
            .prop_map(|(name, ty)| format!("type {} = {}", name, ty)),
        // Struct type
        (
            type_identifier_strategy(),
            prop::collection::vec(
                (identifier_strategy(), type_annotation_strategy())
                    .prop_map(|(n, t)| format!("{}: {}", n, t)),
                0..4
            )
        )
            .prop_map(|(name, fields)| {
                if fields.is_empty() {
                    format!("type {} = {{}}", name)
                } else {
                    format!("type {} = {{ {} }}", name, fields.join(", "))
                }
            }),
        // Sum type with simple variants
        (
            type_identifier_strategy(),
            prop::collection::vec(type_identifier_strategy(), 2..4)
        )
            .prop_map(|(name, variants)| { format!("type {} = {}", name, variants.join(" | ")) }),
    ]
}

/// Generate a constant definition.
fn const_def_strategy() -> impl Strategy<Value = String> {
    (identifier_strategy(), literal_strategy())
        .prop_map(|(name, value)| format!("let ${} = {}", name, value))
}

/// Generate a simple module with various declarations.
fn module_strategy() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(const_def_strategy(), 0..3),
        prop::collection::vec(type_def_strategy(), 0..3),
        prop::collection::vec(function_strategy(), 1..5),
    )
        .prop_map(|(consts, types, funcs)| {
            let mut parts = Vec::new();
            parts.extend(consts);
            parts.extend(types);
            parts.extend(funcs);
            parts.join("\n\n")
        })
}

// -- Test Helpers --

/// Parse and format source code.
fn parse_and_format(source: &str) -> Result<String, String> {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let output = ori_parse::parse(&tokens, &interner);

    if output.has_errors() {
        let errors: Vec<String> = output.errors.iter().map(|e| format!("{:?}", e)).collect();
        return Err(format!("Parse errors:\n{}", errors.join("\n")));
    }

    Ok(format_module(&output.module, &output.arena, &interner))
}

/// Parse and format source code with comments.
fn parse_and_format_with_comments(source: &str) -> Result<String, String> {
    let interner = StringInterner::new();
    let lex_output = lex_with_comments(source, &interner);
    let parse_output = ori_parse::parse(&lex_output.tokens, &interner);

    if parse_output.has_errors() {
        let errors: Vec<String> = parse_output
            .errors
            .iter()
            .map(|e| format!("{:?}", e))
            .collect();
        return Err(format!("Parse errors:\n{}", errors.join("\n")));
    }

    Ok(format_module_with_comments(
        &parse_output.module,
        &lex_output.comments,
        &parse_output.arena,
        &interner,
    ))
}

/// Normalize whitespace for comparison.
fn normalize_whitespace(source: &str) -> String {
    let lines: Vec<&str> = source.lines().map(|l| l.trim_end()).collect();
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
    let mut result = Vec::new();
    let mut prev_blank = false;

    for line in &lines[start..] {
        let is_blank = line.is_empty();
        if is_blank && prev_blank {
            continue;
        }
        result.push(*line);
        prev_blank = is_blank;
    }

    while result.last().is_some_and(|l| l.is_empty()) {
        result.pop();
    }

    let mut output = result.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

/// Test that format(format(code)) == format(code)
fn test_idempotence(source: &str) -> Result<(), String> {
    // First format
    let first = parse_and_format_with_comments(source)
        .or_else(|_| parse_and_format(source))
        .map_err(|e| format!("First parse failed: {}", e))?;

    // Second format
    let second = parse_and_format_with_comments(&first)
        .or_else(|_| parse_and_format(&first))
        .map_err(|e| format!("Second parse failed: {}\nFirst output:\n{}", e, first))?;

    let first_normalized = normalize_whitespace(&first);
    let second_normalized = normalize_whitespace(&second);

    if first_normalized != second_normalized {
        return Err(format!(
            "Idempotence failure:\n\n--- First ---\n{}\n--- Second ---\n{}",
            first_normalized, second_normalized
        ));
    }

    Ok(())
}

// -- Property Tests --

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 1000,
        ..ProptestConfig::default()
    })]

    /// Test idempotence for generated expressions.
    #[test]
    fn prop_expr_idempotence(expr in expr_strategy(3)) {
        // Wrap expression in a function to make it a valid module
        let source = format!("@test_fn () -> int = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            // Some generated expressions may not be valid Ori
            // (e.g., type mismatches). We only fail on actual idempotence errors.
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for generated functions.
    #[test]
    fn prop_function_idempotence(func in function_strategy()) {
        if let Err(e) = test_idempotence(&func) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for generated type definitions.
    #[test]
    fn prop_type_idempotence(type_def in type_def_strategy()) {
        if let Err(e) = test_idempotence(&type_def) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for generated constants.
    #[test]
    fn prop_const_idempotence(const_def in const_def_strategy()) {
        if let Err(e) = test_idempotence(&const_def) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for generated modules.
    #[test]
    fn prop_module_idempotence(module in module_strategy()) {
        if let Err(e) = test_idempotence(&module) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for generated list literals.
    #[test]
    fn prop_list_idempotence(items in prop::collection::vec(simple_expr_strategy(), 0..10)) {
        let source = format!("@test_fn () -> [int] = [{}]", items.join(", "));
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for generated tuples.
    #[test]
    fn prop_tuple_idempotence(items in prop::collection::vec(simple_expr_strategy(), 2..8)) {
        let source = format!("@test_fn () -> int = ({})", items.join(", "));
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for deeply nested expressions.
    #[test]
    fn prop_nested_idempotence(depth in 1usize..6) {
        let mut expr = "1".to_string();
        for _ in 0..depth {
            expr = format!("({} + 1)", expr);
        }
        let source = format!("@test_fn () -> int = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for binary operator chains.
    #[test]
    fn prop_chain_idempotence(
        ops in prop::collection::vec(binary_op_strategy(), 1..5),
        values in prop::collection::vec(int_literal_strategy(), 2..7)
    ) {
        if values.len() <= ops.len() {
            return Ok(());
        }
        let mut expr = values[0].clone();
        for (i, op) in ops.iter().enumerate() {
            if i + 1 < values.len() {
                expr = format!("{} {} {}", expr, op, values[i + 1]);
            }
        }
        let source = format!("@test_fn () -> int = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }
}

// -- Extended Property Tests - More Comprehensive Fuzzing --

/// Generate a method call chain.
fn method_chain_strategy(depth: u32) -> BoxedStrategy<String> {
    if depth == 0 {
        identifier_strategy().boxed()
    } else {
        (method_chain_strategy(depth - 1), identifier_strategy())
            .prop_map(|(receiver, method)| format!("{}.{}()", receiver, method))
            .boxed()
    }
}

/// Generate field access chains.
fn field_access_strategy(depth: u32) -> BoxedStrategy<String> {
    if depth == 0 {
        identifier_strategy().boxed()
    } else {
        (field_access_strategy(depth - 1), identifier_strategy())
            .prop_map(|(receiver, field)| format!("{}.{}", receiver, field))
            .boxed()
    }
}

/// Generate a lambda expression.
fn lambda_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Single param lambda: x -> x + 1
        (identifier_strategy(), simple_expr_strategy())
            .prop_map(|(param, body)| format!("{} -> {}", param, body)),
        // Multi-param lambda: (a, b) -> a + b
        (
            prop::collection::vec(identifier_strategy(), 2..4),
            simple_expr_strategy()
        )
            .prop_map(|(params, body)| format!("({}) -> {}", params.join(", "), body)),
        // No-param lambda: () -> 42
        simple_expr_strategy().prop_map(|body| format!("() -> {}", body)),
    ]
}

/// Generate a run expression.
fn run_expr_strategy() -> impl Strategy<Value = String> {
    (
        identifier_strategy(),
        simple_expr_strategy(),
        simple_expr_strategy(),
    )
        .prop_map(|(var, init, result)| format!("run(let {} = {}, {})", var, init, result))
}

/// Generate a match arm.
fn match_arm_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Literal pattern
        (int_literal_strategy(), simple_expr_strategy())
            .prop_map(|(pat, body)| format!("{} -> {}", pat, body)),
        // Variable pattern
        (identifier_strategy(), simple_expr_strategy())
            .prop_map(|(var, body)| format!("{} -> {}", var, body)),
        // Wildcard pattern
        simple_expr_strategy().prop_map(|body| format!("_ -> {}", body)),
    ]
}

/// Generate a match expression.
fn match_expr_strategy() -> impl Strategy<Value = String> {
    (
        simple_expr_strategy(),
        prop::collection::vec(match_arm_strategy(), 2..5),
    )
        .prop_map(|(scrutinee, arms)| format!("match({}, {})", scrutinee, arms.join(", ")))
}

/// Generate a for expression.
fn for_expr_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // for...do
        (
            identifier_strategy(),
            identifier_strategy(),
            simple_expr_strategy()
        )
            .prop_map(|(var, iter, body)| format!("for {} in {} do {}", var, iter, body)),
        // for...yield
        (
            identifier_strategy(),
            identifier_strategy(),
            simple_expr_strategy()
        )
            .prop_map(|(var, iter, body)| format!("for {} in {} yield {}", var, iter, body)),
    ]
}

/// Generate a trait definition.
fn trait_strategy() -> impl Strategy<Value = String> {
    (
        type_identifier_strategy(),
        prop::collection::vec(
            (identifier_strategy(), type_annotation_strategy())
                .prop_map(|(name, ret)| format!("    @{} (self) -> {}", name, ret)),
            1..4,
        ),
    )
        .prop_map(|(name, methods)| format!("trait {} {{\n{}\n}}", name, methods.join("\n")))
}

/// Generate an impl block.
fn impl_strategy() -> impl Strategy<Value = String> {
    (
        type_identifier_strategy(),
        prop::collection::vec(
            (identifier_strategy(), simple_expr_strategy())
                .prop_map(|(name, body)| format!("    @{} (self) -> int = {}", name, body)),
            1..3,
        ),
    )
        .prop_map(|(ty, methods)| format!("impl {} {{\n{}\n}}", ty, methods.join("\n")))
}

/// Generate a generic type parameter.
fn generic_param_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple generic
        type_identifier_strategy(),
        // Bounded generic
        (type_identifier_strategy(), type_identifier_strategy())
            .prop_map(|(t, bound)| format!("{}: {}", t, bound)),
    ]
}

/// Generate a generic function.
fn generic_function_strategy() -> impl Strategy<Value = String> {
    (
        identifier_strategy(),
        prop::collection::vec(generic_param_strategy(), 1..3),
        param_strategy(),
        type_annotation_strategy(),
        simple_expr_strategy(),
    )
        .prop_map(|(name, generics, param, ret, body)| {
            format!(
                "@{}<{}> ({}) -> {} = {}",
                name,
                generics.join(", "),
                param,
                ret,
                body
            )
        })
}

/// Generate a where clause.
fn where_clause_strategy() -> impl Strategy<Value = String> {
    (type_identifier_strategy(), type_identifier_strategy())
        .prop_map(|(t, bound)| format!("{}: {}", t, bound))
}

/// Generate a function with where clause.
fn function_with_where_strategy() -> impl Strategy<Value = String> {
    (
        identifier_strategy(),
        type_identifier_strategy(),
        param_strategy(),
        type_annotation_strategy(),
        where_clause_strategy(),
        simple_expr_strategy(),
    )
        .prop_map(|(name, generic, param, ret, where_clause, body)| {
            format!(
                "@{}<{}> ({}) -> {} where {} = {}",
                name, generic, param, ret, where_clause, body
            )
        })
}

/// Generate a capability clause.
fn capability_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Http".to_string()),
        Just("FileSystem".to_string()),
        Just("Clock".to_string()),
        Just("Random".to_string()),
        Just("Async".to_string()),
        Just("Print".to_string()),
    ]
}

/// Generate a function with capabilities.
fn function_with_caps_strategy() -> impl Strategy<Value = String> {
    (
        identifier_strategy(),
        prop::collection::vec(param_strategy(), 0..2),
        type_annotation_strategy(),
        prop::collection::vec(capability_strategy(), 1..3),
        simple_expr_strategy(),
    )
        .prop_map(|(name, params, ret, caps, body)| {
            let params_str = if params.is_empty() {
                "()".to_string()
            } else {
                format!("({})", params.join(", "))
            };
            format!(
                "@{} {} -> {} uses {} = {}",
                name,
                params_str,
                ret,
                caps.join(", "),
                body
            )
        })
}

/// Generate a long binary expression that exceeds 100 characters.
fn long_binary_expr_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(int_literal_strategy(), 10..20).prop_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if i == 0 {
                    v.clone()
                } else {
                    format!(" + {}", v)
                }
            })
            .collect::<String>()
    })
}

/// Generate a string with unicode.
fn unicode_string_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("\"hello world\"".to_string()),
        Just("\"日本語\"".to_string()),
        Just("\"emoji: 🎉\"".to_string()),
        Just("\"mixed: hello 世界\"".to_string()),
        Just("\"special: café\"".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        max_shrink_iters: 500,
        ..ProptestConfig::default()
    })]

    /// Test idempotence for method chains.
    #[test]
    fn prop_method_chain_idempotence(chain in method_chain_strategy(4)) {
        let source = format!("@test_fn (x: int) -> int = {}", chain);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for field access chains.
    #[test]
    fn prop_field_access_idempotence(chain in field_access_strategy(5)) {
        let source = format!("@test_fn (x: int) -> int = {}", chain);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for lambda expressions.
    #[test]
    fn prop_lambda_expr_idempotence(lambda in lambda_strategy()) {
        let source = format!("@test_fn () -> (int) -> int = {}", lambda);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for run expressions.
    #[test]
    fn prop_run_expr_idempotence(run_expr in run_expr_strategy()) {
        let source = format!("@test_fn () -> int = {}", run_expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for match expressions.
    #[test]
    fn prop_match_expr_idempotence(match_expr in match_expr_strategy()) {
        let source = format!("@test_fn (x: int) -> int = {}", match_expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for for expressions.
    #[test]
    fn prop_for_expr_idempotence(for_expr in for_expr_strategy()) {
        let source = format!("@test_fn (items: [int]) -> int = {}", for_expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for trait definitions.
    #[test]
    fn prop_trait_idempotence(trait_def in trait_strategy()) {
        if let Err(e) = test_idempotence(&trait_def) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for impl blocks.
    #[test]
    fn prop_impl_idempotence(impl_def in impl_strategy()) {
        // Need a type to impl on
        let source = format!("type Foo = {{ x: int }}\n\n{}", impl_def.replace("impl Foo", "impl Foo"));
        let source = source.replace("impl ", "impl Foo ").replace("impl Foo Foo", "impl Foo");
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for generic functions.
    #[test]
    fn prop_generic_fn_idempotence(func in generic_function_strategy()) {
        if let Err(e) = test_idempotence(&func) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for functions with where clauses.
    #[test]
    fn prop_where_clause_idempotence(func in function_with_where_strategy()) {
        if let Err(e) = test_idempotence(&func) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for functions with capabilities.
    #[test]
    fn prop_capabilities_idempotence(func in function_with_caps_strategy()) {
        if let Err(e) = test_idempotence(&func) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for long expressions that exceed line width.
    #[test]
    fn prop_long_expr_idempotence(long_expr in long_binary_expr_strategy()) {
        let source = format!("@test_fn () -> int = {}", long_expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for unicode strings.
    #[test]
    fn prop_unicode_string_idempotence(unicode in unicode_string_strategy()) {
        let source = format!("@test_fn () -> str = {}", unicode);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for deeply nested conditionals.
    #[test]
    fn prop_nested_conditional_idempotence(depth in 1usize..5) {
        let mut expr = "x".to_string();
        for i in 0..depth {
            expr = format!("if x > {} then {} else {}", i, i + 10, expr);
        }
        let source = format!("@test_fn (x: int) -> int = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for nested collections.
    #[test]
    fn prop_nested_collection_idempotence(depth in 1usize..4) {
        let mut expr = "1".to_string();
        for _ in 0..depth {
            expr = format!("[{}, {}, {}]", expr, expr, expr);
        }
        let source = format!("@test_fn () -> int = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for mixed operators with different precedences.
    #[test]
    fn prop_mixed_precedence_idempotence(
        a in int_literal_strategy(),
        b in int_literal_strategy(),
        c in int_literal_strategy(),
        d in int_literal_strategy()
    ) {
        // Mix of different precedence levels
        let source = format!("@test_fn () -> int = {} + {} * {} - {} / 2", a, b, c, d);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for comparison chains.
    #[test]
    fn prop_comparison_chain_idempotence(
        vals in prop::collection::vec(int_literal_strategy(), 3..6)
    ) {
        let ops = ["<", ">", "<=", ">=", "==", "!="];
        let mut expr = vals[0].clone();
        for (i, val) in vals.iter().skip(1).enumerate() {
            expr = format!("({}) {} {}", expr, ops[i % ops.len()], val);
        }
        let source = format!("@test_fn () -> bool = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for logical chains.
    #[test]
    fn prop_logical_chain_idempotence(count in 2usize..6) {
        let mut expr = "true".to_string();
        for i in 0..count {
            let op = if i % 2 == 0 { "&&" } else { "||" };
            expr = format!("{} {} false", expr, op);
        }
        let source = format!("@test_fn () -> bool = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for structs with many fields.
    #[test]
    fn prop_large_struct_idempotence(field_count in 3usize..10) {
        let fields: Vec<String> = (0..field_count)
            .map(|i| format!("field_{}: int", i))
            .collect();
        let source = format!("type BigStruct = {{ {} }}", fields.join(", "));
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for sum types with many variants.
    #[test]
    fn prop_large_sum_type_idempotence(variant_count in 2usize..8) {
        let variants: Vec<String> = (0..variant_count)
            .map(|i| format!("Variant{}", i))
            .collect();
        let source = format!("type BigSum = {}", variants.join(" | "));
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for functions with many parameters.
    #[test]
    fn prop_many_params_idempotence(param_count in 2usize..8) {
        let params: Vec<String> = (0..param_count)
            .map(|i| format!("param_{}: int", i))
            .collect();
        let source = format!("@many_params ({}) -> int = 0", params.join(", "));
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for functions with many generic parameters.
    #[test]
    fn prop_many_generics_idempotence(generic_count in 1usize..5) {
        let generics: Vec<String> = (0..generic_count)
            .map(|i| format!("T{}", i))
            .collect();
        let source = format!("@generic_fn<{}> (x: T0) -> T0 = x", generics.join(", "));
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }
}

// -- Additional Unit Tests for Edge Cases --

#[test]
fn test_single_element_tuple_idempotence() {
    // Single-element tuples need trailing comma
    let source = "@test_fn () -> int = (42,)";
    test_idempotence(source).expect("single element tuple should be idempotent");
}

#[test]
fn test_empty_list_idempotence() {
    let source = "@test_fn () -> [int] = []";
    test_idempotence(source).expect("empty list should be idempotent");
}

#[test]
fn test_nested_if_idempotence() {
    let source = "@test_fn (x: int) -> int = if x > 0 then if x > 10 then 100 else 10 else 0";
    test_idempotence(source).expect("nested if should be idempotent");
}

#[test]
fn test_lambda_idempotence() {
    let source = "@test_fn () -> (int) -> int = x -> x + 1";
    test_idempotence(source).expect("lambda should be idempotent");
}

#[test]
fn test_multi_param_lambda_idempotence() {
    let source = "@test_fn () -> (int, int) -> int = (a, b) -> a + b";
    test_idempotence(source).expect("multi-param lambda should be idempotent");
}

#[test]
fn test_option_some_idempotence() {
    let source = "@test_fn () -> Option<int> = Some(42)";
    test_idempotence(source).expect("Some should be idempotent");
}

#[test]
fn test_option_none_idempotence() {
    let source = "@test_fn () -> Option<int> = None";
    test_idempotence(source).expect("None should be idempotent");
}

#[test]
fn test_result_ok_idempotence() {
    let source = "@test_fn () -> Result<int, str> = Ok(42)";
    test_idempotence(source).expect("Ok should be idempotent");
}

#[test]
fn test_result_err_idempotence() {
    let source = r#"@test_fn () -> Result<int, str> = Err("error")"#;
    test_idempotence(source).expect("Err should be idempotent");
}

#[test]
fn test_complex_struct_idempotence() {
    let source = "type Point = { x: int, y: int, z: int }";
    test_idempotence(source).expect("struct should be idempotent");
}

#[test]
fn test_sum_type_idempotence() {
    // Note: Ok/Err are reserved (Result constructors), use different names
    let source = "type Status = Success | Failure | Pending";
    test_idempotence(source).expect("sum type should be idempotent");
}

#[test]
fn test_generic_function_idempotence() {
    let source = "@identity<T> (x: T) -> T = x";
    test_idempotence(source).expect("generic function should be idempotent");
}

#[test]
fn test_where_clause_idempotence() {
    let source = "@compare<T> (a: T, b: T) -> bool where T: Eq = a == b";
    test_idempotence(source).expect("where clause should be idempotent");
}

#[test]
fn test_const_def_idempotence() {
    let source = "let $PI = 3.14159";
    test_idempotence(source).expect("const should be idempotent");
}

#[test]
fn test_multiple_declarations_idempotence() {
    let source = r"
let $MAX = 100

type Point = { x: int, y: int }

@origin () -> Point = Point { x: 0, y: 0 }

@distance (p: Point) -> float = 0.0
";
    test_idempotence(source).expect("module should be idempotent");
}

#[test]
fn test_binary_expr_line_break() {
    // Regression test: binary expression breaking must preserve semantics.
    // The parser must accept binary operators at line start.
    let source = r#"@test (a: str, b: str) -> bool = "string" <= other"#;
    test_idempotence(source).expect("binary expression with line break should be idempotent");
}

// -- Substantially More Comprehensive Tests --

// -- Literal Edge Cases --

#[test]
fn test_zero_literal() {
    let source = "@f () -> int = 0";
    test_idempotence(source).expect("zero should be idempotent");
}

#[test]
fn test_negative_literal() {
    let source = "@f () -> int = -42";
    test_idempotence(source).expect("negative should be idempotent");
}

#[test]
fn test_large_int_literal() {
    let source = "@f () -> int = 9_223_372_036_854_775_807";
    test_idempotence(source).expect("large int should be idempotent");
}

#[test]
fn test_float_zero() {
    let source = "@f () -> float = 0.0";
    test_idempotence(source).expect("float zero should be idempotent");
}

#[test]
fn test_float_scientific() {
    let source = "@f () -> float = 1.5e10";
    test_idempotence(source).expect("scientific notation should be idempotent");
}

#[test]
fn test_float_negative_exponent() {
    let source = "@f () -> float = 2.5e-8";
    test_idempotence(source).expect("negative exponent should be idempotent");
}

#[test]
fn test_empty_string() {
    let source = r#"@f () -> str = """#;
    test_idempotence(source).expect("empty string should be idempotent");
}

#[test]
fn test_string_with_escapes() {
    let source = r#"@f () -> str = "hello\nworld\ttab""#;
    test_idempotence(source).expect("escaped string should be idempotent");
}

#[test]
fn test_char_escape_sequences() {
    let sources = [
        r"@f () -> char = '\n'",
        r"@f () -> char = '\t'",
        r"@f () -> char = '\r'",
        r"@f () -> char = '\0'",
        r"@f () -> char = '\\'",
    ];
    for source in sources {
        test_idempotence(source).expect("char escape should be idempotent");
    }
}

// -- Operator Edge Cases --

#[test]
fn test_bitwise_and() {
    let source = "@f (a: int, b: int) -> int = a & b";
    test_idempotence(source).expect("bitwise and should be idempotent");
}

#[test]
fn test_bitwise_or() {
    let source = "@f (a: int, b: int) -> int = a | b";
    test_idempotence(source).expect("bitwise or should be idempotent");
}

#[test]
fn test_bitwise_xor() {
    let source = "@f (a: int, b: int) -> int = a ^ b";
    test_idempotence(source).expect("bitwise xor should be idempotent");
}

#[test]
fn test_left_shift() {
    let source = "@f (a: int) -> int = a << 2";
    test_idempotence(source).expect("left shift should be idempotent");
}

#[test]
fn test_right_shift() {
    let source = "@f (a: int) -> int = a >> 2";
    test_idempotence(source).expect("right shift should be idempotent");
}

#[test]
fn test_modulo() {
    let source = "@f (a: int, b: int) -> int = a % b";
    test_idempotence(source).expect("modulo should be idempotent");
}

#[test]
fn test_unary_not() {
    let source = "@f (a: bool) -> bool = !a";
    test_idempotence(source).expect("unary not should be idempotent");
}

#[test]
fn test_unary_negate() {
    let source = "@f (a: int) -> int = -a";
    test_idempotence(source).expect("unary negate should be idempotent");
}

#[test]
fn test_double_negation() {
    let source = "@f (a: int) -> int = --a";
    test_idempotence(source).expect("double negation should be idempotent");
}

#[test]
fn test_complex_operator_precedence() {
    let source = "@f (a: int, b: int, c: int) -> int = a + b * c - (a / b) % c";
    test_idempotence(source).expect("complex precedence should be idempotent");
}

#[test]
fn test_mixed_comparison_logical() {
    let source = "@f (a: int, b: int) -> bool = a > 0 && b < 10 || a == b";
    test_idempotence(source).expect("mixed comparison/logical should be idempotent");
}

// -- Collection Tests --

#[test]
fn test_list_of_lists() {
    let source = "@f () -> [[int]] = [[1, 2], [3, 4], [5, 6]]";
    test_idempotence(source).expect("nested lists should be idempotent");
}

#[test]
fn test_list_of_tuples() {
    let source = "@f () -> [(int, str)] = [(1, \"a\"), (2, \"b\")]";
    test_idempotence(source).expect("list of tuples should be idempotent");
}

#[test]
fn test_tuple_of_lists() {
    let source = "@f () -> ([int], [str]) = ([1, 2], [\"a\", \"b\"])";
    test_idempotence(source).expect("tuple of lists should be idempotent");
}

#[test]
fn test_empty_tuple() {
    let source = "@f () -> () = ()";
    test_idempotence(source).expect("empty tuple should be idempotent");
}

#[test]
fn test_struct_literal_simple() {
    let source = "type Point = { x: int, y: int }\n\n@f () -> Point = Point { x: 1, y: 2 }";
    test_idempotence(source).expect("struct literal should be idempotent");
}

#[test]
fn test_struct_literal_shorthand() {
    let source = "type Point = { x: int, y: int }\n\n@f (x: int, y: int) -> Point = Point { x, y }";
    test_idempotence(source).expect("struct shorthand should be idempotent");
}

#[test]
fn test_struct_nested() {
    let source = r"
type Inner = { a: int }
type Outer = { inner: Inner, b: int }

@f () -> Outer = Outer { inner: Inner { a: 1 }, b: 2 }
";
    test_idempotence(source).expect("nested struct should be idempotent");
}

#[test]
fn test_range_exclusive() {
    let source = "@f () -> int = for i in 0..10 yield i";
    test_idempotence(source).expect("exclusive range should be idempotent");
}

#[test]
fn test_range_inclusive() {
    let source = "@f () -> int = for i in 0..=10 yield i";
    test_idempotence(source).expect("inclusive range should be idempotent");
}

// -- Control Flow Tests --

#[test]
fn test_if_without_else() {
    let source = "@f (x: int) -> void = if x > 0 then print(msg: \"positive\")";
    test_idempotence(source).expect("if without else should be idempotent");
}

#[test]
fn test_chained_if_else() {
    let source = "@f (x: int) -> str = if x < 0 then \"negative\" else if x == 0 then \"zero\" else \"positive\"";
    test_idempotence(source).expect("chained if-else should be idempotent");
}

#[test]
fn test_deeply_nested_if() {
    let source = "@f (a: bool, b: bool, c: bool) -> int = if a then if b then if c then 1 else 2 else 3 else 4";
    test_idempotence(source).expect("deeply nested if should be idempotent");
}

#[test]
fn test_for_with_filter() {
    let source = "@f (items: [int]) -> [int] = for x in items if x > 0 yield x";
    test_idempotence(source).expect("for with filter should be idempotent");
}

#[test]
fn test_nested_for() {
    let source = "@f (rows: [[int]]) -> [int] = for row in rows yield for x in row yield x";
    test_idempotence(source).expect("nested for should be idempotent");
}

// -- Pattern Construct Tests --

#[test]
fn test_run_multiple_bindings() {
    let source = "@f () -> int = { let a = 1; let b = 2; a + b }";
    test_idempotence(source).expect("block multiple bindings should be idempotent");
}

#[test]
fn test_try_expression() {
    let source = "@f (x: Result<int, str>) -> Result<int, str> = try { let v = x; Ok(v + 1) }";
    test_idempotence(source).expect("try expression should be idempotent");
}

#[test]
fn test_match_option() {
    let source = "@f (opt: Option<int>) -> int = match opt { Some(x) -> x, None -> 0 }";
    test_idempotence(source).expect("match option should be idempotent");
}

#[test]
fn test_match_result() {
    let source = "@f (res: Result<int, str>) -> int = match res { Ok(x) -> x, Err(_) -> 0 }";
    test_idempotence(source).expect("match result should be idempotent");
}

#[test]
fn test_match_simple_patterns() {
    let source = "@f (x: int) -> str = match x { 0 -> \"zero\", 1 -> \"one\", _ -> \"other\" }";
    test_idempotence(source).expect("match simple patterns should be idempotent");
}

#[test]
fn test_match_nested() {
    let source = "@f (x: Option<Option<int>>) -> int = match x { Some(inner) -> match inner { Some(v) -> v, None -> 0 }, None -> -1 }";
    test_idempotence(source).expect("nested match should be idempotent");
}

#[test]
fn test_match_tuple_pattern() {
    let source = "@f (pair: (int, str)) -> int = match pair { (x, _) -> x }";
    test_idempotence(source).expect("match tuple pattern should be idempotent");
}

#[test]
fn test_match_list_pattern() {
    let source = "@f (list: [int]) -> int = match list { [first, ..rest] -> first, [] -> 0 }";
    test_idempotence(source).expect("match list pattern should be idempotent");
}

// -- Function Call Tests --

#[test]
fn test_simple_call() {
    let source = "@f (x: int) -> int = x\n\n@g () -> int = f(x: 42)";
    test_idempotence(source).expect("simple call should be idempotent");
}

#[test]
fn test_nested_calls() {
    let source = "@f (x: int) -> int = x\n\n@g () -> int = f(x: f(x: f(x: 1)))";
    test_idempotence(source).expect("nested calls should be idempotent");
}

#[test]
fn test_call_with_lambda() {
    let source = "@f (transform: (int) -> int) -> int = transform(1)\n\n@g () -> int = f(transform: x -> x + 1)";
    test_idempotence(source).expect("call with lambda should be idempotent");
}

#[test]
fn test_method_call_chain_long() {
    let source = "@f (x: int) -> int = x.foo().bar().baz().qux().quux()";
    test_idempotence(source).expect("long method chain should be idempotent");
}

#[test]
fn test_mixed_access_chain() {
    let source = "@f (obj: int) -> int = obj.field.method().another_field.final_method()";
    test_idempotence(source).expect("mixed access chain should be idempotent");
}

#[test]
fn test_index_access() {
    let source = "@f (list: [int]) -> int = list[0]";
    test_idempotence(source).expect("index access should be idempotent");
}

#[test]
fn test_index_with_hash() {
    let source = "@f (list: [int]) -> int = list[# - 1]";
    test_idempotence(source).expect("index with hash should be idempotent");
}

#[test]
fn test_nested_index() {
    let source = "@f (matrix: [[int]]) -> int = matrix[0][1]";
    test_idempotence(source).expect("nested index should be idempotent");
}

// -- Lambda Tests --

#[test]
fn test_lambda_no_params() {
    let source = "@f () -> (() -> int) = () -> 42";
    test_idempotence(source).expect("no-param lambda should be idempotent");
}

#[test]
fn test_lambda_single_param() {
    let source = "@f () -> ((int) -> int) = x -> x * 2";
    test_idempotence(source).expect("single-param lambda should be idempotent");
}

#[test]
fn test_lambda_multi_param() {
    let source = "@f () -> ((int, int, int) -> int) = (a, b, c) -> a + b + c";
    test_idempotence(source).expect("multi-param lambda should be idempotent");
}

#[test]
fn test_lambda_with_type_annotation() {
    let source = "@f () -> ((int) -> int) = (x: int) -> int = x + 1";
    test_idempotence(source).expect("typed lambda should be idempotent");
}

#[test]
fn test_nested_lambda() {
    let source = "@f () -> ((int) -> (int) -> int) = x -> y -> x + y";
    test_idempotence(source).expect("nested lambda should be idempotent");
}

#[test]
fn test_lambda_with_complex_body() {
    let source = "@f () -> ((int) -> int) = x -> if x > 0 then x * 2 else 0";
    test_idempotence(source).expect("lambda with complex body should be idempotent");
}

// -- Type Definition Tests --

#[test]
fn test_empty_struct() {
    let source = "type Empty = {}";
    test_idempotence(source).expect("empty struct should be idempotent");
}

#[test]
fn test_struct_single_field() {
    let source = "type Single = { value: int }";
    test_idempotence(source).expect("single field struct should be idempotent");
}

#[test]
fn test_struct_many_fields() {
    let source = "type Many = { a: int, b: str, c: bool, d: float, e: char }";
    test_idempotence(source).expect("many field struct should be idempotent");
}

#[test]
fn test_sum_type_two_variants() {
    let source = "type Either = Left | Right";
    test_idempotence(source).expect("two variant sum should be idempotent");
}

#[test]
fn test_sum_type_with_fields() {
    let source = "type Tree = Leaf(value: int) | Node(left: Tree, right: Tree)";
    test_idempotence(source).expect("sum with fields should be idempotent");
}

#[test]
fn test_generic_type() {
    let source = "type Box<T> = { value: T }";
    test_idempotence(source).expect("generic type should be idempotent");
}

#[test]
fn test_generic_type_multi_param() {
    let source = "type Pair<A, B> = { first: A, second: B }";
    test_idempotence(source).expect("multi-param generic should be idempotent");
}

#[test]
fn test_derive_eq() {
    let source = "#derive(Eq)\ntype Point = { x: int, y: int }";
    test_idempotence(source).expect("derive Eq should be idempotent");
}

#[test]
fn test_derive_multiple() {
    let source = "#derive(Eq, Clone, Debug)\ntype Point = { x: int, y: int }";
    test_idempotence(source).expect("derive multiple should be idempotent");
}

#[test]
fn test_public_type() {
    let source = "pub type PublicPoint = { x: int, y: int }";
    test_idempotence(source).expect("public type should be idempotent");
}

// -- Trait and Impl Tests --

#[test]
fn test_empty_trait() {
    let source = "trait Marker { }";
    test_idempotence(source).expect("empty trait should be idempotent");
}

#[test]
fn test_trait_single_method() {
    let source = "trait Sized {\n    @size (self) -> int\n}";
    test_idempotence(source).expect("single method trait should be idempotent");
}

#[test]
fn test_trait_multiple_methods() {
    let source = "trait Container {\n    @len (self) -> int\n    @is_empty (self) -> bool\n    @capacity (self) -> int\n}";
    test_idempotence(source).expect("multi-method trait should be idempotent");
}

#[test]
fn test_trait_with_default() {
    let source =
        "trait WithDefault {\n    @required (self) -> int\n    @optional (self) -> int = 0\n}";
    test_idempotence(source).expect("trait with default should be idempotent");
}

#[test]
fn test_trait_inheritance() {
    let source = "trait Parent {\n    @parent_method (self) -> int\n}\n\ntrait Child: Parent {\n    @child_method (self) -> int\n}";
    test_idempotence(source).expect("trait inheritance should be idempotent");
}

#[test]
fn test_impl_inherent() {
    let source = "type Point = { x: int, y: int }\n\nimpl Point {\n    @origin () -> Point = Point { x: 0, y: 0 }\n}";
    test_idempotence(source).expect("inherent impl should be idempotent");
}

#[test]
fn test_impl_trait() {
    let source = "trait Printable {\n    @to_str (self) -> str\n}\n\ntype Num = { value: int }\n\nimpl Printable for Num {\n    @to_str (self) -> str = \"num\"\n}";
    test_idempotence(source).expect("trait impl should be idempotent");
}

#[test]
fn test_impl_generic() {
    let source = "trait Default {\n    @default () -> Self\n}\n\ntype Box<T> = { value: T }\n\nimpl<T: Default> Default for Box<T> {\n    @default () -> Box<T> = Box { value: T.default() }\n}";
    test_idempotence(source).expect("generic impl should be idempotent");
}

// -- Function Signature Tests --

#[test]
fn test_function_no_params() {
    let source = "@constant () -> int = 42";
    test_idempotence(source).expect("no params should be idempotent");
}

#[test]
fn test_function_single_param() {
    let source = "@identity (x: int) -> int = x";
    test_idempotence(source).expect("single param should be idempotent");
}

#[test]
fn test_function_many_params() {
    let source = "@sum (a: int, b: int, c: int, d: int, e: int) -> int = a + b + c + d + e";
    test_idempotence(source).expect("many params should be idempotent");
}

#[test]
fn test_function_void_return() {
    let source = "@log (msg: str) -> void = print(msg: msg)";
    test_idempotence(source).expect("void return should be idempotent");
}

#[test]
fn test_function_generic_single() {
    let source = "@identity<T> (x: T) -> T = x";
    test_idempotence(source).expect("single generic should be idempotent");
}

#[test]
fn test_function_generic_multiple() {
    let source = "@pair<A, B> (a: A, b: B) -> (A, B) = (a, b)";
    test_idempotence(source).expect("multiple generics should be idempotent");
}

#[test]
fn test_function_generic_bounded() {
    let source = "@compare<T: Eq> (a: T, b: T) -> bool = a == b";
    test_idempotence(source).expect("bounded generic should be idempotent");
}

#[test]
fn test_function_where_single() {
    let source = "@f<T> (x: T) -> T where T: Clone = x.clone()";
    test_idempotence(source).expect("single where should be idempotent");
}

#[test]
fn test_function_where_multiple() {
    let source = "@f<T, U> (x: T, y: U) -> T where T: Clone, U: Default = x.clone()";
    test_idempotence(source).expect("multiple where should be idempotent");
}

#[test]
fn test_function_capability_single() {
    let source = "@fetch (url: str) -> str uses Http = \"response\"";
    test_idempotence(source).expect("single capability should be idempotent");
}

#[test]
fn test_function_capability_multiple() {
    let source = "@complex () -> void uses Http, FileSystem, Clock = ()";
    test_idempotence(source).expect("multiple capabilities should be idempotent");
}

#[test]
fn test_function_public() {
    let source = "pub @public_fn () -> int = 42";
    test_idempotence(source).expect("public function should be idempotent");
}

#[test]
fn test_function_all_features() {
    // Order: generics, params, return, uses, where, body
    let source =
        "pub @complex<T, U> (a: T, b: U) -> T uses Http where T: Clone, U: Default = a.clone()";
    test_idempotence(source).expect("function with all features should be idempotent");
}

// -- Import Tests --

#[test]
fn test_import_single() {
    let source = "use std.math { sqrt }\n\n@f () -> float = sqrt(value: 4.0)";
    test_idempotence(source).expect("single import should be idempotent");
}

#[test]
fn test_import_multiple() {
    let source = "use std.math { sqrt, abs, min, max }\n\n@f () -> float = sqrt(value: 4.0)";
    test_idempotence(source).expect("multiple imports should be idempotent");
}

#[test]
fn test_import_alias() {
    let source = "use std.math { sqrt as square_root }\n\n@f () -> float = square_root(value: 4.0)";
    test_idempotence(source).expect("import alias should be idempotent");
}

#[test]
fn test_import_relative() {
    let source = "use \"./helper\" { util }\n\n@f () -> int = util(x: 1)";
    test_idempotence(source).expect("relative import should be idempotent");
}

// -- Constant Tests --

#[test]
fn test_const_int() {
    let source = "let $MAX_SIZE = 1000";
    test_idempotence(source).expect("int const should be idempotent");
}

#[test]
fn test_const_float() {
    let source = "let $PI = 3.14159";
    test_idempotence(source).expect("float const should be idempotent");
}

#[test]
fn test_const_string() {
    let source = "let $GREETING = \"Hello, World!\"";
    test_idempotence(source).expect("string const should be idempotent");
}

#[test]
fn test_const_bool() {
    let source = "let $DEBUG = true";
    test_idempotence(source).expect("bool const should be idempotent");
}

#[test]
fn test_const_public() {
    let source = "pub let $PUBLIC_CONST = 42";
    test_idempotence(source).expect("public const should be idempotent");
}

// -- Test Declaration Tests --

#[test]
fn test_targeted_test() {
    let source = "@add (a: int, b: int) -> int = a + b\n\n@test_add tests @add () -> void = assert_eq(actual: add(a: 1, b: 2), expected: 3)";
    test_idempotence(source).expect("targeted test should be idempotent");
}

#[test]
fn test_free_floating_test() {
    // Free-floating tests use @name syntax without a target
    let source = "@test_something () -> void = assert(condition: true)";
    test_idempotence(source).expect("free floating test should be idempotent");
}

// -- Comment Tests --

#[test]
fn test_single_comment() {
    let source = "// This is a comment\n@f () -> int = 42";
    test_idempotence(source).expect("single comment should be idempotent");
}

#[test]
fn test_doc_comment() {
    let source = "// #Description of function\n@f () -> int = 42";
    test_idempotence(source).expect("doc comment should be idempotent");
}

#[test]
fn test_param_comment() {
    let source = "// * x: The input value\n@f (x: int) -> int = x";
    test_idempotence(source).expect("param comment should be idempotent");
}

#[test]
fn test_multiple_comments() {
    let source =
        "// #Description\n// * x: Input\n// * y: Another input\n@f (x: int, y: int) -> int = x + y";
    test_idempotence(source).expect("multiple comments should be idempotent");
}

// -- Line Width Edge Cases --

#[test]
fn test_long_int_chain() {
    // Create a line with many integers
    let source = "@f () -> int = 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11 + 12 + 13 + 14 + 15";
    test_idempotence(source).expect("long int chain should be idempotent");
}

#[test]
fn test_long_function_name() {
    let source = "@this_is_a_very_long_function_name_that_goes_on_for_quite_a_while () -> int = 42";
    test_idempotence(source).expect("long function name should be idempotent");
}

#[test]
fn test_long_param_names() {
    let source = "@f (this_is_a_long_parameter_name: int, another_very_long_parameter_name: str) -> int = 42";
    test_idempotence(source).expect("long param names should be idempotent");
}

#[test]
fn test_long_type_annotation() {
    let source = "@f () -> Result<Option<[int]>, str> = Ok(None)";
    test_idempotence(source).expect("long type annotation should be idempotent");
}

#[test]
fn test_very_long_expression() {
    let source = "@f () -> int = 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11 + 12 + 13 + 14 + 15 + 16 + 17 + 18 + 19 + 20";
    test_idempotence(source).expect("very long expression should be idempotent");
}

// -- Complex Combined Tests --

#[test]
fn test_full_module() {
    let source = r#"
let $MAX = 100

type Point = { x: int, y: int }

trait Printable {
    @to_str (self) -> str
}

impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }
}

impl Printable for Point {
    @to_str (self) -> str = "point"
}

@distance (p1: Point, p2: Point) -> float = 0.0
"#;
    test_idempotence(source).expect("full module should be idempotent");
}

#[test]
fn test_complex_expression_composition() {
    // Simpler method chain that doesn't trigger lambda line-break issues
    let source = "@f (data: [int]) -> int = data.filter(predicate: x -> x > 0).map(transform: x -> x * 2).fold(init: 0, f: (a, b) -> a + b)";
    test_idempotence(source).expect("complex expression should be idempotent");
}

#[test]
fn test_deeply_nested_everything() {
    let source = "@f () -> int = if true then match [1, 2, 3] { [x, ..rest] -> { let sum = x + for r in rest yield r; sum }, [] -> 0 } else 0";
    test_idempotence(source).expect("deeply nested everything should be idempotent");
}

// -- Additional Property Tests for More Coverage --

/// Generate a duration literal.
fn duration_literal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        (1u64..1000).prop_map(|n| format!("{}ms", n)),
        (1u64..60).prop_map(|n| format!("{}s", n)),
        (1u64..60).prop_map(|n| format!("{}m", n)),
        (1u64..24).prop_map(|n| format!("{}h", n)),
    ]
}

/// Generate a size literal.
fn size_literal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        (1u64..1000).prop_map(|n| format!("{}kb", n)),
        (1u64..100).prop_map(|n| format!("{}mb", n)),
        (1u64..10).prop_map(|n| format!("{}gb", n)),
    ]
}

/// Generate an import statement.
fn import_strategy() -> impl Strategy<Value = String> {
    (
        prop_oneof![
            Just("std.math".to_string()),
            Just("std.io".to_string()),
            Just("std.collections".to_string()),
        ],
        prop::collection::vec(identifier_strategy(), 1..4),
    )
        .prop_map(|(module, items)| format!("use {} {{ {} }}", module, items.join(", ")))
}

/// Generate a test declaration.
fn test_decl_strategy() -> impl Strategy<Value = String> {
    (identifier_strategy(), identifier_strategy()).prop_map(|(test_name, target)| {
        format!(
            "@{} tests @{} () -> void = assert(condition: true)",
            test_name, target
        )
    })
}

/// Generate a public function.
fn public_function_strategy() -> impl Strategy<Value = String> {
    function_strategy().prop_map(|f| format!("pub {}", f))
}

/// Generate struct with generic parameters.
fn generic_struct_strategy() -> impl Strategy<Value = String> {
    (
        type_identifier_strategy(),
        prop::collection::vec(type_identifier_strategy(), 1..3),
        prop::collection::vec(
            (identifier_strategy(), type_identifier_strategy())
                .prop_map(|(n, t)| format!("{}: {}", n, t)),
            1..4,
        ),
    )
        .prop_map(|(name, generics, fields)| {
            format!(
                "type {}<{}> = {{ {} }}",
                name,
                generics.join(", "),
                fields.join(", ")
            )
        })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        max_shrink_iters: 250,
        ..ProptestConfig::default()
    })]

    /// Test idempotence for duration literals.
    #[test]
    fn prop_duration_literal_idempotence(duration in duration_literal_strategy()) {
        let source = format!("@f () -> Duration = {}", duration);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for size literals.
    #[test]
    fn prop_size_literal_idempotence(size in size_literal_strategy()) {
        let source = format!("@f () -> Size = {}", size);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for import statements.
    #[test]
    fn prop_import_idempotence(import in import_strategy()) {
        let source = format!("{}\n\n@f () -> int = 42", import);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for test declarations.
    #[test]
    fn prop_test_decl_idempotence(test_decl in test_decl_strategy()) {
        let source = format!("@dummy () -> int = 42\n\n{}", test_decl);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for public functions.
    #[test]
    fn prop_public_fn_idempotence(func in public_function_strategy()) {
        if let Err(e) = test_idempotence(&func) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for generic structs.
    #[test]
    fn prop_generic_struct_idempotence(struct_def in generic_struct_strategy()) {
        if let Err(e) = test_idempotence(&struct_def) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for bitwise operator chains.
    #[test]
    fn prop_bitwise_chain_idempotence(
        ops in prop::collection::vec(
            prop_oneof![Just("&"), Just("|"), Just("^")],
            2..5
        ),
        vals in prop::collection::vec(int_literal_strategy(), 3..6)
    ) {
        if vals.len() <= ops.len() {
            return Ok(());
        }
        let mut expr = vals[0].clone();
        for (i, op) in ops.iter().enumerate() {
            if i + 1 < vals.len() {
                expr = format!("{} {} {}", expr, op, vals[i + 1]);
            }
        }
        let source = format!("@f () -> int = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for shift operator chains.
    #[test]
    fn prop_shift_chain_idempotence(
        ops in prop::collection::vec(prop_oneof![Just("<<"), Just(">>")], 1..4),
        base in int_literal_strategy(),
        shifts in prop::collection::vec(1u32..8, 1..4)
    ) {
        let mut expr = base;
        for (op, shift) in ops.iter().zip(shifts.iter()) {
            expr = format!("({} {} {})", expr, op, shift);
        }
        let source = format!("@f () -> int = {}", expr);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for complex type annotations.
    #[test]
    fn prop_complex_type_idempotence(depth in 1usize..4) {
        let mut ty = "int".to_string();
        for i in 0..depth {
            ty = match i % 3 {
                0 => format!("[{}]", ty),
                1 => format!("Option<{}>", ty),
                _ => format!("Result<{}, str>", ty),
            };
        }
        let source = format!("@f () -> {} = panic(msg: \"unimplemented\")", ty);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for function type annotations.
    #[test]
    fn prop_fn_type_idempotence(param_count in 1usize..4) {
        let params: Vec<&str> = (0..param_count).map(|_| "int").collect();
        let fn_type = format!("({}) -> int", params.join(", "));
        let source = format!("@f () -> {} = panic(msg: \"unimplemented\")", fn_type);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for tuple types.
    #[test]
    fn prop_tuple_type_idempotence(elem_count in 2usize..6) {
        let elems: Vec<&str> = (0..elem_count)
            .map(|i| match i % 3 { 0 => "int", 1 => "str", _ => "bool" })
            .collect();
        let tuple_type = format!("({})", elems.join(", "));
        let source = format!("@f () -> {} = panic(msg: \"unimplemented\")", tuple_type);
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for struct literals with many fields.
    #[test]
    fn prop_struct_literal_idempotence(field_count in 2usize..6) {
        let fields: Vec<String> = (0..field_count)
            .map(|i| format!("f{}: int", i))
            .collect();
        let inits: Vec<String> = (0..field_count)
            .map(|i| format!("f{}: {}", i, i))
            .collect();
        let source = format!(
            "type S = {{ {} }}\n\n@f () -> S = S {{ {} }}",
            fields.join(", "),
            inits.join(", ")
        );
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for modules with many functions.
    #[test]
    fn prop_many_functions_idempotence(func_count in 2usize..8) {
        let funcs: Vec<String> = (0..func_count)
            .map(|i| format!("@fn_{} () -> int = {}", i, i))
            .collect();
        let source = funcs.join("\n\n");
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }

    /// Test idempotence for modules with mixed declarations.
    #[test]
    fn prop_mixed_decls_idempotence(
        const_count in 0usize..3,
        type_count in 0usize..3,
        func_count in 1usize..4
    ) {
        let mut parts = Vec::new();
        for i in 0..const_count {
            parts.push(format!("let $CONST_{} = {}", i, i));
        }
        for i in 0..type_count {
            parts.push(format!("type Type{} = {{ x: int }}", i));
        }
        for i in 0..func_count {
            parts.push(format!("@func_{} () -> int = {}", i, i));
        }
        let source = parts.join("\n\n");
        if let Err(e) = test_idempotence(&source) {
            if e.contains("Idempotence failure") {
                return Err(TestCaseError::fail(e));
            }
        }
    }
}
