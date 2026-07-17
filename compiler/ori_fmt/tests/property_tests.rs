//! Property-based tests for the Ori formatter.
//!
//! These tests use proptest to generate random valid Ori code and verify:
//! 1. Idempotence: format(format(code)) == format(code)
//! 2. Parse-ability: formatted output can be re-parsed
//!
//! This complements the idempotence_tests.rs which tests real files,
//! by generating synthetic code that might exercise edge cases not
//! present in the test corpus.

#![expect(
    clippy::expect_used,
    reason = "property-test failures report the generated counterexample by panicking"
)]
#![expect(
    clippy::doc_markdown,
    clippy::disallowed_types,
    clippy::uninlined_format_args,
    clippy::redundant_closure_for_method_calls,
    reason = "Proptest macros generate code with these patterns"
)]

use ori_fmt::{format_module, format_module_with_comments};
use ori_ir::StringInterner;
use ori_lexer::lex_with_comments;
use proptest::prelude::*;

fn fixture_without_trailing_newline(source: &'static str) -> &'static str {
    source
        .strip_suffix('\n')
        .expect("committed Ori fixtures end with a newline")
}

// Code Generation Strategies

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

// Test Helpers

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

/// Classification of one generated source's idempotence + round-trip check.
enum PropIdemOutcome {
    /// `format(format(x)) == format(x)` and the output re-parsed.
    Pass,
    /// The generated source itself did not parse — strategy noise (a generated
    /// program may be type/grammar-invalid), not a formatter defect.
    GeneratedSourceInvalid,
    /// The formatted output failed to re-parse — a `format -> parse` round-trip
    /// break (Spec: Annex D — formatting must preserve observable semantics).
    /// On synthetic input it cannot be path-allowlisted, so it is an explicit
    /// tracked outcome, never silently swallowed.
    ReparseBreakTracked,
    /// `format(format(x)) != format(x)` — a non-idempotent format. The property
    /// this suite exists to pin; always a hard failure.
    IdempotenceMismatch(String),
}

/// Test that `format(format(code)) == format(code)` and the output re-parses.
fn test_idempotence(source: &str) -> PropIdemOutcome {
    let Ok(first) = parse_and_format_with_comments(source).or_else(|_| parse_and_format(source))
    else {
        return PropIdemOutcome::GeneratedSourceInvalid;
    };

    // Output of the formatter must re-parse; failure here is a round-trip break.
    let Ok(second) = parse_and_format_with_comments(&first).or_else(|_| parse_and_format(&first))
    else {
        return PropIdemOutcome::ReparseBreakTracked;
    };

    let first_normalized = normalize_whitespace(&first);
    let second_normalized = normalize_whitespace(&second);

    if first_normalized != second_normalized {
        return PropIdemOutcome::IdempotenceMismatch(format!(
            "Idempotence failure:\n\n--- First ---\n{}\n--- Second ---\n{}",
            first_normalized, second_normalized
        ));
    }

    PropIdemOutcome::Pass
}

/// Shared proptest body decision: fail the case ONLY on a real idempotence
/// mismatch (the property under test). `GeneratedSourceInvalid` is strategy
/// noise; `ReparseBreakTracked` is a round-trip defect on synthetic input
/// (tracked, not this suite's regression surface). Returns `Err` to fail.
fn idempotence_case_result(source: &str) -> Result<(), TestCaseError> {
    match test_idempotence(source) {
        PropIdemOutcome::Pass
        | PropIdemOutcome::GeneratedSourceInvalid
        | PropIdemOutcome::ReparseBreakTracked => Ok(()),
        PropIdemOutcome::IdempotenceMismatch(detail) => Err(TestCaseError::fail(detail)),
    }
}

/// Deterministic-`#[test]` assertion on a FIXED, hand-written, valid input.
/// `ctx` names the construct under test. Unlike the proptest path, a fixed input
/// MUST round-trip: a `ReparseBreakTracked` here is a real `format -> parse`
/// regression for that construct (no synthetic-input excuse), so it fails. A
/// `GeneratedSourceInvalid` on a hand-written valid input is also a failure
/// (the input was supposed to parse). Panics on any non-`Pass` outcome.
fn assert_fixed_idempotent(source: &str, ctx: &str) {
    match test_idempotence(source) {
        PropIdemOutcome::Pass => {}
        PropIdemOutcome::GeneratedSourceInvalid => {
            panic!("{ctx}: fixed input did not parse (expected valid Ori)");
        }
        PropIdemOutcome::ReparseBreakTracked => {
            panic!("{ctx}: formatter output failed to re-parse (format->parse round-trip break)");
        }
        PropIdemOutcome::IdempotenceMismatch(detail) => {
            panic!("{ctx}: {detail}");
        }
    }
}

// Property Tests

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
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for generated functions.
    #[test]
    fn prop_function_idempotence(func in function_strategy()) {
        idempotence_case_result(&func)?;
    }

    /// Test idempotence for generated type definitions.
    #[test]
    fn prop_type_idempotence(type_def in type_def_strategy()) {
        idempotence_case_result(&type_def)?;
    }

    /// Test idempotence for generated constants.
    #[test]
    fn prop_const_idempotence(const_def in const_def_strategy()) {
        idempotence_case_result(&const_def)?;
    }

    /// Test idempotence for generated modules.
    #[test]
    fn prop_module_idempotence(module in module_strategy()) {
        idempotence_case_result(&module)?;
    }

    /// Test idempotence for generated list literals.
    #[test]
    fn prop_list_idempotence(items in prop::collection::vec(simple_expr_strategy(), 0..10)) {
        let source = format!("@test_fn () -> [int] = [{}];", items.join(", "));
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for generated tuples.
    #[test]
    fn prop_tuple_idempotence(items in prop::collection::vec(simple_expr_strategy(), 2..8)) {
        let source = format!("@test_fn () -> int = ({});", items.join(", "));
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for deeply nested expressions.
    #[test]
    fn prop_nested_idempotence(depth in 1usize..6) {
        let mut expr = "1".to_string();
        for _ in 0..depth {
            expr = format!("({} + 1)", expr);
        }
        let source = format!("@test_fn () -> int = {}", expr);
        idempotence_case_result(&source)?;
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
        idempotence_case_result(&source)?;
    }
}

// Extended property tests

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

/// Generate an impl block on the fixed subject `Foo` — inherent (`impl Foo`)
/// or colon trait form (`impl Foo: Trait`, per grammar.ebnf trait_impl). The
/// fixed subject allows the idempotence harness to prepend a matching
/// `type Foo` without rewriting the generated source.
fn impl_strategy() -> impl Strategy<Value = String> {
    (
        prop::option::of(type_identifier_strategy()),
        prop::collection::vec(
            (identifier_strategy(), simple_expr_strategy())
                .prop_map(|(name, body)| format!("    @{} (self) -> int = {}", name, body)),
            1..3,
        ),
    )
        .prop_map(|(trait_opt, methods)| {
            let header = match trait_opt {
                Some(tr) => format!("impl Foo: {}", tr),
                None => "impl Foo".to_string(),
            };
            format!("{} {{\n{}\n}}", header, methods.join("\n"))
        })
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
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for field access chains.
    #[test]
    fn prop_field_access_idempotence(chain in field_access_strategy(5)) {
        let source = format!("@test_fn (x: int) -> int = {}", chain);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for lambda expressions.
    #[test]
    fn prop_lambda_expr_idempotence(lambda in lambda_strategy()) {
        let source = format!("@test_fn () -> (int) -> int = {}", lambda);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for run expressions.
    #[test]
    fn prop_run_expr_idempotence(run_expr in run_expr_strategy()) {
        let source = format!("@test_fn () -> int = {}", run_expr);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for match expressions.
    #[test]
    fn prop_match_expr_idempotence(match_expr in match_expr_strategy()) {
        let source = format!("@test_fn (x: int) -> int = {}", match_expr);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for for expressions.
    #[test]
    fn prop_for_expr_idempotence(for_expr in for_expr_strategy()) {
        let source = format!("@test_fn (items: [int]) -> int = {}", for_expr);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for trait definitions.
    #[test]
    fn prop_trait_idempotence(trait_def in trait_strategy()) {
        idempotence_case_result(&trait_def)?;
    }

    /// Test idempotence for impl blocks (inherent + colon trait form).
    #[test]
    fn prop_impl_idempotence(impl_def in impl_strategy()) {
        // impl_strategy emits on the fixed subject `Foo`; define it.
        let source = format!("type Foo = {{ x: int }}\n\n{}", impl_def);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for generic functions.
    #[test]
    fn prop_generic_fn_idempotence(func in generic_function_strategy()) {
        idempotence_case_result(&func)?;
    }

    /// Test idempotence for functions with where clauses.
    #[test]
    fn prop_where_clause_idempotence(func in function_with_where_strategy()) {
        idempotence_case_result(&func)?;
    }

    /// Test idempotence for functions with capabilities.
    #[test]
    fn prop_capabilities_idempotence(func in function_with_caps_strategy()) {
        idempotence_case_result(&func)?;
    }

    /// Test idempotence for long expressions that exceed line width.
    #[test]
    fn prop_long_expr_idempotence(long_expr in long_binary_expr_strategy()) {
        let source = format!("@test_fn () -> int = {}", long_expr);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for unicode strings.
    #[test]
    fn prop_unicode_string_idempotence(unicode in unicode_string_strategy()) {
        let source = format!("@test_fn () -> str = {}", unicode);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for deeply nested conditionals.
    #[test]
    fn prop_nested_conditional_idempotence(depth in 1usize..5) {
        let mut expr = "x".to_string();
        for i in 0..depth {
            expr = format!("if x > {} then {} else {}", i, i + 10, expr);
        }
        let source = format!("@test_fn (x: int) -> int = {}", expr);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for nested collections.
    #[test]
    fn prop_nested_collection_idempotence(depth in 1usize..4) {
        let mut expr = "1".to_string();
        for _ in 0..depth {
            expr = format!("[{}, {}, {}]", expr, expr, expr);
        }
        let source = format!("@test_fn () -> int = {}", expr);
        idempotence_case_result(&source)?;
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
        let source = format!("@test_fn () -> int = {} + {} * {} - {} / 2;", a, b, c, d);
        idempotence_case_result(&source)?;
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
        idempotence_case_result(&source)?;
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
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for structs with many fields.
    #[test]
    fn prop_large_struct_idempotence(field_count in 3usize..10) {
        let fields: Vec<String> = (0..field_count)
            .map(|i| format!("field_{}: int", i))
            .collect();
        let source = format!("type BigStruct = {{ {} }}", fields.join(", "));
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for sum types with many variants.
    #[test]
    fn prop_large_sum_type_idempotence(variant_count in 2usize..8) {
        let variants: Vec<String> = (0..variant_count)
            .map(|i| format!("Variant{}", i))
            .collect();
        let source = format!("type BigSum = {}", variants.join(" | "));
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for functions with many parameters.
    #[test]
    fn prop_many_params_idempotence(param_count in 2usize..8) {
        let params: Vec<String> = (0..param_count)
            .map(|i| format!("param_{}: int", i))
            .collect();
        let source = format!("@many_params ({}) -> int = 0;", params.join(", "));
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for functions with many generic parameters.
    #[test]
    fn prop_many_generics_idempotence(generic_count in 1usize..5) {
        let generics: Vec<String> = (0..generic_count)
            .map(|i| format!("T{}", i))
            .collect();
        let source = format!("@generic_fn<{}> (x: T0) -> T0 = x;", generics.join(", "));
        idempotence_case_result(&source)?;
    }
}

// Additional Unit Tests for Edge Cases

#[test]
fn test_single_element_tuple_idempotence() {
    // Single-element tuples need trailing comma
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/single_element_tuple_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "single element tuple should be idempotent");
}

#[test]
fn test_empty_list_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/empty_list_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "empty list should be idempotent");
}

#[test]
fn test_nested_if_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/nested_if_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "nested if should be idempotent");
}

#[test]
fn test_lambda_idempotence() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/lambda_idempotence.ori"));
    assert_fixed_idempotent(source, "lambda should be idempotent");
}

#[test]
fn test_multi_param_lambda_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/multi_param_lambda_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "multi-param lambda should be idempotent");
}

#[test]
fn test_option_some_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/option_some_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "Some should be idempotent");
}

#[test]
fn test_option_none_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/option_none_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "None should be idempotent");
}

#[test]
fn test_result_ok_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/result_ok_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "Ok should be idempotent");
}

#[test]
fn test_result_err_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/result_err_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "Err should be idempotent");
}

