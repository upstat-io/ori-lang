//! Tests for built-in type conversion functions (str, int, float, byte).

use inkwell::context::Context;
use ori_ir::ast::{Expr, ExprKind};
use ori_ir::{ExprArena, StringInterner, TypeId};

use super::helper::TestCodegen;

#[test]
fn test_builtin_int_from_int() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: int(42) - should return 42
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });

    let int_name = interner.intern("int");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(int_name),
        span: ori_ir::Span::new(0, 1),
    });

    let args = arena.alloc_expr_list([arg]);
    let call_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Call {
            func: func_ident,
            args,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_int");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        call_expr,
        &arena,
        &expr_types,
    );

    let result = codegen.jit_execute_i64("test_int").expect("JIT failed");
    assert_eq!(result, 42);
}

#[test]
fn test_builtin_int_from_bool_true() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: int(true) - should return 1
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: ori_ir::Span::new(0, 1),
    });

    let int_name = interner.intern("int");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(int_name),
        span: ori_ir::Span::new(0, 1),
    });

    let args = arena.alloc_expr_list([arg]);
    let call_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Call {
            func: func_ident,
            args,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_int_bool");
    let expr_types = vec![TypeId::BOOL, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        call_expr,
        &arena,
        &expr_types,
    );

    let result = codegen
        .jit_execute_i64("test_int_bool")
        .expect("JIT failed");
    assert_eq!(result, 1);
}

#[test]
fn test_builtin_int_from_bool_false() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: int(false) - should return 0
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });

    let int_name = interner.intern("int");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(int_name),
        span: ori_ir::Span::new(0, 1),
    });

    let args = arena.alloc_expr_list([arg]);
    let call_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Call {
            func: func_ident,
            args,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_int_bool_false");
    let expr_types = vec![TypeId::BOOL, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        call_expr,
        &arena,
        &expr_types,
    );

    let result = codegen
        .jit_execute_i64("test_int_bool_false")
        .expect("JIT failed");
    assert_eq!(result, 0);
}

#[test]
fn test_builtin_int_from_float() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: int(3.7) - should return 3 (truncated)
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Float(3.7f64.to_bits()),
        span: ori_ir::Span::new(0, 1),
    });

    let int_name = interner.intern("int");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(int_name),
        span: ori_ir::Span::new(0, 1),
    });

    let args = arena.alloc_expr_list([arg]);
    let call_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Call {
            func: func_ident,
            args,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_int_float");
    let expr_types = vec![TypeId::FLOAT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        call_expr,
        &arena,
        &expr_types,
    );

    let result = codegen
        .jit_execute_i64("test_int_float")
        .expect("JIT failed");
    assert_eq!(result, 3);
}

#[test]
fn test_builtin_byte_from_int() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: byte(255) - should return 255
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Int(255),
        span: ori_ir::Span::new(0, 1),
    });

    let byte_name = interner.intern("byte");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(byte_name),
        span: ori_ir::Span::new(0, 1),
    });

    let args = arena.alloc_expr_list([arg]);
    let call_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Call {
            func: func_ident,
            args,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_byte");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::BYTE];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        call_expr,
        &arena,
        &expr_types,
    );

    // Note: byte gets zero-extended to i64 for the return
    let result = codegen.jit_execute_i64("test_byte").expect("JIT failed");
    assert_eq!(result & 0xFF, 255);
}

#[test]
fn test_builtin_byte_truncation() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: byte(256) - should return 0 (truncated to 8 bits)
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Int(256),
        span: ori_ir::Span::new(0, 1),
    });

    let byte_name = interner.intern("byte");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(byte_name),
        span: ori_ir::Span::new(0, 1),
    });

    let args = arena.alloc_expr_list([arg]);
    let call_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Call {
            func: func_ident,
            args,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_byte_trunc");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::BYTE];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        call_expr,
        &arena,
        &expr_types,
    );

    let result = codegen
        .jit_execute_i64("test_byte_trunc")
        .expect("JIT failed");
    assert_eq!(result & 0xFF, 0);
}
