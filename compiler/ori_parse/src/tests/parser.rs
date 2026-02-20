//! Core parser tests.
//!
//! Tests for basic parsing functionality including literals, expressions,
//! operators, capabilities, and context management.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test assertions use unwrap/expect for clarity"
)]

use crate::{parse, ParseContext, ParseOutput, Parser};
use ori_ir::{
    BinaryOp, BindingPattern, ExprKind, FunctionExpKind, FunctionSeq, Mutability, StmtKind,
    StringInterner,
};

fn parse_source(source: &str) -> ParseOutput {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    parse(&tokens, &interner)
}

#[test]
fn test_parse_literal() {
    let result = parse_source("@main () -> int = 42;");

    assert!(!result.has_errors());
    assert_eq!(result.module.functions.len(), 1);

    let func = &result.module.functions[0];
    let body = result.arena.get_expr(func.body);
    assert!(matches!(body.kind, ExprKind::Int(42)));
}

#[test]
fn test_parse_binary_expr() {
    let result = parse_source("@add () -> int = 1 + 2 * 3;");

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
    let result = parse_source("@test () -> int = if true then 1 else 2;");

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
    let result = parse_source("@test () -> int = { let x = 1; let y = 2; x + y }");

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
    let result = parse_source("@test () -> void = let x = 1;");

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
    let result = parse_source("@test () -> void = let x: int = 1;");

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
    let result = parse_source("@test () -> int = { let x = 1; x }");

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
    // Test parsing print function_exp (one of the remaining compiler patterns)
    let result = parse_source("@test () -> void = print(msg: \"hello\");");

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
    );"#,
    );

    if result.has_errors() {
        eprintln!("Parse errors: {:?}", result.errors);
    }
    assert!(!result.has_errors(), "Expected no parse errors");
}

#[test]
fn test_parse_list() {
    let result = parse_source("@test () -> int = [1, 2, 3];");

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

    let result1 = parse_source("@main () -> int = 42;");
    let result2 = parse_source("@main () -> int = 42;");
    let result3 = parse_source("@main () -> int = 43;");

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
    );"#,
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
fn test_parse_block_in_test() {
    // Test block expression syntax in a test function with target
    let result = parse_source(
        r#"
@add (a: int, b: int) -> int = a + b;

@test_add tests @add () -> void = {
    let result = add(a: 1, b: 2);
    print(msg: "done")
}
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
@add (a: int, b: int) -> int = a + b;

@test_add tests @add () -> void = {
    @add(a: 1, b: 2)
}
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
@fetch (url: str) -> str uses Http = Http.get(url: url);
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
@save (data: str) -> void uses FileSystem, Async = FileSystem.write(path: "/data", content: data);
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
@process<T> (data: T) -> T uses Logger where T: Clone = data;
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
@add (a: int, b: int) -> int = a + b;
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
        42;
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
        fetch(url: "/data");
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
            42;
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
@example () -> async int = 42;
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
@test () -> int = {
    let async = 42;
    async
}
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

@async_op () -> int uses Async = 42;
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
    let result = parse_source("@test () -> int = 8 >> 2;");

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
    let result = parse_source("@test () -> bool = 5 >= 3;");

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
    let result = parse_source("@test () -> int = 2 << 3;");

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
    let result = parse_source("@test () -> bool = 5 > 3;");

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
    let result = parse_source("@test () -> int = 8 > > 2;");

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
    let result = parse_source("@test () -> bool = 5 > = 3;");

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
@test () -> Result<Result<int, str>, str> = {
    let x = 8 >> 2;
    Ok(Ok(x))
}",
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

@test () -> int = Point { x: 1, y: 2 }.x;
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

@test () -> int = if true then Point { x: 1, y: 2 }.x else 0;
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

@test () -> int = if Point { x: 1, y: 2 }.x > 0 then 1 else 0;
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
    let tokens = ori_lexer::lex("@test () = 42;", &interner);
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

@main () -> void = ();
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

@main () -> int = 42;
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
@main () -> int = 42;
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
        let output = parse_with_comments("@main () -> int = 42;");

        assert!(output.metadata.comments.is_empty());
    }

    #[test]
    fn test_metadata_regular_vs_doc_comments() {
        let source = r"// Regular comment
// #Doc comment
@main () -> int = 42;
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
@foo () -> int = 1;

// #Function 2
@bar () -> int = 2;
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
        let source = "@main () -> int = {\n    let x = 1;\n    x + 1\n}\n";
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
        let tokens = ori_lexer::lex("@main () -> int = 42;", &interner);
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
@main () -> int = 42;
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

@main () -> int = 42;
";
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
        let source = r"@main () -> int = 42;
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
            other @ crate::ParseWarning::UnknownCallingConvention { .. } => {
                panic!("expected DetachedDocComment, got {other:?}")
            }
        }
    }

    #[test]
    fn test_no_warning_for_regular_comments() {
        let source = r"// Regular comment (not a doc comment)

@main () -> int = 42;
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

@main () -> int = 42;
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

@main () -> int = 42;
";
        let mut output = parse_with_comments(source);
        output.check_detached_doc_comments();

        assert!(!output.warnings.is_empty());
        let diagnostic = output.warnings[0].to_diagnostic();

        // Verify diagnostic has correct severity
        assert!(diagnostic.code.is_warning());
    }
}