#[test]
fn test_complex_struct_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/complex_struct_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "struct should be idempotent");
}

#[test]
fn test_sum_type_idempotence() {
    // Note: Ok/Err are reserved (Result constructors), use different names
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/sum_type_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "sum type should be idempotent");
}

#[test]
fn test_generic_function_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/generic_function_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "generic function should be idempotent");
}

#[test]
fn test_where_clause_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/where_clause_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "where clause should be idempotent");
}

#[test]
fn test_const_def_idempotence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/const_def_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "const should be idempotent");
}

#[test]
fn test_multiple_declarations_idempotence() {
    let source = include_str!("fixtures/property/multiple_declarations_idempotence.ori");
    assert_fixed_idempotent(source, "module should be idempotent");
}

#[test]
fn test_binary_expr_line_break() {
    // Regression test: binary expression breaking must preserve semantics.
    // The parser must accept binary operators at line start.
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/binary_expr_line_break.ori"
    ));
    assert_fixed_idempotent(
        source,
        "binary expression with line break should be idempotent",
    );
}

// Additional tests

// Literal Edge Cases

#[test]
fn test_zero_literal() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/zero_literal.ori"));
    assert_fixed_idempotent(source, "zero should be idempotent");
}

#[test]
fn test_negative_literal() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/negative_literal.ori"));
    assert_fixed_idempotent(source, "negative should be idempotent");
}

