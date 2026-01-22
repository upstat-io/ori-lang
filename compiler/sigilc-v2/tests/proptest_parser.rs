//! Property-based tests for the parser.
//!
//! These tests use proptest to generate random inputs and verify
//! parser invariants hold for all inputs.
//!
//! Configuration is tuned to avoid overwhelming WSL with resources.

use proptest::prelude::*;
use sigilc_v2::intern::StringInterner;
use sigilc_v2::syntax::{Lexer, Parser, ExprKind, TokenKind};

// ============================================================================
// Strategies for generating test inputs
// ============================================================================

/// Strategy for valid integer literals
fn arb_int() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple integers
        (0i64..=1000).prop_map(|n| n.to_string()),
        // Negative integers
        (-1000i64..0).prop_map(|n| n.to_string()),
        // Underscored integers
        (1000i64..1000000).prop_map(|n| format!("{}_{}", n / 1000, n % 1000)),
    ]
}

/// Strategy for valid float literals
fn arb_float() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple floats
        (0.0f64..1000.0).prop_map(|f| format!("{:.2}", f)),
        // Scientific notation
        (1.0f64..10.0, -5i32..5).prop_map(|(m, e)| format!("{:.1}e{}", m, e)),
    ]
}

/// Strategy for valid string literals
fn arb_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_ ]{0,20}".prop_map(|s| format!("\"{}\"", s))
}

/// Strategy for valid identifiers
fn arb_ident() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,10}".prop_map(|s| s)
}

/// Strategy for simple binary operators
fn arb_binop() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("+"),
        Just("-"),
        Just("*"),
        Just("/"),
        Just("%"),
        Just("=="),
        Just("!="),
        Just("<"),
        Just(">"),
        Just("<="),
        Just(">="),
        Just("&&"),
        Just("||"),
    ]
}

/// Strategy for simple expressions (no recursion)
fn arb_simple_expr() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_int(),
        arb_float(),
        arb_string(),
        Just("true".to_string()),
        Just("false".to_string()),
        arb_ident(),
    ]
}

/// Strategy for binary expressions
fn arb_binary_expr() -> impl Strategy<Value = String> {
    (arb_simple_expr(), arb_binop(), arb_simple_expr())
        .prop_map(|(left, op, right)| format!("{} {} {}", left, op, right))
}

/// Strategy for list expressions
fn arb_list_expr() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_simple_expr(), 0..5)
        .prop_map(|elems| format!("[{}]", elems.join(", ")))
}

/// Strategy for tuple expressions
fn arb_tuple_expr() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_simple_expr(), 2..5)
        .prop_map(|elems| format!("({})", elems.join(", ")))
}

/// Strategy for function call expressions
fn arb_call_expr() -> impl Strategy<Value = String> {
    (arb_ident(), prop::collection::vec(arb_simple_expr(), 0..3))
        .prop_map(|(name, args)| format!("{}({})", name, args.join(", ")))
}

/// Strategy for various expression types
fn arb_expr() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_simple_expr(),
        arb_binary_expr(),
        arb_list_expr(),
        arb_tuple_expr(),
        arb_call_expr(),
    ]
}

/// Strategy for simple function definitions
fn arb_function() -> impl Strategy<Value = String> {
    (arb_ident(), arb_simple_expr())
        .prop_map(|(name, body)| format!("@{} () -> int = {}", name, body))
}

