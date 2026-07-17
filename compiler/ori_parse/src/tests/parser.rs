//! Core parser tests.
//!
//! Tests for basic parsing functionality including literals, expressions,
//! operators, capabilities, and context management.

use crate::{parse, ParseContext, ParseOutput, Parser};
use ori_ir::{
    BinaryOp, BindingPattern, ExprKind, FunctionExpKind, FunctionSeq, Mutability, StmtKind,
    StringInterner, UnaryOp,
};

fn parse_source(source: &str) -> ParseOutput {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    parse(&tokens, &interner)
}

fn fixture_without_trailing_newline(fixture: &'static str) -> &'static str {
    let Some(source) = fixture.strip_suffix('\n') else {
        panic!("parser fixtures must end with a newline");
    };
    source
}

#[test]
fn test_parse_literal() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_literal.ori"
    )));

    assert!(!result.has_errors());
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    assert!(matches!(body.kind, ExprKind::Int(42)));
}

#[test]
fn test_parse_binary_expr() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_binary_expr.ori"
    )));

    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    // Should be Add(1, Mul(2, 3)) due to precedence
    if let ExprKind::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = &body.kind
    {
        assert!(matches!(
            result.arena.get_expr(*left).kind,
            ExprKind::Int(1)
        ));

        let right_expr = result.arena.get_expr(*right);
        assert!(matches!(
            right_expr.kind,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    } else {
        panic!("Expected binary add expression");
    }
}

#[test]
fn test_parse_adjacent_unary_negation_right_associatively() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_adjacent_unary_negation_right_associatively.ori"
    )));

    assert!(
        !result.has_errors(),
        "adjacent unary negation should parse cleanly: {:?}",
        result.errors
    );

    let body = result.arena.get_expr(result.module.functions[0].body);
    let ExprKind::Unary {
        op: UnaryOp::Neg,
        operand,
    } = body.kind
    else {
        panic!("expected outer unary negation, got {:?}", body.kind);
    };
    assert!(matches!(
        result.arena.get_expr(operand).kind,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            ..
        }
    ));
}

#[test]
fn test_parse_if_expr() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_if_expr.ori"
    )));

    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::If {
        cond,
        then_branch,
        else_branch,
    } = &body.kind
    {
        assert!(matches!(
            result.arena.get_expr(*cond).kind,
            ExprKind::Bool(true)
        ));
        assert!(matches!(
            result.arena.get_expr(*then_branch).kind,
            ExprKind::Int(1)
        ));
        assert!(else_branch.is_present());
    } else {
        panic!("Expected if expression");
    }
}

#[test]
fn test_parse_block_expr() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_block_expr.ori"
    )));

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Block { stmts, result: res } = &body.kind {
        let stmt_list = result.arena.get_stmt_range(*stmts);
        assert_eq!(stmt_list.len(), 2, "Expected 2 let statements");
        assert!(
            matches!(stmt_list[0].kind, StmtKind::Let { .. }),
            "First stmt should be Let"
        );
        assert!(
            matches!(stmt_list[1].kind, StmtKind::Let { .. }),
            "Second stmt should be Let"
        );
        assert!(res.is_valid(), "Block should have a result expression");
    } else {
        panic!("Expected Block expression, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_let_expression() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_let_expression.ori"
    )));

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors(), "Expected no parse errors");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Let {
        pattern: pattern_id,
        ty,
        mutable,
        ..
    } = &body.kind
    {
        let pattern = result.arena.get_binding_pattern(*pattern_id);
        assert!(matches!(pattern, BindingPattern::Name { .. }));
        assert!(!ty.is_valid());
        // Per spec: let x = v is mutable by default
        assert!(mutable.is_mutable());
    } else {
        panic!("Expected let expression, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_let_with_type() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_let_with_type.ori"
    )));

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Let { ty, .. } = &body.kind {
        assert!(ty.is_valid());
    } else {
        panic!("Expected let expression");
    }
}

#[test]
fn test_parse_block_with_let() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_block_with_let.ori"
    )));

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Block { stmts, result: res } = &body.kind {
        let stmt_list = result.arena.get_stmt_range(*stmts);
        assert_eq!(stmt_list.len(), 1, "Expected 1 let statement");
        assert!(
            matches!(stmt_list[0].kind, StmtKind::Let { .. }),
            "Stmt should be Let"
        );
        assert!(res.is_valid(), "Block should have a result expression");
    } else {
        panic!("Expected Block expression, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_function_exp_print() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_function_exp_print.ori"
    )));

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors(), "Expected no parse errors");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::FunctionExp(exp_id) = &body.kind {
        let func_exp = result.arena.get_function_exp(*exp_id);
        assert!(matches!(func_exp.kind, FunctionExpKind::Print));
        let props = result.arena.get_named_exprs(func_exp.props);
        assert_eq!(props.len(), 1);
    } else {
        panic!("Expected print function_exp, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_timeout_multiline() {
    // Test parsing timeout function_exp with multiline format
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_timeout_multiline.ori"
    )));

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors(), "Expected no parse errors");
}

#[test]
fn test_parse_list() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_list.ori"
    )));

    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::List(range) = &body.kind {
        assert_eq!(range.len(), 3);
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_parse_result_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    let result1 = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_literal.ori"
    )));
    let result2 = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_literal.ori"
    )));
    let result3 = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_result_hash.ori"
    )));

    set.insert(result1);
    set.insert(result2); // duplicate
    set.insert(result3);

    assert_eq!(set.len(), 2);
}

#[test]
fn test_parse_timeout_pattern() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/parse_timeout_pattern.ori"
    )));

    for err in &result.errors {
        eprintln!("Parse error: {err:?}");
    }
    assert!(
        result.errors.is_empty(),
        "Unexpected parse errors: {:?}",
        result.errors
    );
}

#[test]
fn test_parse_block_in_test() {
    // Test block expression syntax in a test function with target
    let result = parse_source(include_str!("fixtures/parser/parse_block_in_test.ori"));

    for err in &result.errors {
        eprintln!("Parse error: {err:?}");
    }
    assert!(
        result.errors.is_empty(),
        "Unexpected parse errors: {:?}",
        result.errors
    );
    assert_eq!(result.module.functions.len(), 1, "Expected 1 function");
    assert_eq!(result.module.tests.len(), 1, "Expected 1 test");
}

#[test]
fn test_at_in_expression_is_error() {
    // @ is only for function definitions, not calls
    // Using @name(...) in an expression should be a syntax error
    let result = parse_source(include_str!(
        "fixtures/parser/at_in_expression_is_error.ori"
    ));

    assert!(
        result.has_errors(),
        "Expected parse error for @add in expression"
    );
}