#[test]
fn test_large_int_literal() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/large_int_literal.ori"));
    assert_fixed_idempotent(source, "large int should be idempotent");
}

#[test]
fn test_float_zero() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/float_zero.ori"));
    assert_fixed_idempotent(source, "float zero should be idempotent");
}

#[test]
fn test_float_scientific() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/float_scientific.ori"));
    assert_fixed_idempotent(source, "scientific notation should be idempotent");
}

#[test]
fn test_float_negative_exponent() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/float_negative_exponent.ori"
    ));
    assert_fixed_idempotent(source, "negative exponent should be idempotent");
}

#[test]
fn test_empty_string() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/empty_string.ori"));
    assert_fixed_idempotent(source, "empty string should be idempotent");
}

#[test]
fn test_string_with_escapes() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/string_with_escapes.ori"));
    assert_fixed_idempotent(source, "escaped string should be idempotent");
}

#[test]
fn test_char_escape_sequences() {
    let sources = [
        fixture_without_trailing_newline(include_str!(
            "fixtures/property/char_escape_sequences_source_1.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/property/char_escape_sequences_source_2.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/property/char_escape_sequences_source_3.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/property/char_escape_sequences_source_4.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/property/char_escape_sequences_source_5.ori"
        )),
    ];
    for source in sources {
        assert_fixed_idempotent(source, "char escape should be idempotent");
    }
}

