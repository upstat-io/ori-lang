use inkwell::context::Context;
use ori_ir::ast::{BinaryOp, Expr, ExprKind};
use ori_ir::{ExprArena, StringInterner};
use ori_types::Idx;

use super::helper::TestCodegen;

#[test]
fn test_simple_add() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create a simple function: fn add() -> i64 { 2 + 3 }
    let mut arena = ExprArena::new();

    // Build: 2 + 3
    let two = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: ori_ir::Span::new(0, 1),
    });
    let three = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });
    let add_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: two,
            right: three,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_add");
    let expr_types = vec![Idx::INT, Idx::INT, Idx::INT];

    codegen.compile_function(fn_name, &[], &[], Idx::INT, add_expr, &arena, &expr_types);

    // Print IR for debugging
    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Generated LLVM IR:\n{}", codegen.print_to_string());
    }

    // JIT execute
    let result = codegen.jit_execute_i64("test_add").expect("JIT failed");
    assert_eq!(result, 5);
}

#[test]
fn test_duration_literal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int { 5s } -> 5_000_000_000 (nanoseconds)
    let mut arena = ExprArena::new();

    let duration_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Duration {
            value: 5,
            unit: ori_ir::DurationUnit::Seconds,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_duration");
    let expr_types = vec![Idx::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        Idx::INT,
        duration_expr,
        &arena,
        &expr_types,
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Duration IR:\n{}", codegen.print_to_string());
    }

    // JIT execute - should return 5_000_000_000 (5 seconds in nanoseconds)
    let result = codegen
        .jit_execute_i64("test_duration")
        .expect("JIT failed");
    assert_eq!(result, 5_000_000_000);
}

#[test]
fn test_size_literal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int { 2kb } -> 2000 (bytes)
    // Ori uses SI units: 1kb = 1000 bytes (not 1024)
    let mut arena = ExprArena::new();

    let size_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Size {
            value: 2,
            unit: ori_ir::SizeUnit::Kilobytes,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_size");
    let expr_types = vec![Idx::INT];

    codegen.compile_function(fn_name, &[], &[], Idx::INT, size_expr, &arena, &expr_types);

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Size IR:\n{}", codegen.print_to_string());
    }

    // JIT execute - should return 2000 (2 * 1000, SI units)
    let result = codegen.jit_execute_i64("test_size").expect("JIT failed");
    assert_eq!(result, 2000);
}
