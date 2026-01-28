use inkwell::context::Context;
use ori_ir::ast::{Expr, ExprKind};
use ori_ir::{ExprArena, StringInterner, TypeId};

use super::helper::TestCodegen;

#[test]
fn test_string_literal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> str { "hello" }
    let mut arena = ExprArena::new();

    // String literal "hello"
    let hello = interner.intern("hello");
    let str_expr = arena.alloc_expr(Expr {
        kind: ExprKind::String(hello),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_str");
    let expr_types = vec![TypeId::STR];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::STR,
        str_expr,
        &arena,
        &expr_types,
    );

    println!("String IR:\n{}", codegen.print_to_string());

    // Verify IR contains string constant
    let ir = codegen.print_to_string();
    assert!(ir.contains("hello")); // String content
    assert!(ir.contains("{ i64, ptr }")); // String struct type
    assert!(ir.contains("i64 5")); // Length = 5
}

#[test]
fn test_string_multiple() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create two functions that use the same string literal
    // Should reuse the global constant
    let mut arena = ExprArena::new();

    let hello = interner.intern("world");
    let str_expr1 = arena.alloc_expr(Expr {
        kind: ExprKind::String(hello),
        span: ori_ir::Span::new(0, 1),
    });
    let str_expr2 = arena.alloc_expr(Expr {
        kind: ExprKind::String(hello),
        span: ori_ir::Span::new(0, 1),
    });

    let fn1_name = interner.intern("test_str1");
    let fn2_name = interner.intern("test_str2");
    let expr_types = vec![TypeId::STR];

    codegen.compile_function(
        fn1_name,
        &[],
        &[],
        TypeId::STR,
        str_expr1,
        &arena,
        &expr_types,
    );

    codegen.compile_function(
        fn2_name,
        &[],
        &[],
        TypeId::STR,
        str_expr2,
        &arena,
        &expr_types,
    );

    println!("Multiple Strings IR:\n{}", codegen.print_to_string());

    // Verify only one global string constant declaration
    let ir = codegen.print_to_string();
    // Count the actual global declarations (lines starting with @.str)
    let global_count = ir
        .lines()
        .filter(|line: &&str| line.trim_start().starts_with("@.str."))
        .count();
    // Should have exactly 1 global string (reused between functions)
    assert_eq!(
        global_count, 1,
        "Expected 1 global string constant, found {global_count}"
    );
}

#[test]
fn test_string_empty() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> str { "" }
    let mut arena = ExprArena::new();

    // Empty string
    let empty = interner.intern("");
    let str_expr = arena.alloc_expr(Expr {
        kind: ExprKind::String(empty),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_empty_str");
    let expr_types = vec![TypeId::STR];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::STR,
        str_expr,
        &arena,
        &expr_types,
    );

    println!("Empty String IR:\n{}", codegen.print_to_string());

    // Verify IR contains zero length
    let ir = codegen.print_to_string();
    assert!(ir.contains("i64 0")); // Length = 0
}