#[test]
fn test_uses_clause_single_capability() {
    let result = parse_source(include_str!(
        "fixtures/parser/uses_clause_single_capability.ori"
    ));

    assert!(!result.has_errors(), "Expected no parse errors");
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    assert_eq!(func.capabilities.len(), 1);
}

#[test]
fn test_uses_clause_multiple_capabilities() {
    let result = parse_source(include_str!(
        "fixtures/parser/uses_clause_multiple_capabilities.ori"
    ));

    assert!(!result.has_errors(), "Expected no parse errors");
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    assert_eq!(func.capabilities.len(), 2);
}

#[test]
fn test_uses_clause_with_where() {
    // The `uses` clause precedes the `where` clause.
    let result = parse_source(include_str!("fixtures/parser/uses_clause_with_where.ori"));

    assert!(!result.has_errors(), "Expected no parse errors");
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    assert_eq!(func.capabilities.len(), 1);
    assert_eq!(func.where_clauses.len(), 1);
}

#[test]
fn test_no_uses_clause() {
    // Pure function - no uses clause
    let result = parse_source(include_str!("fixtures/parser/no_uses_clause.ori"));

    assert!(!result.has_errors(), "Expected no parse errors");
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    assert!(func.capabilities.is_empty());
}

#[test]
fn test_with_capability_expression() {
    // with Capability = Provider in body
    let result = parse_source(include_str!(
        "fixtures/parser/with_capability_expression.ori"
    ));

    assert!(
        !result.has_errors(),
        "Expected no parse errors: {:?}",
        result.errors
    );
    assert_eq!(result.module.functions.len(), 1);

    // Find the WithCapability expression in the body
    let func = &result.module.functions[0];
    let body_expr = result.arena.get_expr(func.body);
    assert!(
        matches!(body_expr.kind, ExprKind::WithCapability { .. }),
        "Expected WithCapability, got {:?}",
        body_expr.kind
    );
}

#[test]
fn test_with_capability_with_struct_provider() {
    // with Capability = StructLiteral { field: value } in body
    let result = parse_source(include_str!(
        "fixtures/parser/with_capability_with_struct_provider.ori"
    ));

    assert!(
        !result.has_errors(),
        "Expected no parse errors: {:?}",
        result.errors
    );
}

#[test]
fn test_with_capability_nested() {
    // Nested capability provisions
    let result = parse_source(include_str!("fixtures/parser/with_capability_nested.ori"));

    assert!(
        !result.has_errors(),
        "Expected no parse errors: {:?}",
        result.errors
    );
}

#[test]
fn test_no_async_type_modifier() {
    // Ori does not support `async` as a type modifier.
    // Instead, use `uses Async` capability.
    // `async` is not a keyword — it parses as an identifier, while `async int`
    // is invalid because type position contains two identifiers.
    let result = parse_source(include_str!("fixtures/parser/no_async_type_modifier.ori"));

    // Should have parse error - two identifiers is not a valid type
    assert!(
        result.has_errors(),
        "async type modifier should not be supported"
    );
}

#[test]
fn test_async_as_identifier() {
    // `async` is not a reserved keyword (not in spec reserved list).
    // It can be used as a regular identifier.
    let result = parse_source(include_str!("fixtures/parser/async_as_identifier.ori"));

    assert!(
        !result.has_errors(),
        "async should be a valid identifier: {:?}",
        result.errors
    );
}

#[test]
fn test_uses_async_capability_parses() {
    // Async behavior is declared through the `Async` capability.
    let result = parse_source(include_str!(
        "fixtures/parser/uses_async_capability_parses.ori"
    ));

    assert!(
        !result.has_errors(),
        "uses Async should parse correctly: {:?}",
        result.errors
    );

    // Verify the function has the Async capability
    let func = &result.module.functions[0];
    assert_eq!(func.capabilities.len(), 1);
}

#[test]
fn test_shift_right_operator() {
    // >> is detected as two adjacent > tokens in expression context
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/shift_right_operator.ori"
    )));

    assert!(
        !result.has_errors(),
        "Expected no parse errors: {:?}",
        result.errors
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Binary {
        op: BinaryOp::Shr, ..
    } = &body.kind
    {
        // Success
    } else {
        panic!(
            "Expected right shift (>>) binary expression, got {:?}",
            body.kind
        );
    }
}

#[test]
fn test_greater_equal_operator() {
    // >= is detected as adjacent > and = tokens in expression context
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/greater_equal_operator.ori"
    )));

    assert!(
        !result.has_errors(),
        "Expected no parse errors: {:?}",
        result.errors
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Binary {
        op: BinaryOp::GtEq, ..
    } = &body.kind
    {
        // Success
    } else {
        panic!(
            "Expected greater-equal (>=) binary expression, got {:?}",
            body.kind
        );
    }
}

#[test]
fn test_shift_left_operator() {
    // `<<` is a single token from the lexer.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/shift_left_operator.ori"
    )));

    assert!(
        !result.has_errors(),
        "Expected no parse errors: {:?}",
        result.errors
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Binary {
        op: BinaryOp::Shl, ..
    } = &body.kind
    {
        // Success
    } else {
        panic!(
            "Expected left shift (<<) binary expression, got {:?}",
            body.kind
        );
    }
}

#[test]
fn test_greater_than_operator() {
    // A single `>` is the greater-than operator.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/greater_than_operator.ori"
    )));

    assert!(
        !result.has_errors(),
        "Expected no parse errors: {:?}",
        result.errors
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Binary {
        op: BinaryOp::Gt, ..
    } = &body.kind
    {
        // Success
    } else {
        panic!(
            "Expected greater-than (>) binary expression, got {:?}",
            body.kind
        );
    }
}

#[test]
fn test_shift_right_with_space() {
    // > > with space should NOT be treated as >>
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/shift_right_with_space.ori"
    )));

    // This should have errors because `> > 2` is invalid syntax
    // (comparison followed by another >)
    assert!(
        result.has_errors(),
        "Expected parse errors for `> > 2` with space"
    );
}

#[test]
fn test_greater_equal_with_space() {
    // > = with space should NOT be treated as >=
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/greater_equal_with_space.ori"
    )));

    // This should have errors because `> = 3` is invalid syntax
    assert!(
        result.has_errors(),
        "Expected parse errors for `> = 3` with space"
    );
}

#[test]
fn test_nested_generic_and_shift() {
    // Test that nested generics work in a type annotation and >> works in expression
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/nested_generic_and_shift.ori"
    )));

    assert!(
        !result.has_errors(),
        "Expected no parse errors for nested generics and >> operator: {:?}",
        result.errors
    );
}

