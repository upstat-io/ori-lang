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
        assert!(else_branch.is_present());
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

    if let ExprKind::FunctionSeq(seq_id) = &body.kind {
        let seq = result.arena.get_function_seq(*seq_id);
        if let FunctionSeq::Run { bindings, .. } = seq {
            let seq_bindings = result.arena.get_seq_bindings(*bindings);
            assert_eq!(seq_bindings.len(), 2);
        } else {
            panic!("Expected FunctionSeq::Run, got {seq:?}");
        }
    } else {
        panic!("Expected run function_seq, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_let_expression() {
    let result = parse_source("@test () -> void = let x = 1");

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
        assert!(matches!(pattern, BindingPattern::Name(_)));
        assert!(!ty.is_valid());
        // Per spec: let x = v is mutable by default
        assert!(mutable);
    } else {
        panic!("Expected let expression, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_let_with_type() {
    let result = parse_source("@test () -> void = let x: int = 1");

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
fn test_parse_run_with_let() {
    let result = parse_source("@test () -> int = run(let x = 1, x)");

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors());

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);

    if let ExprKind::FunctionSeq(seq_id) = &body.kind {
        let seq = result.arena.get_function_seq(*seq_id);
        if let FunctionSeq::Run { bindings, .. } = seq {
            let seq_bindings = result.arena.get_seq_bindings(*bindings);
            assert_eq!(seq_bindings.len(), 1);
        } else {
            panic!("Expected FunctionSeq::Run, got {seq:?}");
        }
    } else {
        panic!("Expected run function_seq, got {:?}", body.kind);
    }
}

#[test]
fn test_parse_function_exp_print() {
    // Test parsing print function_exp (one of the remaining compiler patterns)
    let result = parse_source("@test () -> void = print(msg: \"hello\")");

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
    let result = parse_source(
        r#"@test () -> void = timeout(
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
        r#"@main () -> void = timeout(
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

// === Metadata Tests ===

mod metadata_tests {
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
        let source = r"// #Description
// This is a test

@main () -> void = ()
";
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
        let source = r"// #Description
// A simple function

@main () -> int = 42
";
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
        let source = r"// #First description
// #Second description

// #This one should attach
@main () -> int = 42
";
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
        let output = parse_with_comments("@main () -> int = 42");

        assert!(output.metadata.comments.is_empty());
    }

    #[test]
    fn test_metadata_regular_vs_doc_comments() {
        let source = r"// Regular comment
// #Doc comment
@main () -> int = 42
";
        let output = parse_with_comments(source);

        assert_eq!(output.metadata.comments.len(), 2);

        // Check comment kinds
        let comments: Vec<_> = output.metadata.comments.iter().collect();
        assert!(!comments[0].kind.is_doc()); // Regular
        assert!(comments[1].kind.is_doc()); // DocDescription
    }

    #[test]
    fn test_metadata_multiple_functions_with_comments() {
        let source = r"// #Function 1
@foo () -> int = 1

// #Function 2
@bar () -> int = 2
";
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
        let source = "@main () -> int =\n    let x = 1\n    x + 1\n";
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
        let source = "// Comment\n@main () -> int = 42\n";
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
        let tokens = ori_lexer::lex("@main () -> int = 42", &interner);
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
        let lex_output = ori_lexer::lex_with_comments("// test\n@main () -> int = 42", &interner);

        // Extract metadata before giving to parser
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

    // === Warning Tests ===

    #[test]
    fn test_no_warnings_when_doc_comments_attached() {
        let source = r"// #Description
@main () -> int = 42
";
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert!(
            output.warnings.is_empty(),
            "Should have no warnings when doc comment is attached"
        );
    }

    #[test]
    fn test_warning_for_detached_doc_comment_blank_line() {
        let source = r"// #Detached doc

@main () -> int = 42
";
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert_eq!(output.warnings.len(), 1, "Should have one warning");
        match &output.warnings[0] {
            crate::ParseWarning::DetachedDocComment { reason, .. } => {
                assert_eq!(*reason, crate::DetachmentReason::BlankLine);
            }
        }
    }

    #[test]
    fn test_warning_for_doc_comment_at_end_of_file() {
        let source = r"@main () -> int = 42
// #Orphan at end
";
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
        }
    }

    #[test]
    fn test_no_warning_for_regular_comments() {
        let source = r"// Regular comment (not a doc comment)

@main () -> int = 42
";
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
        let source = r"// #Detached

@main () -> int = 42
";
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
        let source = r"// #Detached

@main () -> int = 42
";
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert!(!output.warnings.is_empty());
        let diagnostic = output.warnings[0].to_diagnostic();

        // Verify diagnostic has correct severity
        assert!(diagnostic.code.is_warning());
    }
}
