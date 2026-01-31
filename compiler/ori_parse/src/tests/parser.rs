//! Core parser tests.
//!
//! Tests for basic parsing functionality including literals, expressions,
//! operators, capabilities, and context management.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{parse, ParseContext, ParseOutput, Parser};
use ori_ir::{BinaryOp, BindingPattern, ExprKind, FunctionExpKind, FunctionSeq, StringInterner};

fn parse_source(source: &str) -> ParseOutput {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    parse(&tokens, &interner)
}

#[test]
fn test_parse_literal() {
    let result = parse_source("@main () -> int = 42");

    assert!(!result.has_errors());
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    assert!(matches!(body.kind, ExprKind::Int(42)));
}

#[test]
fn test_parse_binary_expr() {
    let result = parse_source("@add () -> int = 1 + 2 * 3");

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
fn test_parse_if_expr() {
    let result = parse_source("@test () -> int = if true then 1 else 2");

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
        assert!(else_branch.is_some());
    } else {
        panic!("Expected if expression");
    }
}

#[test]
fn test_parse_function_seq_run() {
    let result = parse_source("@test () -> int = run(let x = 1, let y = 2, x + y)");

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::FunctionSeq(FunctionSeq::Run { bindings, .. }) = &body.kind {
        let seq_bindings = result.arena.get_seq_bindings(*bindings);
        assert_eq!(seq_bindings.len(), 2);
    } else {
        panic!("Expected run function_seq, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_let_expression() {
    let result = parse_source("@test () = let x = 1");

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors(), "Expected no parse errors");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Let {
        pattern,
        ty,
        mutable,
        ..
    } = &body.kind
    {
        assert!(matches!(pattern, BindingPattern::Name(_)));
        assert!(ty.is_none());
        assert!(!mutable);
    } else {
        panic!("Expected let expression, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_let_with_type() {
    let result = parse_source("@test () = let x: int = 1");

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::Let { ty, .. } = &body.kind {
        assert!(ty.is_some());
    } else {
        panic!("Expected let expression");
    }
}

#[test]
fn test_parse_run_with_let() {
    let result = parse_source("@test () = run(let x = 1, x)");

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::FunctionSeq(FunctionSeq::Run { bindings, .. }) = &body.kind {
        let seq_bindings = result.arena.get_seq_bindings(*bindings);
        assert_eq!(seq_bindings.len(), 1);
    } else {
        panic!("Expected run function_seq, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_function_exp_print() {
    // Test parsing print function_exp (one of the remaining compiler patterns)
    let result = parse_source("@test () = print(msg: \"hello\")");

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors(), "Expected no parse errors");

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::FunctionExp(func_exp) = &body.kind {
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
    let result = parse_source(
        r#"@test () = timeout(
        operation: print(msg: "hi"),
        after: 5s
    )"#,
    );

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors(), "Expected no parse errors");
}

#[test]
fn test_parse_list() {
    let result = parse_source("@test () -> int = [1, 2, 3]");

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

    let result1 = parse_source("@main () -> int = 42");
    let result2 = parse_source("@main () -> int = 42");
    let result3 = parse_source("@main () -> int = 43");

    set.insert(result1);
    set.insert(result2); // duplicate
    set.insert(result3);

    assert_eq!(set.len(), 2);
}

#[test]
fn test_parse_timeout_pattern() {
    let result = parse_source(
        r#"@main () = timeout(
        operation: print(msg: "hello"),
        after: 5s
    )"#,
    );

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
fn test_parse_runner_syntax() {
    // Test the exact syntax used in the runner tests
    // Functions are called without @ prefix
    let result = parse_source(
        r#"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    let result = add(a: 1, b: 2),
    print(msg: "done")
)
"#,
    );

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
    let result = parse_source(
        r"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    @add(a: 1, b: 2)
)
",
    );

    assert!(
        result.has_errors(),
        "Expected parse error for @add in expression"
    );
}

#[test]
fn test_uses_clause_single_capability() {
    let result = parse_source(
        r"
@fetch (url: str) -> str uses Http = Http.get(url: url)
",
    );

    assert!(!result.has_errors(), "Expected no parse errors");
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    assert_eq!(func.capabilities.len(), 1);
}

#[test]
fn test_uses_clause_multiple_capabilities() {
    let result = parse_source(
        r#"
@save (data: str) -> void uses FileSystem, Async = FileSystem.write(path: "/data", content: data)
"#,
    );

    assert!(!result.has_errors(), "Expected no parse errors");
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    assert_eq!(func.capabilities.len(), 2);
}

#[test]
fn test_uses_clause_with_where() {
    // uses clause must come before where clause
    let result = parse_source(
        r"
@process<T> (data: T) -> T uses Logger where T: Clone = data
",
    );

    assert!(!result.has_errors(), "Expected no parse errors");
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    assert_eq!(func.capabilities.len(), 1);
    assert_eq!(func.where_clauses.len(), 1);
}

#[test]
fn test_no_uses_clause() {
    // Pure function - no uses clause
    let result = parse_source(
        r"
@add (a: int, b: int) -> int = a + b
",
    );

    assert!(!result.has_errors(), "Expected no parse errors");
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    assert!(func.capabilities.is_empty());
}

#[test]
fn test_with_capability_expression() {
    // with Capability = Provider in body
    let result = parse_source(
        r"
@example () -> int =
    with Http = MockHttp in
        42
",
    );

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
    let result = parse_source(
        r#"
@example () -> int =
    with Http = RealHttp { base_url: "https://api.example.com" } in
        fetch(url: "/data")
"#,
    );

    assert!(
        !result.has_errors(),
        "Expected no parse errors: {:?}",
        result.errors
    );
}

#[test]
fn test_with_capability_nested() {
    // Nested capability provisions
    let result = parse_source(
        r"
@example () -> int =
    with Http = MockHttp in
        with Cache = MockCache in
            42
",
    );

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
    // The `async` keyword is reserved but should cause a parse error when used as type.
    let result = parse_source(
        r"
@example () -> async int = 42
",
    );

    // Should have parse error - async is not a valid type modifier
    assert!(
        result.has_errors(),
        "async type modifier should not be supported"
    );
}

#[test]
fn test_async_keyword_reserved() {
    // The async keyword is reserved and cannot be used as an identifier
    let result = parse_source(
        r"
@test () -> int = run(
    let async = 42,
    async,
)
",
    );

    // Should have parse error - async is a reserved keyword
    assert!(result.has_errors(), "async should be a reserved keyword");
}

#[test]
fn test_uses_async_capability_parses() {
    // The correct way to declare async behavior: uses Async capability
    let result = parse_source(
        r"
trait Async {}

@async_op () -> int uses Async = 42
",
    );

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
    let result = parse_source("@test () -> int = 8 >> 2");

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
    let result = parse_source("@test () -> bool = 5 >= 3");

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
    // << should still work (single token from lexer)
    let result = parse_source("@test () -> int = 2 << 3");

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
    // Single > should still work
    let result = parse_source("@test () -> bool = 5 > 3");

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
    let result = parse_source("@test () -> int = 8 > > 2");

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
    let result = parse_source("@test () -> bool = 5 > = 3");

    // This should have errors because `> = 3` is invalid syntax
    assert!(
        result.has_errors(),
        "Expected parse errors for `> = 3` with space"
    );
}

#[test]
fn test_nested_generic_and_shift() {
    // Test that nested generics work in a type annotation and >> works in expression
    let result = parse_source(
        r"
@test () -> Result<Result<int, str>, str> = run(
    let x = 8 >> 2,
    Ok(Ok(x))
)",
    );

    assert!(
        !result.has_errors(),
        "Expected no parse errors for nested generics and >> operator: {:?}",
        result.errors
    );
}

// --- Context Management Tests ---

#[test]
fn test_struct_literal_in_expression() {
    // Struct literals work normally in expressions
    let result = parse_source(
        r"
type Point = { x: int, y: int }

@test () -> int = Point { x: 1, y: 2 }.x
",
    );

    assert!(
        !result.has_errors(),
        "Struct literal should parse in normal expression: {:?}",
        result.errors
    );
}

#[test]
fn test_struct_literal_in_if_then_body() {
    // Struct literals work in the then body of an if expression
    let result = parse_source(
        r"
type Point = { x: int, y: int }

@test () -> int = if true then Point { x: 1, y: 2 }.x else 0
",
    );

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
    let result = parse_source(
        r"
type Point = { x: int, y: int }

@test () -> int = if Point { x: 1, y: 2 }.x > 0 then 1 else 0
",
    );

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
    let tokens = ori_lexer::lex("@test () = 42", &interner);
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