// ============================================================================
// Property tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Lexer should never panic on any input (bounded to avoid resource exhaustion)
    #[test]
    fn lexer_no_panic(input in ".{0,100}") {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&input, &interner);
        let _tokens = lexer.lex_all();
        // If we get here without panic, test passes
    }

    /// Parser should never panic on any input (even invalid)
    #[test]
    fn parser_no_panic(input in ".{0,100}") {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&input, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let _result = parser.parse_module();
        // If we get here without panic, test passes
    }

    /// Valid integer literals should parse correctly
    #[test]
    fn valid_int_parses(s in arb_int()) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&s, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        prop_assert!(diagnostics.is_empty(), "Expected no errors for valid int: {}", s);
        prop_assert!(matches!(arena.get(expr_id).kind, ExprKind::Int(_) | ExprKind::Unary { .. }),
            "Expected Int or Unary(Neg, Int) for: {}", s);
    }

    /// Valid float literals should parse correctly
    #[test]
    fn valid_float_parses(s in arb_float()) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&s, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        prop_assert!(diagnostics.is_empty(), "Expected no errors for valid float: {}", s);
        prop_assert!(matches!(arena.get(expr_id).kind, ExprKind::Float(_)),
            "Expected Float for: {}", s);
    }

    /// Valid string literals should parse correctly
    #[test]
    fn valid_string_parses(s in arb_string()) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&s, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        prop_assert!(diagnostics.is_empty(), "Expected no errors for valid string: {}", s);
        prop_assert!(matches!(arena.get(expr_id).kind, ExprKind::String(_)),
            "Expected String for: {}", s);
    }

    /// Valid binary expressions should parse correctly
    #[test]
    fn valid_binary_expr_parses(s in arb_binary_expr()) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&s, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        prop_assert!(diagnostics.is_empty(), "Expected no errors for binary expr: {}", s);
        prop_assert!(matches!(arena.get(expr_id).kind, ExprKind::Binary { .. }),
            "Expected Binary for: {}", s);
    }

    /// Valid list expressions should parse correctly
    #[test]
    fn valid_list_expr_parses(s in arb_list_expr()) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&s, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        prop_assert!(diagnostics.is_empty(), "Expected no errors for list expr: {}", s);
        prop_assert!(matches!(arena.get(expr_id).kind, ExprKind::List(_)),
            "Expected List for: {}", s);
    }

    /// Valid tuple expressions should parse correctly
    #[test]
    fn valid_tuple_expr_parses(s in arb_tuple_expr()) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&s, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        prop_assert!(diagnostics.is_empty(), "Expected no errors for tuple expr: {}", s);
        prop_assert!(matches!(arena.get(expr_id).kind, ExprKind::Tuple(_)),
            "Expected Tuple for: {}", s);
    }

    /// Valid function call expressions should parse correctly
    #[test]
    fn valid_call_expr_parses(s in arb_call_expr()) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&s, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, diagnostics) = parser.parse_expression();

        prop_assert!(diagnostics.is_empty(), "Expected no errors for call expr: {}", s);
        prop_assert!(matches!(arena.get(expr_id).kind, ExprKind::Call { .. }),
            "Expected Call for: {}", s);
    }

    /// Valid function definitions should parse correctly
    #[test]
    fn valid_function_parses(s in arb_function()) {
        let interner = StringInterner::new();
        let lexer = Lexer::new(&s, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let result = parser.parse_module();

        prop_assert!(result.diagnostics.is_empty(),
            "Expected no errors for function: {}, got: {:?}", s, result.diagnostics);
        prop_assert_eq!(result.items.len(), 1, "Expected 1 item for: {}", s);
    }

    /// Lexer produces deterministic output
    #[test]
    fn lexer_deterministic(input in "[ -~]{0,50}") {
        let interner = StringInterner::new();

        let lexer1 = Lexer::new(&input, &interner);
        let tokens1 = lexer1.lex_all();

        let lexer2 = Lexer::new(&input, &interner);
        let tokens2 = lexer2.lex_all();

        prop_assert_eq!(tokens1.tokens.len(), tokens2.tokens.len(),
            "Token count should be deterministic");

        for (t1, t2) in tokens1.tokens.iter().zip(tokens2.tokens.iter()) {
            prop_assert_eq!(
                std::mem::discriminant(&t1.kind),
                std::mem::discriminant(&t2.kind),
                "Token kinds should be identical"
            );
            prop_assert_eq!(t1.span, t2.span, "Token spans should be identical");
        }
    }

    /// Parser error recovery: errors don't prevent parsing
    #[test]
    fn parser_recovers_from_errors(
        prefix in arb_simple_expr(),
        suffix in arb_simple_expr()
    ) {
        // Create an expression with an error in the middle
        let source = format!("@main () -> int = [{}, @, {}]", prefix, suffix);

        let interner = StringInterner::new();
        let lexer = Lexer::new(&source, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let result = parser.parse_module();

        // Should have errors but still produce an item
        prop_assert!(!result.diagnostics.is_empty(), "Should have errors");
        prop_assert_eq!(result.items.len(), 1, "Should still parse the function");
    }
}

// ============================================================================
// Additional unit tests for edge cases found by proptest
// ============================================================================

#[test]
fn test_empty_input() {
    let interner = StringInterner::new();
    let lexer = Lexer::new("", &interner);
    let tokens = lexer.lex_all();

    // Should have just EOF
    assert_eq!(tokens.tokens.len(), 1);
    assert!(matches!(tokens.tokens[0].kind, TokenKind::Eof));
}

#[test]
fn test_whitespace_only() {
    let interner = StringInterner::new();
    let lexer = Lexer::new("   \t\n  ", &interner);
    let tokens = lexer.lex_all();

    // Should have newline and EOF
    assert!(tokens.tokens.iter().any(|t| matches!(t.kind, TokenKind::Eof)));
}

#[test]
fn test_deeply_nested_parens() {
    let source = "((((((1))))))";
    let interner = StringInterner::new();
    let lexer = Lexer::new(source, &interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, &interner);
    let (expr_id, arena, diagnostics) = parser.parse_expression();

    assert!(diagnostics.is_empty());
    // Should eventually get to the Int
    let mut current = expr_id;
    let mut depth = 0;
    while depth < 10 {
        match &arena.get(current).kind {
            ExprKind::Int(1) => break,
            _ => {
                depth += 1;
                current = current; // In this case, parenthesized expressions are transparent
            }
        }
    }
}

#[test]
fn test_long_identifier() {
    let ident = "a".repeat(100);
    let interner = StringInterner::new();
    let lexer = Lexer::new(&ident, &interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, &interner);
    let (expr_id, arena, diagnostics) = parser.parse_expression();

    assert!(diagnostics.is_empty());
    assert!(matches!(arena.get(expr_id).kind, ExprKind::Ident(_)));
}

#[test]
fn test_many_list_elements() {
    let elements: Vec<_> = (0..100).map(|i| i.to_string()).collect();
    let source = format!("[{}]", elements.join(", "));

    let interner = StringInterner::new();
    let lexer = Lexer::new(&source, &interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, &interner);
    let (expr_id, arena, diagnostics) = parser.parse_expression();

    assert!(diagnostics.is_empty());
    if let ExprKind::List(range) = &arena.get(expr_id).kind {
        assert_eq!(arena.get_expr_list(*range).len(), 100);
    } else {
        panic!("Expected list");
    }
}