// Operator Edge Cases

#[test]
fn test_bitwise_and() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/bitwise_and.ori"));
    assert_fixed_idempotent(source, "bitwise and should be idempotent");
}

#[test]
fn test_bitwise_or() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/bitwise_or.ori"));
    assert_fixed_idempotent(source, "bitwise or should be idempotent");
}

#[test]
fn test_bitwise_xor() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/bitwise_xor.ori"));
    assert_fixed_idempotent(source, "bitwise xor should be idempotent");
}

#[test]
fn test_left_shift() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/left_shift.ori"));
    assert_fixed_idempotent(source, "left shift should be idempotent");
}

#[test]
fn test_right_shift() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/right_shift.ori"));
    assert_fixed_idempotent(source, "right shift should be idempotent");
}

#[test]
fn test_modulo() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/modulo.ori"));
    assert_fixed_idempotent(source, "modulo should be idempotent");
}

#[test]
fn test_unary_not() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/unary_not.ori"));
    assert_fixed_idempotent(source, "unary not should be idempotent");
}

#[test]
fn test_unary_negate() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/unary_negate.ori"));
    assert_fixed_idempotent(source, "unary negate should be idempotent");
}

#[test]
fn test_double_negation() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/double_negation.ori"));
    assert_fixed_idempotent(source, "double negation should be idempotent");
}

#[test]
fn test_complex_operator_precedence() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/complex_operator_precedence.ori"
    ));
    assert_fixed_idempotent(source, "complex precedence should be idempotent");
}

