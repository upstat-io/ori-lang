use ori_ir::ast::Expr;
use ori_ir::canon::{CanExpr, ConstValue};
use ori_ir::{
    BinaryOp, DurationUnit, ExprArena, ExprKind, SharedInterner, SizeUnit, Span, UnaryOp,
};
use ori_types::{Idx, TypeCheckResult, TypedModule};

use crate::lower;

fn test_type_result(expr_types: Vec<Idx>) -> TypeCheckResult {
    let mut typed = TypedModule::new();
    for idx in expr_types {
        typed.expr_types.push(idx);
    }
    TypeCheckResult::ok(typed)
}

fn test_interner() -> SharedInterner {
    SharedInterner::new()
}

// Binary folding

#[test]
fn fold_int_addition() {
    // 1 + 2 → Constant(3)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(4, 5)));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
        Span::new(0, 5),
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);

    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Int(3));
        }
        other => panic!("expected Constant(3), got {other:?}"),
    }
}

#[test]
fn fold_int_subtraction() {
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(10), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Sub,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Int(7));
        }
        other => panic!("expected Constant(7), got {other:?}"),
    }
}

#[test]
fn fold_int_multiplication() {
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(6), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(7), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Mul,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Int(42));
        }
        other => panic!("expected Constant(42), got {other:?}"),
    }
}

#[test]
fn no_fold_division_by_zero() {
    // 10 / 0 should NOT be folded (runtime error).
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(10), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Div,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    // Should remain a Binary node (not folded).
    assert!(
        matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
        "division by zero should not be folded"
    );
}

#[test]
fn no_fold_integer_overflow() {
    // i64::MAX + 1 should NOT be folded.
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(i64::MAX), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(
        matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
        "integer overflow should not be folded"
    );
}

#[test]
fn fold_bool_and() {
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Bool(false), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::And,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::BOOL, Idx::BOOL, Idx::BOOL]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Bool(false));
        }
        other => panic!("expected Constant(false), got {other:?}"),
    }
}

#[test]
fn fold_int_comparison() {
    // 3 < 5 → Constant(true)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(5), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Lt,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::BOOL]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Bool(true));
        }
        other => panic!("expected Constant(true), got {other:?}"),
    }
}

// Unary folding

#[test]
fn fold_negation() {
    // -42 → Constant(-42)
    let mut arena = ExprArena::new();
    let operand = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Int(-42));
        }
        other => panic!("expected Constant(-42), got {other:?}"),
    }
}

#[test]
fn fold_not() {
    // !true → Constant(false)
    let mut arena = ExprArena::new();
    let operand = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Unary {
            op: UnaryOp::Not,
            operand,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::BOOL, Idx::BOOL]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Bool(false));
        }
        other => panic!("expected Constant(false), got {other:?}"),
    }
}

// Dead branch elimination

#[test]
fn dead_branch_if_true() {
    // if true { 42 } else { 99 } → 42
    let mut arena = ExprArena::new();
    let cond = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::DUMMY));
    let then_br = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::DUMMY));
    let else_br = arena.alloc_expr(Expr::new(ExprKind::Int(99), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::If {
            cond,
            then_branch: then_br,
            else_branch: else_br,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::BOOL, Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    // Should be the then-branch value directly (Int(42)).
    assert_eq!(
        *result.arena.kind(result.root),
        CanExpr::Int(42),
        "if true should eliminate to then-branch"
    );
}

#[test]
fn dead_branch_if_false() {
    // if false { 42 } else { 99 } → 99
    let mut arena = ExprArena::new();
    let cond = arena.alloc_expr(Expr::new(ExprKind::Bool(false), Span::DUMMY));
    let then_br = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::DUMMY));
    let else_br = arena.alloc_expr(Expr::new(ExprKind::Int(99), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::If {
            cond,
            then_branch: then_br,
            else_branch: else_br,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::BOOL, Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert_eq!(
        *result.arena.kind(result.root),
        CanExpr::Int(99),
        "if false should eliminate to else-branch"
    );
}

// Bitwise folding

#[test]
fn fold_bitwise_and() {
    // 0xFF & 0x0F → 0x0F
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(0xFF), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(0x0F), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::BitAnd,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Int(0x0F));
        }
        other => panic!("expected Constant(0x0F), got {other:?}"),
    }
}