// Context Management Tests

#[test]
fn test_struct_literal_in_expression() {
    // Struct literals work normally in expressions
    let result = parse_source(include_str!(
        "fixtures/parser/struct_literal_in_expression.ori"
    ));

    assert!(
        !result.has_errors(),
        "Struct literal should parse in normal expression: {:?}",
        result.errors
    );
}

#[test]
fn test_struct_literal_in_if_then_body() {
    // Struct literals work in the then body of an if expression
    let result = parse_source(include_str!(
        "fixtures/parser/struct_literal_in_if_then_body.ori"
    ));

    assert!(
        !result.has_errors(),
        "Struct literal should parse in if body: {:?}",
        result.errors
    );
}

#[test]
fn test_if_condition_disallows_struct_literal() {
    // Struct literals are NOT allowed directly in if conditions
    // This is a common pattern in many languages to prevent ambiguity
    // Note: In Ori with `then` keyword, this is mostly for consistency,
    // but it helps prevent confusing code like `if Point { ... }.valid then`
    let result = parse_source(include_str!(
        "fixtures/parser/if_condition_disallows_struct_literal.ori"
    ));

    // This should fail because struct literal is not allowed in if condition
    assert!(
        result.has_errors(),
        "Struct literal should NOT be allowed in if condition"
    );
}

#[test]
fn test_context_methods() {
    // Exercise the context API to ensure it compiles and works
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(
        fixture_without_trailing_newline(include_str!("fixtures/parser/context_methods.ori")),
        &interner,
    );
    let mut parser = Parser::new(&tokens, &interner);

    // Test context() getter
    let ctx = parser.context();
    assert_eq!(ctx, ParseContext::NONE);

    // Test has_context()
    assert!(!parser.has_context(ParseContext::IN_LOOP));

    // Test with_context()
    let result = parser.with_context(ParseContext::IN_LOOP, |p| {
        assert!(p.has_context(ParseContext::IN_LOOP));
        42
    });
    assert_eq!(result, 42);
    assert!(!parser.has_context(ParseContext::IN_LOOP)); // restored

    // Test without_context() - first add a context, then remove it
    parser.context = ParseContext::IN_LOOP;
    let result = parser.without_context(ParseContext::IN_LOOP, |p| {
        assert!(!p.has_context(ParseContext::IN_LOOP));
        43
    });
    assert_eq!(result, 43);
    assert!(parser.has_context(ParseContext::IN_LOOP)); // restored
}

// Metadata Tests

mod metadata_tests {
    use super::fixture_without_trailing_newline;
    use crate::parse_with_metadata;
    use ori_ir::{ModuleExtra, StringInterner};

    fn parse_with_comments(source: &str) -> crate::ParseOutput {
        let interner = StringInterner::new();
        let lex_output = ori_lexer::lex_with_comments(source, &interner);
        let (tokens, metadata) = lex_output.into_parts();
        parse_with_metadata(&tokens, metadata, &interner)
    }

    #[test]
    fn test_metadata_preserved_in_parse_output() {
        let source = include_str!("fixtures/parser/metadata_preserved_in_parse_output.ori");
        let output = parse_with_comments(source);

        // Comments should be preserved
        assert_eq!(output.metadata.comments.len(), 2);

        // Blank line should be detected
        assert!(!output.metadata.blank_lines.is_empty());

        // Newlines should be tracked
        assert!(!output.metadata.newlines.is_empty());
    }

    #[test]
    fn test_metadata_doc_comments_for_function() {
        let source = include_str!("fixtures/parser/metadata_doc_comments_for_function.ori");
        let output = parse_with_comments(source);

        // Get the function start position
        assert_eq!(
            output.module.functions.len(),
            1,
            "Should parse one function"
        );
        let func = &output.module.functions[0];
        let fn_start = func.span.start;

        // Doc comments should be available (though blocked by blank line in this case)
        let docs = output.metadata.doc_comments_for(fn_start);
        // Blank line blocks the doc comments from attaching
        assert!(
            docs.is_empty(),
            "Blank line should block doc comment attachment"
        );
    }

    #[test]
    fn test_metadata_blank_line_blocks_doc_comment() {
        // Use # prefix for doc comments
        let source = include_str!("fixtures/parser/metadata_blank_line_blocks_doc_comment.ori");
        let output = parse_with_comments(source);

        // Get the function start position
        assert_eq!(
            output.module.functions.len(),
            1,
            "Should parse one function"
        );
        let func = &output.module.functions[0];
        let fn_start = func.span.start;

        // Only the last doc comment should attach (blank line blocks the first two)
        let docs = output.metadata.doc_comments_for(fn_start);
        assert_eq!(
            docs.len(),
            1,
            "Only doc comment after blank line should attach"
        );
        // Verify it's the right one
        assert!(docs[0].kind.is_doc());
    }

    #[test]
    fn test_metadata_no_comments() {
        let output = parse_with_comments(fixture_without_trailing_newline(include_str!(
            "fixtures/parser/parse_literal.ori"
        )));

        assert!(output.metadata.comments.is_empty());
    }

    #[test]
    fn test_metadata_regular_vs_doc_comments() {
        let source = include_str!("fixtures/parser/metadata_regular_vs_doc_comments.ori");
        let output = parse_with_comments(source);

        assert_eq!(output.metadata.comments.len(), 2);

        // Check comment kinds
        let comments: Vec<_> = output.metadata.comments.iter().collect();
        assert!(!comments[0].kind.is_doc()); // Regular
        assert!(comments[1].kind.is_doc()); // DocDescription
    }

    #[test]
    fn test_metadata_multiple_functions_with_comments() {
        let source = include_str!("fixtures/parser/metadata_multiple_functions_with_comments.ori");
        let output = parse_with_comments(source);

        assert_eq!(
            output.module.functions.len(),
            2,
            "Should parse two functions"
        );
        assert_eq!(output.metadata.comments.len(), 2);

        // Each function should have its own doc comment
        let foo = &output.module.functions[0];
        let bar = &output.module.functions[1];

        let foo_docs = output.metadata.doc_comments_for(foo.span.start);
        let bar_docs = output.metadata.doc_comments_for(bar.span.start);

        assert_eq!(foo_docs.len(), 1, "foo should have one doc comment");
        assert_eq!(bar_docs.len(), 1, "bar should have one doc comment");
    }

    #[test]
    fn test_metadata_multiline() {
        let source = include_str!("fixtures/parser/metadata_multiline.ori");
        let output = parse_with_comments(source);

        assert_eq!(
            output.module.functions.len(),
            1,
            "Should parse one function"
        );
        // Function body spans multiple lines
        let func = &output.module.functions[0];
        let is_multi = output.metadata.is_multiline(func.span);
        assert!(is_multi);
    }