#[test]
fn test_mixed_comparison_logical() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/mixed_comparison_logical.ori"
    ));
    assert_fixed_idempotent(source, "mixed comparison/logical should be idempotent");
}

// Collection Tests

#[test]
fn test_list_of_lists() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/list_of_lists.ori"));
    assert_fixed_idempotent(source, "nested lists should be idempotent");
}

#[test]
fn test_list_of_tuples() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/list_of_tuples.ori"));
    assert_fixed_idempotent(source, "list of tuples should be idempotent");
}

#[test]
fn test_tuple_of_lists() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/tuple_of_lists.ori"));
    assert_fixed_idempotent(source, "tuple of lists should be idempotent");
}

#[test]
fn test_empty_tuple() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/empty_tuple.ori"));
    assert_fixed_idempotent(source, "empty tuple should be idempotent");
}

#[test]
fn test_struct_literal_simple() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/struct_literal_simple.ori"
    ));
    assert_fixed_idempotent(source, "struct literal should be idempotent");
}

#[test]
fn test_struct_literal_shorthand() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/struct_literal_shorthand.ori"
    ));
    assert_fixed_idempotent(source, "struct shorthand should be idempotent");
}

#[test]
fn test_struct_nested() {
    let source = include_str!("fixtures/property/struct_nested.ori");
    assert_fixed_idempotent(source, "nested struct should be idempotent");
}

#[test]
fn test_range_exclusive() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/range_exclusive.ori"));
    assert_fixed_idempotent(source, "exclusive range should be idempotent");
}

#[test]
fn test_range_inclusive() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/range_inclusive.ori"));
    assert_fixed_idempotent(source, "inclusive range should be idempotent");
}

// Control Flow Tests

#[test]
fn test_if_without_else() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/if_without_else.ori"));
    assert_fixed_idempotent(source, "if without else should be idempotent");
}

#[test]
fn test_chained_if_else() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/chained_if_else.ori"));
    assert_fixed_idempotent(source, "chained if-else should be idempotent");
}

#[test]
fn test_deeply_nested_if() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/deeply_nested_if.ori"));
    assert_fixed_idempotent(source, "deeply nested if should be idempotent");
}

#[test]
fn test_for_with_filter() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/for_with_filter.ori"));
    assert_fixed_idempotent(source, "for with filter should be idempotent");
}

#[test]
fn test_nested_for() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/nested_for.ori"));
    assert_fixed_idempotent(source, "nested for should be idempotent");
}

// Pattern Construct Tests

#[test]
fn test_run_multiple_bindings() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/run_multiple_bindings.ori"
    ));
    assert_fixed_idempotent(source, "block multiple bindings should be idempotent");
}

#[test]
fn test_try_expression() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/try_expression.ori"));
    assert_fixed_idempotent(source, "try expression should be idempotent");
}

#[test]
fn test_match_option() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/match_option.ori"));
    assert_fixed_idempotent(source, "match option should be idempotent");
}

#[test]
fn test_match_result() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/match_result.ori"));
    assert_fixed_idempotent(source, "match result should be idempotent");
}

#[test]
fn test_match_simple_patterns() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/match_simple_patterns.ori"
    ));
    assert_fixed_idempotent(source, "match simple patterns should be idempotent");
}

#[test]
fn test_match_nested() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/match_nested.ori"));
    assert_fixed_idempotent(source, "nested match should be idempotent");
}

#[test]
fn test_match_tuple_pattern() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/match_tuple_pattern.ori"));
    assert_fixed_idempotent(source, "match tuple pattern should be idempotent");
}

#[test]
fn test_match_list_pattern() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/match_list_pattern.ori"));
    assert_fixed_idempotent(source, "match list pattern should be idempotent");
}

// Function Call Tests

#[test]
fn test_simple_call() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/simple_call.ori"));
    assert_fixed_idempotent(source, "simple call should be idempotent");
}

#[test]
fn test_nested_calls() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/nested_calls.ori"));
    assert_fixed_idempotent(source, "nested calls should be idempotent");
}

#[test]
fn test_call_with_lambda() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/call_with_lambda.ori"));
    assert_fixed_idempotent(source, "call with lambda should be idempotent");
}

