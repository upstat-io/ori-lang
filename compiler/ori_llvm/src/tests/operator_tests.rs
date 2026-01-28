//! Tests for binary and unary operators.

use inkwell::context::Context;
use ori_ir::ast::{BinaryOp, Expr, ExprKind, UnaryOp};
use ori_ir::{ExprArena, StringInterner, TypeId};

use super::helper::TestCodegen;

// === Integer Arithmetic ===

#[test]
fn test_subtract() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 10 - 3 = 7
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Sub, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_sub");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_sub").expect("JIT failed");
    assert_eq!(result, 7);
}

#[test]
fn test_multiply() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 6 * 7 = 42
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(6),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(7),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Mul, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_mul");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_mul").expect("JIT failed");
    assert_eq!(result, 42);
}

#[test]
fn test_divide() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 20 / 4 = 5
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(20),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(4),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Div, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_div");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_div").expect("JIT failed");
    assert_eq!(result, 5);
}

#[test]
fn test_modulo() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 17 % 5 = 2
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(17),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Mod, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_mod");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_mod").expect("JIT failed");
    assert_eq!(result, 2);
}

// === Comparison Operators ===

#[test]
fn test_less_than_true() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 3 < 5 = true
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Lt, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_lt");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_lt").expect("JIT failed");
    assert!(result);
}

#[test]
fn test_less_than_false() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 5 < 3 = false
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Lt, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_lt_false");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_lt_false").expect("JIT failed");
    assert!(!result);
}

#[test]
fn test_greater_than() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 10 > 5 = true
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Gt, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_gt");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_gt").expect("JIT failed");
    assert!(result);
}

#[test]
fn test_less_than_or_equal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 5 <= 5 = true
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::LtEq, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_le");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_le").expect("JIT failed");
    assert!(result);
}

#[test]
fn test_greater_than_or_equal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 5 >= 5 = true
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::GtEq, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_ge");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_ge").expect("JIT failed");
    assert!(result);
}

#[test]
fn test_equal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 42 == 42 = true
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Eq, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_eq");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_eq").expect("JIT failed");
    assert!(result);
}

#[test]
fn test_not_equal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 1 != 2 = true
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::NotEq, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_ne");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_ne").expect("JIT failed");
    assert!(result);
}

// === Logical Operators ===

#[test]
fn test_logical_and_true() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: true && true = true
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::And, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_and");
    let expr_types = vec![TypeId::BOOL, TypeId::BOOL, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_and").expect("JIT failed");
    assert!(result);
}

#[test]
fn test_logical_and_false() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: true && false = false
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::And, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_and_false");
    let expr_types = vec![TypeId::BOOL, TypeId::BOOL, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_and_false").expect("JIT failed");
    assert!(!result);
}

#[test]
fn test_logical_or_true() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: false || true = true
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Or, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_or");
    let expr_types = vec![TypeId::BOOL, TypeId::BOOL, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_or").expect("JIT failed");
    assert!(result);
}

#[test]
fn test_logical_or_false() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: false || false = false
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Or, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_or_false");
    let expr_types = vec![TypeId::BOOL, TypeId::BOOL, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_or_false").expect("JIT failed");
    assert!(!result);
}

// === Unary Operators ===

#[test]
fn test_unary_negate() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: -42 = -42
    let operand = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Unary { op: UnaryOp::Neg, operand },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_neg");
    let expr_types = vec![TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_neg").expect("JIT failed");
    assert_eq!(result, -42);
}

#[test]
fn test_unary_not() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: !true = false
    let operand = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Unary { op: UnaryOp::Not, operand },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_not");
    let expr_types = vec![TypeId::BOOL, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_not").expect("JIT failed");
    assert!(!result);
}

#[test]
fn test_unary_not_false() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: !false = true
    let operand = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Unary { op: UnaryOp::Not, operand },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_not_false");
    let expr_types = vec![TypeId::BOOL, TypeId::BOOL];

    codegen.compile_function(fn_name, &[], &[], TypeId::BOOL, expr, &arena, &expr_types);

    let result = codegen.jit_execute_bool("test_not_false").expect("JIT failed");
    assert!(result);
}

// === Bitwise Operators ===

#[test]
fn test_bitwise_and() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 0b1100 & 0b1010 = 0b1000 = 8
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0b1100),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0b1010),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::BitAnd, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_bitand");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_bitand").expect("JIT failed");
    assert_eq!(result, 0b1000);
}

#[test]
fn test_bitwise_or() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 0b1100 | 0b0011 = 0b1111 = 15
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0b1100),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0b0011),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::BitOr, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_bitor");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_bitor").expect("JIT failed");
    assert_eq!(result, 0b1111);
}

#[test]
fn test_bitwise_xor() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 0b1100 ^ 0b1010 = 0b0110 = 6
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0b1100),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0b1010),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::BitXor, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_bitxor");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_bitxor").expect("JIT failed");
    assert_eq!(result, 0b0110);
}

#[test]
fn test_shift_left() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 1 << 4 = 16
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(4),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Shl, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_shl");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_shl").expect("JIT failed");
    assert_eq!(result, 16);
}

#[test]
fn test_shift_right() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 16 >> 2 = 4
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Int(16),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: ori_ir::Span::new(0, 1),
    });
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Shr, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_shr");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_shr").expect("JIT failed");
    assert_eq!(result, 4);
}

// === Float Operators ===

#[test]
fn test_float_add() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 1.5 + 2.5 = 4.0 (we'll test by converting to int)
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Float(1.5f64.to_bits()),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Float(2.5f64.to_bits()),
        span: ori_ir::Span::new(0, 1),
    });
    let float_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Add, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    // Convert to int
    let int_name = interner.intern("int");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(int_name),
        span: ori_ir::Span::new(0, 1),
    });
    let args = arena.alloc_expr_list([float_expr]);
    let call_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Call { func: func_ident, args },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_float_add");
    let expr_types = vec![TypeId::FLOAT, TypeId::FLOAT, TypeId::FLOAT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, call_expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_float_add").expect("JIT failed");
    assert_eq!(result, 4);
}

#[test]
fn test_float_mul() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: 2.0 * 3.0 = 6.0
    let left = arena.alloc_expr(Expr {
        kind: ExprKind::Float(2.0f64.to_bits()),
        span: ori_ir::Span::new(0, 1),
    });
    let right = arena.alloc_expr(Expr {
        kind: ExprKind::Float(3.0f64.to_bits()),
        span: ori_ir::Span::new(0, 1),
    });
    let float_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op: BinaryOp::Mul, left, right },
        span: ori_ir::Span::new(0, 1),
    });

    // Convert to int
    let int_name = interner.intern("int");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(int_name),
        span: ori_ir::Span::new(0, 1),
    });
    let args = arena.alloc_expr_list([float_expr]);
    let call_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Call { func: func_ident, args },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_float_mul");
    let expr_types = vec![TypeId::FLOAT, TypeId::FLOAT, TypeId::FLOAT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(fn_name, &[], &[], TypeId::INT, call_expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_float_mul").expect("JIT failed");
    assert_eq!(result, 6);
}