    #[test]
    fn test_metadata_line_number() {
        let source = include_str!("fixtures/parser/metadata_line_number.ori");
        let output = parse_with_comments(source);

        assert_eq!(
            output.module.functions.len(),
            1,
            "Should parse one function"
        );
        // Function starts on line 2
        let func = &output.module.functions[0];
        let line = output.metadata.line_number(func.span.start);
        assert_eq!(line, 2);
    }

    #[test]
    fn test_parse_with_empty_metadata() {
        // Test that parse() produces empty metadata by default
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(
            fixture_without_trailing_newline(include_str!("fixtures/parser/parse_literal.ori")),
            &interner,
        );
        let output = crate::parse(&tokens, &interner);

        // Default parse produces empty metadata
        assert!(output.metadata.comments.is_empty());
        assert!(output.metadata.blank_lines.is_empty());
        assert!(output.metadata.newlines.is_empty());
    }

    #[test]
    fn test_parse_with_explicit_metadata() {
        // Test that parse_with_metadata correctly transfers metadata
        let interner = StringInterner::new();
        let lex_output = ori_lexer::lex_with_comments(
            fixture_without_trailing_newline(include_str!(
                "fixtures/parser/parse_with_explicit_metadata.ori"
            )),
            &interner,
        );

        let expected_comment_count = lex_output.comments.len();
        let expected_newline_count = lex_output.newlines.len();

        let metadata = ModuleExtra {
            comments: lex_output.comments.clone(),
            blank_lines: lex_output.blank_lines.clone(),
            newlines: lex_output.newlines.clone(),
            trailing_commas: Vec::new(),
        };

        let output = parse_with_metadata(&lex_output.tokens, metadata, &interner);

        assert_eq!(output.metadata.comments.len(), expected_comment_count);
        assert_eq!(output.metadata.newlines.len(), expected_newline_count);
    }

    // Warning Tests

    #[test]
    fn test_no_warnings_when_doc_comments_attached() {
        let source = include_str!("fixtures/parser/no_warnings_when_doc_comments_attached.ori");
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert!(
            output.warnings.is_empty(),
            "Should have no warnings when doc comment is attached"
        );
    }

    #[test]
    fn test_warning_for_detached_doc_comment_blank_line() {
        let source =
            include_str!("fixtures/parser/warning_for_detached_doc_comment_blank_line.ori");
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert_eq!(output.warnings.len(), 1, "Should have one warning");
        match &output.warnings[0] {
            crate::ParseWarning::DetachedDocComment { reason, .. } => {
                assert_eq!(*reason, crate::DetachmentReason::BlankLine);
            }
            other @ crate::ParseWarning::UnknownCallingConvention { .. } => {
                panic!("expected DetachedDocComment, got {other:?}")
            }
        }
    }

    #[test]
    fn test_warning_for_doc_comment_at_end_of_file() {
        let source = include_str!("fixtures/parser/warning_for_doc_comment_at_end_of_file.ori");
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert_eq!(
            output.warnings.len(),
            1,
            "Should have one warning for orphan at end"
        );
        match &output.warnings[0] {
            crate::ParseWarning::DetachedDocComment { reason, .. } => {
                assert_eq!(*reason, crate::DetachmentReason::NoFollowingDeclaration);
            }
            other @ crate::ParseWarning::UnknownCallingConvention { .. } => {
                panic!("expected DetachedDocComment, got {other:?}")
            }
        }
    }

    #[test]
    fn test_no_warning_for_regular_comments() {
        let source = include_str!("fixtures/parser/no_warning_for_regular_comments.ori");
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        // Regular comments don't generate warnings
        assert!(
            output.warnings.is_empty(),
            "Regular comments should not generate warnings"
        );
    }

    #[test]
    fn test_warning_includes_helpful_hint() {
        let source = include_str!("fixtures/parser/warning_includes_helpful_hint.ori");
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert!(!output.warnings.is_empty());
        let warning = &output.warnings[0];
        let message = warning.message();
        assert!(
            message.contains("blank line"),
            "Warning should mention blank line"
        );
    }

    #[test]
    fn test_warning_to_diagnostic() {
        let source = include_str!("fixtures/parser/warning_includes_helpful_hint.ori");
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert!(!output.warnings.is_empty());
        let diagnostic = output.warnings[0].to_diagnostic();

        assert!(diagnostic.code.is_warning());
    }
}

// `dispatch_declaration()` rejects every token that cannot begin a module-level
// declaration, including control-flow keywords and bare literals.

/// All valid declaration forms must parse without errors.
#[test]
fn test_valid_declarations_at_module_level() {
    let valid_sources = &[
        // Functions
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_01.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_02.ori"
        )),
        // Types
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_03.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_04.ori"
        )),
        // Traits
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_05.ori"
        )),
        // Impl blocks
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_06.ori"
        )),
        // Constants
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_07.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_08.ori"
        )),
        // Bare constants
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_09.ori"
        )),
        // Imports
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_10.ori"
        )),
        // Extend blocks
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_11.ori"
        )),
        // Visibility modifiers
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_12.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_13.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_14.ori"
        )),
        // Multiple declarations
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_15.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_16.ori"
        )),
        // Empty file
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/valid_declarations_at_module_level_17.ori"
        )),
        // Only whitespace/newlines
        include_str!("fixtures/parser/valid_declarations_at_module_level_18.ori"),
    ];

    for source in valid_sources {
        let result = parse_source(source);
        assert!(
            !result.has_errors(),
            "Expected no errors for valid source:\n  {source}\nErrors: {:?}",
            result.errors
        );
    }
}

/// Invalid tokens at module top level must produce errors, not pass silently.
#[test]
fn test_invalid_tokens_at_module_level_produce_errors() {
    let invalid_sources = &[
        // Expression keywords — not valid at top level
        "break",
        "continue",
        "if true = 1",
        "for x in [1, 2, 3] = x",
        "match 1 { 1 => 2 }",
        "loop = 1",
        // Bare literals
        "42",
        "\"hello\"",
        "true",
        "3.14",
        // Bare identifiers
        "hello",
        "x",
        "foo_bar",
        // Operators
        "+",
        "=",
        "==",
        // Punctuation that doesn't start a declaration
        "(",
        ")",
        "{",
        "}",
        "[",
        "]",
    ];

    for source in invalid_sources {
        let result = parse_source(source);
        assert!(
            result.has_errors(),
            "Expected errors for invalid top-level source:\n  {source}\nGot no errors"
        );
    }
}