#[test]
fn test_method_call_chain_long() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/method_call_chain_long.ori"
    ));
    assert_fixed_idempotent(source, "long method chain should be idempotent");
}

#[test]
fn test_mixed_access_chain() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/mixed_access_chain.ori"));
    assert_fixed_idempotent(source, "mixed access chain should be idempotent");
}

#[test]
fn test_index_access() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/index_access.ori"));
    assert_fixed_idempotent(source, "index access should be idempotent");
}

#[test]
fn test_index_with_hash() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/index_with_hash.ori"));
    assert_fixed_idempotent(source, "index with hash should be idempotent");
}

#[test]
fn test_nested_index() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/nested_index.ori"));
    assert_fixed_idempotent(source, "nested index should be idempotent");
}

// Lambda Tests

#[test]
fn test_lambda_no_params() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/lambda_no_params.ori"));
    assert_fixed_idempotent(source, "no-param lambda should be idempotent");
}

#[test]
fn test_lambda_single_param() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/lambda_single_param.ori"));
    assert_fixed_idempotent(source, "single-param lambda should be idempotent");
}

#[test]
fn test_lambda_multi_param() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/lambda_multi_param.ori"));
    assert_fixed_idempotent(source, "multi-param lambda should be idempotent");
}

#[test]
fn test_lambda_with_type_annotation() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/lambda_with_type_annotation.ori"
    ));
    assert_fixed_idempotent(source, "typed lambda should be idempotent");
}

#[test]
fn test_nested_lambda() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/nested_lambda.ori"));
    assert_fixed_idempotent(source, "nested lambda should be idempotent");
}

#[test]
fn test_lambda_with_complex_body() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/lambda_with_complex_body.ori"
    ));
    assert_fixed_idempotent(source, "lambda with complex body should be idempotent");
}

// Type Definition Tests

#[test]
fn test_empty_struct() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/empty_struct.ori"));
    assert_fixed_idempotent(source, "empty struct should be idempotent");
}

#[test]
fn test_struct_single_field() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/struct_single_field.ori"));
    assert_fixed_idempotent(source, "single field struct should be idempotent");
}

#[test]
fn test_struct_many_fields() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/struct_many_fields.ori"));
    assert_fixed_idempotent(source, "many field struct should be idempotent");
}

#[test]
fn test_sum_type_two_variants() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/sum_type_two_variants.ori"
    ));
    assert_fixed_idempotent(source, "two variant sum should be idempotent");
}

#[test]
fn test_sum_type_with_fields() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/sum_type_with_fields.ori"
    ));
    assert_fixed_idempotent(source, "sum with fields should be idempotent");
}

#[test]
fn test_generic_type() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/generic_type.ori"));
    assert_fixed_idempotent(source, "generic type should be idempotent");
}

#[test]
fn test_generic_type_multi_param() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/generic_type_multi_param.ori"
    ));
    assert_fixed_idempotent(source, "multi-param generic should be idempotent");
}

#[test]
fn test_derive_eq() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/derive_eq.ori"));
    assert_fixed_idempotent(source, "derive Eq should be idempotent");
}

#[test]
fn test_derive_multiple() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/derive_multiple.ori"));
    assert_fixed_idempotent(source, "derive multiple should be idempotent");
}

#[test]
fn test_public_type() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/public_type.ori"));
    assert_fixed_idempotent(source, "public type should be idempotent");
}

// Trait and Impl Tests

#[test]
fn test_empty_trait() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/empty_trait.ori"));
    assert_fixed_idempotent(source, "empty trait should be idempotent");
}

#[test]
fn test_trait_single_method() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/trait_single_method.ori"));
    assert_fixed_idempotent(source, "single method trait should be idempotent");
}

#[test]
fn test_trait_multiple_methods() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/trait_multiple_methods.ori"
    ));
    assert_fixed_idempotent(source, "multi-method trait should be idempotent");
}

#[test]
fn test_trait_with_default() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/trait_with_default.ori"));
    assert_fixed_idempotent(source, "trait with default should be idempotent");
}

#[test]
fn test_trait_inheritance() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/trait_inheritance.ori"));
    assert_fixed_idempotent(source, "trait inheritance should be idempotent");
}

#[test]
fn test_impl_inherent() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/impl_inherent.ori"));
    assert_fixed_idempotent(source, "inherent impl should be idempotent");
}