#[test]
fn fold_shift_left() {
    // 1 << 4 → 16
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(4), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Shl,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Int(16));
        }
        other => panic!("expected Constant(16), got {other:?}"),
    }
}

// Runtime expressions not folded

#[test]
fn no_fold_runtime_binary() {
    // x + 1 should NOT be folded (x is a runtime variable).
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let name_x = interner.intern("x");

    let left = arena.alloc_expr(Expr::new(ExprKind::Ident(name_x), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(
        matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
        "runtime binary should not be folded"
    );
}

// Duration constant folding

#[test]
fn fold_duration_addition() {
    // 1s + 500ms → Constant(1_500_000_000ns)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 1,
            unit: DurationUnit::Seconds,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 500,
            unit: DurationUnit::Milliseconds,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::DURATION, Idx::DURATION, Idx::DURATION]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Duration {
                    value: 1_500_000_000,
                    unit: DurationUnit::Nanoseconds
                }
            );
        }
        other => panic!("expected Constant(1500000000ns), got {other:?}"),
    }
}

#[test]
fn fold_duration_subtraction() {
    // 2s - 500ms → Constant(1_500_000_000ns)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 2,
            unit: DurationUnit::Seconds,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 500,
            unit: DurationUnit::Milliseconds,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Sub,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::DURATION, Idx::DURATION, Idx::DURATION]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Duration {
                    value: 1_500_000_000,
                    unit: DurationUnit::Nanoseconds
                }
            );
        }
        other => panic!("expected Constant(1500000000ns), got {other:?}"),
    }
}

#[test]
fn fold_duration_comparison() {
    // 1s > 500ms → Constant(true)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 1,
            unit: DurationUnit::Seconds,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 500,
            unit: DurationUnit::Milliseconds,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Gt,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::DURATION, Idx::DURATION, Idx::BOOL]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Bool(true));
        }
        other => panic!("expected Constant(true), got {other:?}"),
    }
}

#[test]
fn fold_duration_equality_across_units() {
    // 1000ms == 1s → Constant(true)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 1000,
            unit: DurationUnit::Milliseconds,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 1,
            unit: DurationUnit::Seconds,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Eq,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::DURATION, Idx::DURATION, Idx::BOOL]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Bool(true));
        }
        other => panic!("expected Constant(true), got {other:?}"),
    }
}

#[test]
fn fold_duration_negation() {
    // -(1s) → Constant(-1_000_000_000ns)
    let mut arena = ExprArena::new();
    let operand = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 1,
            unit: DurationUnit::Seconds,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::DURATION, Idx::DURATION]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Duration {
                    value: (-1_000_000_000_i64).cast_unsigned(),
                    unit: DurationUnit::Nanoseconds,
                }
            );
        }
        other => panic!("expected Constant(-1000000000ns), got {other:?}"),
    }
}

#[test]
fn fold_duration_mul_int() {
    // 500ms * 3 → Constant(1_500_000_000ns)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 500,
            unit: DurationUnit::Milliseconds,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Mul,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::DURATION, Idx::INT, Idx::DURATION]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Duration {
                    value: 1_500_000_000,
                    unit: DurationUnit::Nanoseconds
                }
            );
        }
        other => panic!("expected Constant(1500000000ns), got {other:?}"),
    }
}

#[test]
fn fold_int_mul_duration() {
    // 2 * 1s → Constant(2_000_000_000ns)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::DUMMY));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 1,
            unit: DurationUnit::Seconds,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Mul,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::DURATION, Idx::DURATION]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Duration {
                    value: 2_000_000_000,
                    unit: DurationUnit::Nanoseconds
                }
            );
        }
        other => panic!("expected Constant(2000000000ns), got {other:?}"),
    }
}