/// `return` at module level must produce a specific `UnsupportedKeyword` error.
#[test]
fn test_return_at_module_level_produces_specific_error() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/return_at_module_level_produces_specific_error.ori"
    )));
    assert!(result.has_errors());
    let err = &result.errors[0];
    assert_eq!(err.code, ori_diagnostic::ErrorCode::E1015);
    assert!(
        err.message.contains("return"),
        "Error message should mention `return`: {}",
        err.message
    );
}

/// `return` inside a function body also produces a specific error (via `parse_control_flow_primary`).
#[test]
fn test_return_in_function_body_produces_error() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/return_in_function_body_produces_error.ori"
    )));
    assert!(result.has_errors());
    let return_err = result
        .errors
        .iter()
        .find(|e| e.code == ori_diagnostic::ErrorCode::E1015);
    assert!(
        return_err.is_some(),
        "Expected E1015 for `return` in function body, errors: {:?}",
        result.errors
    );
}

/// `let x = 42` (without `$`) at module level produces a specific error about immutability.
#[test]
fn test_mutable_let_at_module_level_rejected() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/mutable_let_at_module_level_rejected.ori"
    )));
    assert!(result.has_errors());
    let err = &result.errors[0];
    assert!(
        err.message.contains("immutable"),
        "Error should mention immutability: {}",
        err.message
    );
}

/// `let $x = 42` at module level parses as a constant.
#[test]
fn test_const_let_at_module_level_accepted() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/const_let_at_module_level_accepted.ori"
    )));
    assert!(!result.has_errors());
    assert_eq!(result.module.consts.len(), 1);
}

/// `pub let $x = 42` at module level parses as a public constant.
#[test]
fn test_pub_const_let_at_module_level_accepted() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/pub_const_let_at_module_level_accepted.ori"
    )));
    assert!(!result.has_errors());
    assert_eq!(result.module.consts.len(), 1);
}

/// Foreign keywords from other languages produce suggestions at module level.
#[test]
fn test_foreign_keywords_at_module_level() {
    let foreign_keywords = &["fn", "func", "function", "class", "struct", "interface"];

    for kw in foreign_keywords {
        let result = parse_source(kw);
        assert!(
            result.has_errors(),
            "Expected error for foreign keyword `{kw}` at module level"
        );
        // Foreign keywords should have help text
        let err = &result.errors[0];
        assert!(
            !err.help.is_empty(),
            "Foreign keyword `{kw}` should have help suggestion, got: {err:?}",
        );
    }
}

/// Multiple invalid tokens in a row each produce their own error.
#[test]
fn test_multiple_invalid_tokens_each_produce_error() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/multiple_invalid_tokens_each_produce_error.ori"
    )));
    assert!(result.has_errors());
    assert!(
        result.errors.len() >= 3,
        "Expected at least 3 errors for 3 invalid tokens, got {}: {:?}",
        result.errors.len(),
        result.errors
    );
}

/// Valid declarations mixed with invalid tokens preserve the valid items.
#[test]
fn test_mixed_valid_and_invalid_at_module_level() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/mixed_valid_and_invalid_at_module_level.ori"
    )));
    assert!(result.has_errors());
    // The recovered module contains both valid functions.
    assert_eq!(
        result.module.functions.len(),
        2,
        "Both valid functions should parse despite intervening invalid token"
    );
}

/// Semicolons after top-level items are accepted.
#[test]
fn test_semicolons_after_top_level_items_accepted() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/semicolons_after_top_level_items_accepted.ori"
    )));
    assert!(
        !result.has_errors(),
        "Semicolons after function definitions should be accepted: {result:?}"
    );
}

// Labels: break:label, continue:label, for:label, loop:label

/// Parse source and return both output and interner (needed for label name lookups).
fn parse_source_with_interner(source: &str) -> (ParseOutput, StringInterner) {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let output = parse(&tokens, &interner);
    (output, interner)
}

#[test]
fn repeated_test_display_names_receive_stable_distinct_identities() {
    let (result, interner) =
        parse_source_with_interner(fixture_without_trailing_newline(include_str!(
            "fixtures/parser/repeated_test_display_names_receive_stable_distinct_identities.ori"
        )));
    assert!(
        result.errors.is_empty(),
        "parse errors: {:?}",
        result.errors
    );
    assert_eq!(result.module.tests.len(), 2);
    let first = &result.module.tests[0];
    let second = &result.module.tests[1];
    assert_eq!(first.id, ori_ir::TestId::new(0));
    assert_eq!(second.id, ori_ir::TestId::new(1));
    assert_eq!(first.display_name, second.display_name);
    assert_ne!(first.name, second.name);
    assert_eq!(interner.lookup(first.name), "test$test$0");
    assert_eq!(interner.lookup(second.name), "test$test$1");
}

#[test]
fn test_labeled_break() {
    let (result, interner) = parse_source_with_interner(fixture_without_trailing_newline(
        include_str!("fixtures/parser/labeled_break.ori"),
    ));
    assert!(
        !result.has_errors(),
        "labeled break should parse: {result:?}"
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::Loop { label, body } = &body.kind {
        assert_eq!(
            interner.lookup(*label),
            "outer",
            "loop label should be 'outer'"
        );
        // loop body is a block { break:outer 42 }
        let loop_body = result.arena.get_expr(*body);
        let break_id = if let ExprKind::Block { result: res, .. } = &loop_body.kind {
            *res
        } else if let ExprKind::Break { .. } = &loop_body.kind {
            *body
        } else {
            panic!("expected Block or Break, got {loop_body:?}");
        };
        let break_expr = result.arena.get_expr(break_id);
        if let ExprKind::Break { label, value } = &break_expr.kind {
            assert_eq!(
                interner.lookup(*label),
                "outer",
                "break label should be 'outer'"
            );
            assert!(value.is_present(), "break should have a value");
        } else {
            panic!("expected Break, got {break_expr:?}");
        }
    } else {
        panic!("expected Loop, got {body:?}");
    }
}

#[test]
fn test_labeled_continue() {
    let (result, interner) = parse_source_with_interner(fixture_without_trailing_newline(
        include_str!("fixtures/parser/labeled_continue.ori"),
    ));
    assert!(
        !result.has_errors(),
        "labeled continue should parse: {result:?}"
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::For { label, body, .. } = &body.kind {
        assert_eq!(
            interner.lookup(*label),
            "outer",
            "for label should be 'outer'"
        );
        let cont = result.arena.get_expr(*body);
        if let ExprKind::Continue { label, .. } = &cont.kind {
            assert_eq!(
                interner.lookup(*label),
                "outer",
                "continue label should be 'outer'"
            );
        } else {
            panic!("expected Continue, got {cont:?}");
        }
    } else {
        panic!("expected For, got {body:?}");
    }
}

#[test]
fn test_unlabeled_break_in_loop_parses_with_value() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/unlabeled_break_in_loop_parses_with_value.ori"
    )));
    assert!(!result.has_errors(), "unlabeled break should still parse");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::Loop { label, body } = &body.kind {
        assert_eq!(*label, ori_ir::Name::EMPTY, "loop should have no label");
        // loop body is a block { break 42 }
        let loop_body = result.arena.get_expr(*body);
        let break_id = if let ExprKind::Block { result: res, .. } = &loop_body.kind {
            *res
        } else if let ExprKind::Break { .. } = &loop_body.kind {
            *body
        } else {
            panic!("expected Block or Break, got {loop_body:?}");
        };
        let break_expr = result.arena.get_expr(break_id);
        if let ExprKind::Break { label, value } = &break_expr.kind {
            assert_eq!(*label, ori_ir::Name::EMPTY, "break should have no label");
            assert!(value.is_present(), "break should have a value");
        } else {
            panic!("expected Break, got {break_expr:?}");
        }
    } else {
        panic!("expected Loop, got {body:?}");
    }
}