// Tests that dispatch_declaration() correctly handles ALL token kinds at the
// module top level. Previously, unrecognized tokens were silently eaten by
// a catch-all `self.advance()`, causing `return 42`, `break`, bare integers,
// and other invalid top-level code to pass `ori check` as OK.

/// All valid declaration forms must parse without errors.
#[test]
fn test_valid_declarations_at_module_level() {
    let valid_sources = &[
        // Functions
        "@add (a: int, b: int) -> int = a + b;",
        "@main () -> void = print(msg: \"hello\");",
        // Types
        "type Point = { x: int, y: int }",
        "type Color = Red | Green | Blue;",
        // Traits
        "trait Printable {\n    @to_str (self) -> str\n}",
        // Impl blocks
        "type Foo = { x: int }\nimpl Foo {\n    @get_x (self) -> int = self.x;\n}",
        // Constants
        "let $x = 42;",
        "let $name = \"hello\";",
        // Constants without `let` (backwards compat)
        "$y = 100;",
        // Imports
        "use std.math { sqrt }",
        // Extend blocks
        "type Bar = { v: int }\nextend Bar {\n    @val (self) -> int = self.v;\n}",
        // Visibility modifiers
        "pub @add (a: int, b: int) -> int = a + b;",
        "pub type Color = Red | Green | Blue;",
        "pub let $x = 42;",
        // Multiple declarations
        "type A = { x: int }\ntype B = { y: str }",
        "@foo () -> int = 1;\n@bar () -> int = 2;",
        // Empty file
        "",
        // Only whitespace/newlines
        "\n\n\n",
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
    let result = parse_source("return 42");
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
    let result = parse_source("@foo () -> int = return 42;");
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
    let result = parse_source("let x = 42");
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
    let result = parse_source("let $timeout = 30;");
    assert!(!result.has_errors());
    assert_eq!(result.module.consts.len(), 1);
}

/// `pub let $x = 42` at module level parses as a public constant.
#[test]
fn test_pub_const_let_at_module_level_accepted() {
    let result = parse_source("pub let $api_base = \"https://example.com\";");
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
    let result = parse_source("42\ntrue\n\"hello\"");
    assert!(result.has_errors());
    assert!(
        result.errors.len() >= 3,
        "Expected at least 3 errors for 3 invalid tokens, got {}: {:?}",
        result.errors.len(),
        result.errors
    );
}

/// Valid declarations mixed with invalid tokens: valid parts still parse.
#[test]
fn test_mixed_valid_and_invalid_at_module_level() {
    let result = parse_source("@foo () -> int = 42;\n42\n@bar () -> int = 1;");
    assert!(result.has_errors());
    // The valid functions should still be parsed
    assert_eq!(
        result.module.functions.len(),
        2,
        "Both valid functions should parse despite intervening invalid token"
    );
}

/// Semicolons after top-level items are accepted (optional during dual-mode).
#[test]
fn test_semicolons_after_top_level_items_accepted() {
    let result = parse_source("@foo () -> int = 42;");
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
fn test_labeled_break() {
    let (result, interner) =
        parse_source_with_interner("@f () -> int = loop:outer { break:outer 42 }");
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
    let (result, interner) =
        parse_source_with_interner("@f () -> void = for:outer x in [1] do continue:outer;");
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
fn test_unlabeled_break_still_works() {
    let result = parse_source("@f () -> int = loop { break 42 }");
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
fn test_unlabeled_continue_still_works() {
    let result = parse_source("@f () -> void = for x in [1] do continue;");
    assert!(
        !result.has_errors(),
        "unlabeled continue should still parse"
    );
}

#[test]
fn test_labeled_for_loop() {
    let (result, interner) =
        parse_source_with_interner("@f () -> void = for:items x in [1, 2, 3] do break;");
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
    let (result, interner) = parse_source_with_interner("@f () -> int = loop:main { break 0 }");
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
fn test_labeled_continue_with_value() {
    let result = parse_source(
        "@f () -> [int] = for:lp x in [1, 2, 3] yield {\
            if x == 2 then continue:lp 0; x }",
    );
    assert!(
        !result.has_errors(),
        "labeled continue with value should parse: {result:?}"
    );
}

#[test]
fn test_nested_labels() {
    let result = parse_source(
        "@f () -> void = for:outer x in [1] do for:inner y in [2] do {\
            if y == 2 then continue:outer;\
            if x == 1 then break:inner }",
    );
    assert!(
        !result.has_errors(),
        "nested labels should parse: {result:?}"
    );
}

#[test]
fn test_label_with_space_is_not_label() {
    // `break :outer` with a space should NOT parse as a label.
    // The `:outer` is treated as a value expression which is invalid.
    let result = parse_source("@f () -> void = for x in [1] do break :outer;");
    assert!(
        result.has_errors(),
        "space before colon should prevent label parsing"
    );
}

#[test]
fn test_tuple_field_access() {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex("@f (t: (int, int)) -> int = t.0;", &interner);
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
    // Chained tuple field access requires parentheses: (t.0).1
    // because the lexer tokenizes `0.1` as a float literal.
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex("@f (t: ((int, int), int)) -> int = (t.0).1;", &interner);
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
fn test_bare_chained_tuple_field_is_error() {
    // `t.0.1` without parens fails because lexer tokenizes `0.1` as float.
    // This is a known limitation — use `(t.0).1` instead.
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex("@f (t: ((int, int), int)) -> int = t.0.1;", &interner);
    let result = parse(&tokens, &interner);
    assert!(
        result.has_errors(),
        "bare t.0.1 should fail (lexer sees 0.1 as float)"
    );
}

// =============================================================================
// $ immutability in let parsing paths (LEAK-1)
// =============================================================================
//
// Three paths parse let bindings:
// 1. parse_block_let_binding (blocks)     — correct: lets parse_binding_pattern handle $
// 2. parse_let_expr_body (expression-form) — buggy: consumes $ before parse_binding_pattern
// 3. parse_try_let_binding (try blocks)    — buggy: consumes $ before parse_binding_pattern
//
// The evaluator reads mutability from BindingPattern::Name.mutable, not from
// ExprKind::Let.mutable or StmtKind::Let.mutable. When $ is consumed before
// parse_binding_pattern sees it, the pattern records Mutable (wrong).

#[test]
fn test_let_expr_dollar_immutable_on_pattern() {
    // Expression-form let: `let $x = 42` should produce Immutable on the pattern.
    let result = parse_source("@test () -> void = let $x = 42;");
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

    // Statement-level mutability is correct (set before parse_binding_pattern)
    assert_eq!(
        *mutable,
        Mutability::Immutable,
        "ExprKind::Let.mutable should be Immutable for `let $x`"
    );

    // Pattern-level mutability MUST also be Immutable — this is what the evaluator reads
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
    // Regression guard: block-form let correctly passes $ to parse_binding_pattern.
    let result = parse_source("@test () -> int = { let $x = 42; x }");
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
    let result = parse_source("@test () -> void = try { let $x = Ok(5); Ok(x) }");

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
    let result = parse_source("@test () -> void = let x = 42;");
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