#[test]
fn fold_duration_div_int() {
    // 1s / 4 → Constant(250_000_000ns)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 1,
            unit: DurationUnit::Seconds,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(4), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Div,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::DURATION, Idx::INT, Idx::DURATION]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Duration {
                    value: 250_000_000,
                    unit: DurationUnit::Nanoseconds
                }
            );
        }
        other => panic!("expected Constant(250000000ns), got {other:?}"),
    }
}

#[test]
fn no_fold_duration_div_zero() {
    // 1s / 0 should NOT be folded.
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Duration {
            value: 1,
            unit: DurationUnit::Seconds,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Div,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::DURATION, Idx::INT, Idx::DURATION]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(
        matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
        "duration div by zero should not be folded"
    );
}

// Size constant folding

#[test]
fn fold_size_addition() {
    // 1kb + 500b → Constant(1500b)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 1,
            unit: SizeUnit::Kilobytes,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 500,
            unit: SizeUnit::Bytes,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::SIZE, Idx::SIZE, Idx::SIZE]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Size {
                    value: 1500,
                    unit: SizeUnit::Bytes
                }
            );
        }
        other => panic!("expected Constant(1500b), got {other:?}"),
    }
}

#[test]
fn fold_size_subtraction() {
    // 1kb - 500b → Constant(500b)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 1,
            unit: SizeUnit::Kilobytes,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 500,
            unit: SizeUnit::Bytes,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Sub,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::SIZE, Idx::SIZE, Idx::SIZE]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Size {
                    value: 500,
                    unit: SizeUnit::Bytes
                }
            );
        }
        other => panic!("expected Constant(500b), got {other:?}"),
    }
}

#[test]
fn no_fold_size_negative_result() {
    // 500b - 1kb → NOT folded (result would be negative).
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 500,
            unit: SizeUnit::Bytes,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 1,
            unit: SizeUnit::Kilobytes,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Sub,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::SIZE, Idx::SIZE, Idx::SIZE]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(
        matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
        "size subtraction yielding negative should not be folded"
    );
}

#[test]
fn fold_size_comparison() {
    // 1mb > 1kb → Constant(true)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 1,
            unit: SizeUnit::Megabytes,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 1,
            unit: SizeUnit::Kilobytes,
        },
        Span::DUMMY,
    ));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Gt,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::SIZE, Idx::SIZE, Idx::BOOL]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(*result.constants.get(*cid), ConstValue::Bool(true));
        }
        other => panic!("expected Constant(true), got {other:?}"),
    }
}

#[test]
fn fold_size_mul_int() {
    // 500b * 3 → Constant(1500b)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 500,
            unit: SizeUnit::Bytes,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Mul,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::SIZE, Idx::INT, Idx::SIZE]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Size {
                    value: 1500,
                    unit: SizeUnit::Bytes
                }
            );
        }
        other => panic!("expected Constant(1500b), got {other:?}"),
    }
}

#[test]
fn no_fold_size_mul_negative_int() {
    // 500b * -1 → NOT folded (negative multiplier for Size).
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 500,
            unit: SizeUnit::Bytes,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(-1), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Mul,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::SIZE, Idx::INT, Idx::SIZE]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(
        matches!(result.arena.kind(result.root), CanExpr::Binary { .. }),
        "size * negative int should not be folded"
    );
}

#[test]
fn fold_size_div_int() {
    // 1kb / 4 → Constant(250b)
    let mut arena = ExprArena::new();
    let left = arena.alloc_expr(Expr::new(
        ExprKind::Size {
            value: 1,
            unit: SizeUnit::Kilobytes,
        },
        Span::DUMMY,
    ));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(4), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Div,
            left,
            right,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::SIZE, Idx::INT, Idx::SIZE]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ConstValue::Size {
                    value: 250,
                    unit: SizeUnit::Bytes
                }
            );
        }
        other => panic!("expected Constant(250b), got {other:?}"),
    }
}