#[test]
fn test_unlabeled_continue_in_for_parses_without_value() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/unlabeled_continue_in_for_parses_without_value.ori"
    )));
    assert!(
        !result.has_errors(),
        "unlabeled continue should still parse"
    );
}

#[test]
fn test_labeled_for_loop() {
    let (result, interner) = parse_source_with_interner(fixture_without_trailing_newline(
        include_str!("fixtures/parser/labeled_for_loop.ori"),
    ));
    assert!(!result.has_errors(), "labeled for should parse: {result:?}");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::For { label, .. } = &body.kind {
        assert_eq!(
            interner.lookup(*label),
            "items",
            "for label should be 'items'"
        );
    } else {
        panic!("expected For, got {body:?}");
    }
}

#[test]
fn test_labeled_loop() {
    let (result, interner) = parse_source_with_interner(fixture_without_trailing_newline(
        include_str!("fixtures/parser/labeled_loop.ori"),
    ));
    assert!(
        !result.has_errors(),
        "labeled loop should parse: {result:?}"
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::Loop { label, .. } = &body.kind {
        assert_eq!(
            interner.lookup(*label),
            "main",
            "loop label should be 'main'"
        );
    } else {
        panic!("expected Loop, got {body:?}");
    }
}

#[test]
fn test_while_loop_parses_with_cond_and_body() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/while_loop_parses_with_cond_and_body.ori"
    )));
    assert!(!result.has_errors(), "while should parse: {result:?}");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::While { label, cond, body } = &body.kind {
        assert_eq!(
            *label,
            ori_ir::Name::EMPTY,
            "unlabeled while has empty label"
        );
        let cond_expr = result.arena.get_expr(*cond);
        assert!(
            matches!(cond_expr.kind, ExprKind::Bool(true)),
            "while cond should be `true`, got {cond_expr:?}"
        );
        let body_expr = result.arena.get_expr(*body);
        assert!(
            matches!(body_expr.kind, ExprKind::Break { .. }),
            "while body should be `break`, got {body_expr:?}"
        );
    } else {
        panic!("expected While, got {body:?}");
    }
}

#[test]
fn test_labeled_while_loop_carries_label() {
    let (result, interner) = parse_source_with_interner(fixture_without_trailing_newline(
        include_str!("fixtures/parser/labeled_while_loop_carries_label.ori"),
    ));
    assert!(
        !result.has_errors(),
        "labeled while should parse: {result:?}"
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::While { label, .. } = &body.kind {
        assert_eq!(
            interner.lookup(*label),
            "outer",
            "while label should be 'outer'"
        );
    } else {
        panic!("expected While, got {body:?}");
    }
}

#[test]
fn test_while_loop_with_do_block_body_parses() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/while_loop_with_do_block_body_parses.ori"
    )));
    assert!(
        !result.has_errors(),
        "while with do-block body should parse: {result:?}"
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::While { cond, body, .. } = &body.kind {
        let cond_expr = result.arena.get_expr(*cond);
        assert!(
            matches!(cond_expr.kind, ExprKind::Bool(true)),
            "while cond should be `true`, got {cond_expr:?}"
        );
        let body_expr = result.arena.get_expr(*body);
        assert!(
            matches!(body_expr.kind, ExprKind::Block { .. }),
            "while body should be a Block, got {body_expr:?}"
        );
    } else {
        panic!("expected While, got {body:?}");
    }
}

#[test]
fn test_while_loop_with_compound_and_condition_parses() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/while_loop_with_compound_and_condition_parses.ori"
    )));
    assert!(
        !result.has_errors(),
        "while with `&&` compound condition should parse: {result:?}"
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::While { cond, .. } = &body.kind {
        let cond_expr = result.arena.get_expr(*cond);
        assert!(
            matches!(
                cond_expr.kind,
                ExprKind::Binary {
                    op: BinaryOp::And,
                    ..
                }
            ),
            "while cond should be `&&` Binary, got {cond_expr:?}"
        );
    } else {
        panic!("expected While, got {body:?}");
    }
}

#[test]
fn test_nested_while_loops_parse() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/nested_while_loops_parse.ori"
    )));
    assert!(
        !result.has_errors(),
        "nested while should parse: {result:?}"
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::While { body, .. } = &body.kind {
        let inner = result.arena.get_expr(*body);
        assert!(
            matches!(inner.kind, ExprKind::While { .. }),
            "outer while body should be an inner While, got {inner:?}"
        );
    } else {
        panic!("expected outer While, got {body:?}");
    }
}

#[test]
fn test_while_loop_rejects_yield_form() {
    // The `while` grammar accepts only `do`, so `while c yield e` must not parse.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/while_loop_rejects_yield_form.ori"
    )));
    assert!(
        result.has_errors(),
        "`while...yield` must NOT parse (no yield form per proposal): {result:?}"
    );
}

#[test]
fn test_labeled_continue_with_value() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/labeled_continue_with_value.ori"
    )));
    assert!(
        !result.has_errors(),
        "labeled continue with value should parse: {result:?}"
    );
}

#[test]
fn test_nested_labels() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/nested_labels.ori"
    )));
    assert!(
        !result.has_errors(),
        "nested labels should parse: {result:?}"
    );
}

#[test]
fn test_label_with_space_is_not_label() {
    // `break :outer` with a space should NOT parse as a label.
    // The `:outer` is treated as a value expression which is invalid.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/label_with_space_is_not_label.ori"
    )));
    assert!(
        result.has_errors(),
        "space before colon should prevent label parsing"
    );
}