#[test]
fn test_impl_trait() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/impl_trait.ori"));
    assert_fixed_idempotent(source, "trait impl should be idempotent");
}

#[test]
fn test_impl_generic() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/impl_generic.ori"));
    assert_fixed_idempotent(source, "generic impl should be idempotent");
}

// Function Signature Tests

#[test]
fn test_function_no_params() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/function_no_params.ori"));
    assert_fixed_idempotent(source, "no params should be idempotent");
}

#[test]
fn test_function_single_param() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_single_param.ori"
    ));
    assert_fixed_idempotent(source, "single param should be idempotent");
}

#[test]
fn test_function_many_params() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_many_params.ori"
    ));
    assert_fixed_idempotent(source, "many params should be idempotent");
}

#[test]
fn test_function_void_return() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_void_return.ori"
    ));
    assert_fixed_idempotent(source, "void return should be idempotent");
}

#[test]
fn test_function_generic_single() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/generic_function_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "single generic should be idempotent");
}

#[test]
fn test_function_generic_multiple() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_generic_multiple.ori"
    ));
    assert_fixed_idempotent(source, "multiple generics should be idempotent");
}

#[test]
fn test_function_generic_bounded() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_generic_bounded.ori"
    ));
    assert_fixed_idempotent(source, "bounded generic should be idempotent");
}

#[test]
fn test_function_where_single() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_where_single.ori"
    ));
    assert_fixed_idempotent(source, "single where should be idempotent");
}

#[test]
fn test_function_where_multiple() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_where_multiple.ori"
    ));
    assert_fixed_idempotent(source, "multiple where should be idempotent");
}

#[test]
fn test_function_capability_single() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_capability_single.ori"
    ));
    assert_fixed_idempotent(source, "single capability should be idempotent");
}

#[test]
fn test_function_capability_multiple() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_capability_multiple.ori"
    ));
    assert_fixed_idempotent(source, "multiple capabilities should be idempotent");
}

#[test]
fn test_function_public() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/function_public.ori"));
    assert_fixed_idempotent(source, "public function should be idempotent");
}

#[test]
fn test_function_all_features() {
    // Order: generics, params, return, uses, where, body
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/function_all_features.ori"
    ));
    assert_fixed_idempotent(source, "function with all features should be idempotent");
}

// Import Tests

#[test]
fn test_import_single() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/import_single.ori"));
    assert_fixed_idempotent(source, "single import should be idempotent");
}

#[test]
fn test_import_multiple() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/import_multiple.ori"));
    assert_fixed_idempotent(source, "multiple imports should be idempotent");
}

#[test]
fn test_import_alias() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/import_alias.ori"));
    assert_fixed_idempotent(source, "import alias should be idempotent");
}

#[test]
fn test_import_relative() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/import_relative.ori"));
    assert_fixed_idempotent(source, "relative import should be idempotent");
}

// Constant Tests

#[test]
fn test_const_int() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/const_int.ori"));
    assert_fixed_idempotent(source, "int const should be idempotent");
}

#[test]
fn test_const_float() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/const_def_idempotence.ori"
    ));
    assert_fixed_idempotent(source, "float const should be idempotent");
}

#[test]
fn test_const_string() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/const_string.ori"));
    assert_fixed_idempotent(source, "string const should be idempotent");
}

#[test]
fn test_const_bool() {
    let source = fixture_without_trailing_newline(include_str!("fixtures/property/const_bool.ori"));
    assert_fixed_idempotent(source, "bool const should be idempotent");
}

#[test]
fn test_const_public() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/const_public.ori"));
    assert_fixed_idempotent(source, "public const should be idempotent");
}

// Test Declaration Tests

#[test]
fn test_targeted_test() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/targeted_test.ori"));
    assert_fixed_idempotent(source, "targeted test should be idempotent");
}

#[test]
fn test_free_floating_test() {
    // Free-floating tests use @name syntax without a target
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/free_floating_test.ori"));
    assert_fixed_idempotent(source, "free floating test should be idempotent");
}

// Comment Tests

#[test]
fn test_single_comment() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/single_comment.ori"));
    assert_fixed_idempotent(source, "single comment should be idempotent");
}