#[test]
fn test_tuple_field_access() {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(
        fixture_without_trailing_newline(include_str!("fixtures/parser/tuple_field_access.ori")),
        &interner,
    );
    let result = parse(&tokens, &interner);

    assert!(
        !result.has_errors(),
        "tuple field access should parse: {:?}",
        result.errors
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    if let ExprKind::Field { field, .. } = &body.kind {
        assert_eq!(interner.lookup(*field), "0");
    } else {
        panic!("expected ExprKind::Field, got {:?}", body.kind);
    }
}

#[test]
fn test_chained_tuple_field_access_with_parens() {
    // Spec: Clause 14.1.1 requires parentheses because `0.1` is a float literal.
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/chained_tuple_field_access_with_parens.ori"
        )),
        &interner,
    );
    let result = parse(&tokens, &interner);

    assert!(
        !result.has_errors(),
        "parenthesized chained tuple field access should parse: {:?}",
        result.errors
    );

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    // (t.0).1 parses as Field(Field(t, "0"), "1") — parens are transparent
    if let ExprKind::Field { receiver, field } = &body.kind {
        assert_eq!(interner.lookup(*field), "1");
        let inner = result.arena.get_expr(*receiver);
        if let ExprKind::Field {
            field: inner_field, ..
        } = &inner.kind
        {
            assert_eq!(interner.lookup(*inner_field), "0");
        } else {
            panic!("expected inner ExprKind::Field, got {:?}", inner.kind);
        }
    } else {
        panic!("expected ExprKind::Field, got {:?}", body.kind);
    }
}

#[test]
fn test_tuple_member_float_range_boundaries_parse() {
    let cases = [
        (
            "parenthesized tuple selection",
            fixture_without_trailing_newline(include_str!(
                "fixtures/parser/chained_tuple_field_access_with_parens.ori"
            )),
        ),
        (
            "tuple selection then named member",
            fixture_without_trailing_newline(include_str!(
                "fixtures/parser/tuple_member_float_range_boundaries_parse_01.ori"
            )),
        ),
        (
            "named member then tuple selection",
            fixture_without_trailing_newline(include_str!(
                "fixtures/parser/tuple_member_float_range_boundaries_parse_02.ori"
            )),
        ),
        (
            "float literal",
            fixture_without_trailing_newline(include_str!(
                "fixtures/parser/tuple_member_float_range_boundaries_parse_03.ori"
            )),
        ),
        (
            "tuple selection then exclusive range",
            fixture_without_trailing_newline(include_str!(
                "fixtures/parser/tuple_member_float_range_boundaries_parse_04.ori"
            )),
        ),
        (
            "tuple selection then inclusive range",
            fixture_without_trailing_newline(include_str!(
                "fixtures/parser/tuple_member_float_range_boundaries_parse_05.ori"
            )),
        ),
    ];
    let mut visited = 0;
    for (label, source) in cases {
        let result = parse_source(source);
        assert!(
            !result.has_errors(),
            "{label} should parse without ambiguity: {:?}",
            result.errors
        );
        visited += 1;
    }
    assert_eq!(visited, cases.len(), "every matrix cell must run");
}

#[test]
fn test_bare_chained_tuple_field_reports_parenthesis_fix() {
    // Spec: Clause 14.1.1 requires `(t.0).1`; bare `t.0.1` contains float `0.1`.
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(
        fixture_without_trailing_newline(include_str!(
            "fixtures/parser/bare_chained_tuple_field_reports_parenthesis_fix.ori"
        )),
        &interner,
    );
    let result = parse(&tokens, &interner);
    let error = result
        .errors
        .iter()
        .find(|error| error.code() == ori_diagnostic::ErrorCode::E1004)
        .unwrap_or_else(|| panic!("expected E1004, got errors: {:?}", result.errors));
    assert!(
        error.message().contains("float literal"),
        "diagnostic must name the lexical cause: {error:?}"
    );
    let help = error.help().join("\n");
    assert!(
        help.contains("parenthesize") && help.contains("(value.0).1"),
        "diagnostic must show the parenthesized fix: {error:?}"
    );
    assert!(
        help.contains("Spec: Clause 14.1.1"),
        "diagnostic must cite the governing rule: {error:?}"
    );
}

// Dollar-prefixed bindings are immutable in every let parsing path.
// `BindingPattern::Name::mutable` is the evaluator's authority, so each path
// preserves the dollar token through binding-pattern parsing.

#[test]
fn test_let_expr_dollar_immutable_on_pattern() {
    // Expression-form let: `let $x = 42` should produce Immutable on the pattern.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/let_expr_dollar_immutable_on_pattern.ori"
    )));
    assert!(!result.has_errors(), "Expected no parse errors");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    let ExprKind::Let {
        pattern: pat_id,
        mutable,
        ..
    } = &body.kind
    else {
        panic!("Expected ExprKind::Let, got {:?}", body.kind);
    };

    // Statement and pattern mutability must agree.
    assert_eq!(
        *mutable,
        Mutability::Immutable,
        "ExprKind::Let.mutable should be Immutable for `let $x`"
    );

    // Pattern mutability is the evaluator's ownership authority.
    let BindingPattern::Name {
        mutable: pat_mut, ..
    } = result.arena.get_binding_pattern(*pat_id)
    else {
        panic!("Expected BindingPattern::Name");
    };
    assert_eq!(
        *pat_mut,
        Mutability::Immutable,
        "BindingPattern::Name.mutable should be Immutable for `let $x` (evaluator authority)"
    );
}

#[test]
fn test_block_let_dollar_immutable_on_pattern() {
    // Block-form let preserves the dollar token through pattern parsing.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/block_let_dollar_immutable_on_pattern.ori"
    )));
    assert!(!result.has_errors(), "Expected no parse errors");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    let ExprKind::Block { stmts, .. } = &body.kind else {
        panic!("Expected Block, got {:?}", body.kind);
    };

    let stmt_list = result.arena.get_stmt_range(*stmts);
    assert_eq!(stmt_list.len(), 1);

    let StmtKind::Let {
        pattern: pat_id,
        mutable,
        ..
    } = &stmt_list[0].kind
    else {
        panic!("Expected StmtKind::Let");
    };

    assert_eq!(*mutable, Mutability::Immutable);

    let BindingPattern::Name {
        mutable: pat_mut, ..
    } = result.arena.get_binding_pattern(*pat_id)
    else {
        panic!("Expected BindingPattern::Name");
    };
    assert_eq!(
        *pat_mut,
        Mutability::Immutable,
        "block-form let $x: BindingPattern.mutable should be Immutable"
    );
}

#[test]
fn test_try_let_dollar_immutable_on_pattern() {
    // Try-block let: `try { let $x = Ok(5); Ok(x) }` should produce Immutable on the pattern.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/try_let_dollar_immutable_on_pattern.ori"
    )));

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors(), "Expected no parse errors");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    let ExprKind::FunctionSeq(seq_id) = &body.kind else {
        panic!("Expected FunctionSeq, got {:?}", body.kind);
    };

    let FunctionSeq::Try { stmts, .. } = result.arena.get_function_seq(*seq_id) else {
        panic!("Expected FunctionSeq::Try");
    };

    let stmt_list = result.arena.get_stmt_range(*stmts);
    assert!(!stmt_list.is_empty(), "Expected at least one try binding");

    let StmtKind::Let {
        pattern: pat_id,
        mutable,
        ..
    } = &stmt_list[0].kind
    else {
        panic!("Expected StmtKind::Let in try, got {:?}", stmt_list[0].kind);
    };

    assert_eq!(
        *mutable,
        Mutability::Immutable,
        "StmtKind::Let.mutable should be Immutable for `let $x` in try"
    );

    let BindingPattern::Name {
        mutable: pat_mut, ..
    } = result.arena.get_binding_pattern(*pat_id)
    else {
        panic!("Expected BindingPattern::Name");
    };
    assert_eq!(
        *pat_mut,
        Mutability::Immutable,
        "try-block let $x: BindingPattern.mutable should be Immutable (evaluator authority)"
    );
}

#[test]
fn test_let_expr_default_mutable_on_pattern() {
    // Verify that `let x = 42` (no $) produces Mutable on both levels.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/let_expr_default_mutable_on_pattern.ori"
    )));
    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    let ExprKind::Let {
        pattern: pat_id,
        mutable,
        ..
    } = &body.kind
    else {
        panic!("Expected ExprKind::Let");
    };

    assert_eq!(*mutable, Mutability::Mutable);

    let BindingPattern::Name {
        mutable: pat_mut, ..
    } = result.arena.get_binding_pattern(*pat_id)
    else {
        panic!("Expected BindingPattern::Name");
    };
    assert_eq!(
        *pat_mut,
        Mutability::Mutable,
        "let x (no $): BindingPattern.mutable should be Mutable"
    );
}

// Lambda grammar — typed parameters with inferred OR explicit return.
//
// Spec: grammar.ebnf §lambda / §lambda_tail
//   lambda      = lambda_params "->" lambda_tail
//   lambda_tail = type "=" expression | expression
// Typed-param lambdas infer the return type unless a `type =` prefix is present
// (proposal: typed-lambda-inferred-return). Untyped params in a typed lambda
// are rejected with E1018.

#[test]
fn test_typed_lambda_inferred_return_accepted() {
    // `(x: int) -> x` has a typed parameter and an inferred return type.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/typed_lambda_inferred_return_accepted.ori"
    )));
    assert!(
        !result.has_errors(),
        "typed-param inferred-return lambda must parse cleanly; errors: {:?}",
        result.errors
    );
}

#[test]
fn test_typed_lambda_inferred_return_with_operator_body_accepted() {
    // (x: int) -> x * 2 : body is an expression, not a bare type.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/typed_lambda_inferred_return_with_operator_body_accepted.ori"
    )));
    assert!(
        !result.has_errors(),
        "inferred-return lambda with operator body must parse; errors: {:?}",
        result.errors
    );
}

#[test]
fn test_typed_lambda_multi_param_inferred_return_accepted() {
    // (a: int, b: int) -> a - b : multiple typed params, inferred return.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/typed_lambda_multi_param_inferred_return_accepted.ori"
    )));
    assert!(
        !result.has_errors(),
        "multi-typed-param inferred-return lambda must parse; errors: {:?}",
        result.errors
    );
}

#[test]
fn test_typed_lambda_bare_return_type_parses_as_inferred_body() {
    // (x: int) -> int : no `=`, so the speculative type parse restores and the
    // tail `int` parses as the inferred-return body (the type name used as a
    // value expression). Parsing accepts it; type checking rejects the type
    // name as a value with E2005 when the lambda is used.
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/typed_lambda_bare_return_type_parses_as_inferred_body.ori"
    )));
    assert!(
        !result.has_errors(),
        "bare `(x: int) -> int` must parse cleanly (body is the type name `int`); errors: {:?}",
        result.errors
    );
}

#[test]
fn test_typed_lambda_juxtaposed_tokens_after_return_type_rejected() {
    // (x: int) -> int x : `int` parses as the inferred-return body, then the
    // trailing `x` is unexpected — rejected as a statement-boundary error
    // (E1002 "expected `;` or `}` after let binding").
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/typed_lambda_juxtaposed_tokens_after_return_type_rejected.ori"
    )));
    assert!(
        result.has_errors(),
        "juxtaposed `int x` after `->` must still be rejected"
    );
    let has_e1002 = result
        .errors
        .iter()
        .any(|e| e.code == ori_diagnostic::ErrorCode::E1002);
    assert!(
        has_e1002,
        "expected E1002 (statement boundary after the parsed inferred-return body); errors: {:?}",
        result.errors
    );
}

#[test]
fn test_typed_lambda_mixed_typed_untyped_no_ret_ty_rejected() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/typed_lambda_mixed_typed_untyped_no_ret_ty_rejected.ori"
    )));
    assert!(result.has_errors(), "expected parse errors");
    let has_e1018 = result
        .errors
        .iter()
        .any(|e| e.code == ori_diagnostic::ErrorCode::E1018);
    assert!(
        has_e1018,
        "expected E1018 (untyped param in typed_lambda); errors: {:?}",
        result.errors
    );
}

#[test]
fn test_typed_lambda_mixed_typed_untyped_with_ret_ty_rejected() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/typed_lambda_mixed_typed_untyped_with_ret_ty_rejected.ori"
    )));
    assert!(result.has_errors(), "expected parse errors");
    let has_e1018 = result
        .errors
        .iter()
        .any(|e| e.code == ori_diagnostic::ErrorCode::E1018);
    assert!(
        has_e1018,
        "expected E1018 (untyped param in typed_lambda); errors: {:?}",
        result.errors
    );
}

#[test]
fn test_typed_lambda_valid_form_accepted() {
    let result = parse_source(fixture_without_trailing_newline(include_str!(
        "fixtures/parser/typed_lambda_valid_form_accepted.ori"
    )));
    assert!(
        !result.has_errors(),
        "valid typed_lambda must parse cleanly; errors: {:?}",
        result.errors
    );
}