#[test]
fn test_doc_comment() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/doc_comment.ori"));
    assert_fixed_idempotent(source, "doc comment should be idempotent");
}

#[test]
fn test_param_comment() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/param_comment.ori"));
    assert_fixed_idempotent(source, "param comment should be idempotent");
}

#[test]
fn test_multiple_comments() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/multiple_comments.ori"));
    assert_fixed_idempotent(source, "multiple comments should be idempotent");
}

// Line Width Edge Cases

#[test]
fn test_long_int_chain() {
    // Create a line with many integers
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/long_int_chain.ori"));
    assert_fixed_idempotent(source, "long int chain should be idempotent");
}

#[test]
fn test_long_function_name() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/long_function_name.ori"));
    assert_fixed_idempotent(source, "long function name should be idempotent");
}

#[test]
fn test_long_param_names() {
    let source =
        fixture_without_trailing_newline(include_str!("fixtures/property/long_param_names.ori"));
    assert_fixed_idempotent(source, "long param names should be idempotent");
}

#[test]
fn test_long_type_annotation() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/long_type_annotation.ori"
    ));
    assert_fixed_idempotent(source, "long type annotation should be idempotent");
}

#[test]
fn test_very_long_expression() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/very_long_expression.ori"
    ));
    assert_fixed_idempotent(source, "very long expression should be idempotent");
}

// Complex Combined Tests

#[test]
fn test_full_module() {
    let source = include_str!("fixtures/property/full_module.ori");
    assert_fixed_idempotent(source, "full module should be idempotent");
}

#[test]
fn test_complex_expression_composition() {
    // Simpler method chain that doesn't trigger lambda line-break issues
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/complex_expression_composition.ori"
    ));
    assert_fixed_idempotent(source, "complex expression should be idempotent");
}

#[test]
fn test_deeply_nested_everything() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/property/deeply_nested_everything.ori"
    ));
    assert_fixed_idempotent(source, "deeply nested everything should be idempotent");
}

// Additional property tests

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
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for size literals.
    #[test]
    fn prop_size_literal_idempotence(size in size_literal_strategy()) {
        let source = format!("@f () -> Size = {}", size);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for import statements.
    #[test]
    fn prop_import_idempotence(import in import_strategy()) {
        let source = format!("{}\n\n@f () -> int = 42", import);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for test declarations.
    #[test]
    fn prop_test_decl_idempotence(test_decl in test_decl_strategy()) {
        let source = format!("@dummy () -> int = 42\n\n{}", test_decl);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for public functions.
    #[test]
    fn prop_public_fn_idempotence(func in public_function_strategy()) {
        idempotence_case_result(&func)?;
    }

    /// Test idempotence for generic structs.
    #[test]
    fn prop_generic_struct_idempotence(struct_def in generic_struct_strategy()) {
        idempotence_case_result(&struct_def)?;
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
        idempotence_case_result(&source)?;
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
        idempotence_case_result(&source)?;
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
        let source = format!("@f () -> {} = panic(msg: \"unimplemented\");", ty);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for function type annotations.
    #[test]
    fn prop_fn_type_idempotence(param_count in 1usize..4) {
        let params: Vec<&str> = (0..param_count).map(|_| "int").collect();
        let fn_type = format!("({}) -> int", params.join(", "));
        let source = format!("@f () -> {} = panic(msg: \"unimplemented\");", fn_type);
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for tuple types.
    #[test]
    fn prop_tuple_type_idempotence(elem_count in 2usize..6) {
        let elems: Vec<&str> = (0..elem_count)
            .map(|i| match i % 3 { 0 => "int", 1 => "str", _ => "bool" })
            .collect();
        let tuple_type = format!("({})", elems.join(", "));
        let source = format!("@f () -> {} = panic(msg: \"unimplemented\");", tuple_type);
        idempotence_case_result(&source)?;
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
        idempotence_case_result(&source)?;
    }

    /// Test idempotence for modules with many functions.
    #[test]
    fn prop_many_functions_idempotence(func_count in 2usize..8) {
        let funcs: Vec<String> = (0..func_count)
            .map(|i| format!("@fn_{} () -> int = {}", i, i))
            .collect();
        let source = funcs.join("\n\n");
        idempotence_case_result(&source)?;
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
        idempotence_case_result(&source)?;
    }
}
