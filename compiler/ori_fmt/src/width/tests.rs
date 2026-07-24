//! Tests for width calculation.

use super::*;
use crate::test_util::has_wildcard_match_arm;
use ori_ir::{
    ast::{Expr, ExprKind, Stmt, StmtKind},
    BinaryOp, ExprArena, Name, Span, StmtRange, StringInterner, UnaryOp,
};

/// Helper to create a test expression in the arena.
fn make_expr(arena: &mut ExprArena, kind: ExprKind) -> ExprId {
    arena.alloc_expr(Expr::new(kind, Span::new(0, 1)))
}

// Raw-helper unit tests (int/float/bool/string/char/duration/size/operator/
// binding-pattern width) live in each width submodule's sibling `tests.rs`
// (`literals/tests.rs`, `compounds/tests.rs`, `operators/tests.rs`,
// `patterns/tests.rs`) alongside the functions they test. This file covers
// only the `WidthCalculator` dispatch over `ExprKind`.

#[test]
fn test_width_int_literal() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let expr = make_expr(&mut arena, ExprKind::Int(42));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(expr), 2); // "42"
}

#[test]
fn test_width_bool_literal() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let true_expr = make_expr(&mut arena, ExprKind::Bool(true));
    let false_expr = make_expr(&mut arena, ExprKind::Bool(false));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(true_expr), 4); // "true"
    assert_eq!(calc.width(false_expr), 5); // "false"
}

#[test]
fn test_width_string_literal() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let name = interner.intern("hello");
    let expr = make_expr(&mut arena, ExprKind::String(name));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(expr), 7); // '"hello"'
}

#[test]
fn test_width_identifier() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let name = interner.intern("foo");
    let expr = make_expr(&mut arena, ExprKind::Ident(name));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(expr), 3); // "foo"
}

#[test]
fn test_width_config_ref() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let name = interner.intern("timeout");
    let expr = make_expr(&mut arena, ExprKind::Const(name));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(expr), 8); // "$timeout"
}

#[test]
fn test_width_function_ref() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let name = interner.intern("main");
    let expr = make_expr(&mut arena, ExprKind::FunctionRef(name));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(expr), 5); // "@main"
}

#[test]
fn test_width_binary_expr() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // 1 + 2
    let left = make_expr(&mut arena, ExprKind::Int(1));
    let right = make_expr(&mut arena, ExprKind::Int(2));
    let binary = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
    );

    let mut calc = WidthCalculator::new(&arena, &interner);
    assert_eq!(calc.width(binary), 5); // "1 + 2"
}

#[test]
fn test_width_binary_lt() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // 1 < 2
    let left = make_expr(&mut arena, ExprKind::Int(1));
    let right = make_expr(&mut arena, ExprKind::Int(2));
    let lt = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Lt,
            left,
            right,
        },
    );

    let mut calc = WidthCalculator::new(&arena, &interner);
    // "1 < 2" = 1 + 3 + 1 = 5 (operator is " < " = 3 chars)
    assert_eq!(calc.width(lt), 5);
}

#[test]
fn test_width_binary_gt() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // 1 > 2
    let left = make_expr(&mut arena, ExprKind::Int(1));
    let right = make_expr(&mut arena, ExprKind::Int(2));
    let gt = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Gt,
            left,
            right,
        },
    );

    let mut calc = WidthCalculator::new(&arena, &interner);
    // "1 > 2" = 1 + 3 + 1 = 5
    assert_eq!(calc.width(gt), 5);
}

#[test]
fn test_width_nested_binary() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // (1 + 2) * 3
    let one = make_expr(&mut arena, ExprKind::Int(1));
    let two = make_expr(&mut arena, ExprKind::Int(2));
    let three = make_expr(&mut arena, ExprKind::Int(3));

    let add = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: one,
            right: two,
        },
    );
    let mul = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Mul,
            left: add,
            right: three,
        },
    );

    let mut calc = WidthCalculator::new(&arena, &interner);
    // "1 + 2" = 5, " * " = 3, "3" = 1 -> 5 + 3 + 1 = 9
    assert_eq!(calc.width(mul), 9);
}

#[test]
fn test_width_unary_expr() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // -42
    let num = make_expr(&mut arena, ExprKind::Int(42));
    let neg = make_expr(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand: num,
        },
    );

    let mut calc = WidthCalculator::new(&arena, &interner);
    assert_eq!(calc.width(neg), 3); // "-42"
}

#[test]
fn test_width_list_empty() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let list_items = arena.alloc_expr_list_inline(&[]);
    let list = make_expr(&mut arena, ExprKind::List(list_items));

    let mut calc = WidthCalculator::new(&arena, &interner);
    assert_eq!(calc.width(list), 2); // "[]"
}

#[test]
fn test_width_list_with_items() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let one = make_expr(&mut arena, ExprKind::Int(1));
    let two = make_expr(&mut arena, ExprKind::Int(2));
    let three = make_expr(&mut arena, ExprKind::Int(3));
    let list_items = arena.alloc_expr_list_inline(&[one, two, three]);
    let list = make_expr(&mut arena, ExprKind::List(list_items));

    let mut calc = WidthCalculator::new(&arena, &interner);
    // "[1, 2, 3]" = 1 + 1 + 2 + 1 + 2 + 1 + 1 = 9
    assert_eq!(calc.width(list), 9);
}

#[test]
fn test_width_unit() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let unit = make_expr(&mut arena, ExprKind::Unit);
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(unit), 2); // "()"
}

#[test]
fn test_width_self_ref() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let self_ref = make_expr(&mut arena, ExprKind::SelfRef);
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(self_ref), 4); // "self"
}

#[test]
fn test_width_hash_length() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let hash = make_expr(&mut arena, ExprKind::HashLength);
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(hash), 1); // "#"
}

#[test]
fn test_width_none() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let none = make_expr(&mut arena, ExprKind::None);
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(none), 4); // "None"
}

#[test]
fn test_width_some() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let value = make_expr(&mut arena, ExprKind::Int(42));
    let some = make_expr(&mut arena, ExprKind::Some(value));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(some), 8); // "Some(42)"
}

#[test]
fn test_width_ok() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let value = make_expr(&mut arena, ExprKind::Int(1));
    let ok = make_expr(&mut arena, ExprKind::Ok(value));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(ok), 5); // "Ok(1)"
}

#[test]
fn test_width_ok_empty() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let ok = make_expr(&mut arena, ExprKind::Ok(ExprId::INVALID));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(ok), 4); // "Ok()"
}

#[test]
fn test_width_err() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let value = make_expr(&mut arena, ExprKind::Int(1));
    let err = make_expr(&mut arena, ExprKind::Err(value));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(err), 6); // "Err(1)"
}

#[test]
fn test_width_err_empty() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let err = make_expr(&mut arena, ExprKind::Err(ExprId::INVALID));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(err), 5); // "Err()"
}

#[test]
fn test_width_try_postfix() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let value = make_expr(&mut arena, ExprKind::Int(42));
    let try_expr = make_expr(&mut arena, ExprKind::Try(value));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(try_expr), 3); // "42?"
}

#[test]
fn test_width_break() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let brk = make_expr(
        &mut arena,
        ExprKind::Break {
            label: Name::EMPTY,
            value: ExprId::INVALID,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(brk), 5); // "break"
}

#[test]
fn test_width_break_value() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let value = make_expr(&mut arena, ExprKind::Int(42));
    let brk = make_expr(
        &mut arena,
        ExprKind::Break {
            label: Name::EMPTY,
            value,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(brk), 8); // "break 42"
}

#[test]
fn test_width_continue() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let cont = make_expr(
        &mut arena,
        ExprKind::Continue {
            label: Name::EMPTY,
            value: ExprId::INVALID,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(cont), 8); // "continue"
}

#[test]
fn test_width_await() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let value = make_expr(&mut arena, ExprKind::Int(42));
    let await_expr = make_expr(&mut arena, ExprKind::Await(value));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(await_expr), 8); // "42.await"
}

#[test]
fn test_width_assign() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let name = interner.intern("x");
    let target = make_expr(&mut arena, ExprKind::Ident(name));
    let value = make_expr(&mut arena, ExprKind::Int(42));
    let assign = make_expr(&mut arena, ExprKind::Assign { target, value });
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(assign), 6); // "x = 42" = 1 + 3 + 2 = 6
}

#[test]
fn test_width_field_access() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let obj_name = interner.intern("obj");
    let field_name = interner.intern("field");
    let receiver = make_expr(&mut arena, ExprKind::Ident(obj_name));
    let field = make_expr(
        &mut arena,
        ExprKind::Field {
            receiver,
            field: field_name,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(field), 9); // "obj.field"
}

#[test]
fn test_width_index() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let arr_name = interner.intern("arr");
    let receiver = make_expr(&mut arena, ExprKind::Ident(arr_name));
    let index = make_expr(&mut arena, ExprKind::Int(0));
    let index_expr = make_expr(&mut arena, ExprKind::Index { receiver, index });
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(index_expr), 6); // "arr[0]"
}

#[test]
fn test_width_if_simple() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let cond = make_expr(&mut arena, ExprKind::Bool(true));
    let then_branch = make_expr(&mut arena, ExprKind::Int(1));
    let if_expr = make_expr(
        &mut arena,
        ExprKind::If {
            cond,
            then_branch,
            else_branch: ExprId::INVALID,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "if true then 1" = 3 + 4 + 6 + 1 = 14
    assert_eq!(calc.width(if_expr), 14);
}

#[test]
fn test_width_if_else() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let cond = make_expr(&mut arena, ExprKind::Bool(true));
    let then_branch = make_expr(&mut arena, ExprKind::Int(1));
    let else_branch = make_expr(&mut arena, ExprKind::Int(2));
    let if_expr = make_expr(
        &mut arena,
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "if true then 1 else 2" = 3 + 4 + 6 + 1 + 6 + 1 = 21
    assert_eq!(calc.width(if_expr), 21);
}

#[test]
fn test_width_loop() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let body = make_expr(&mut arena, ExprKind::Int(42));
    let loop_expr = make_expr(
        &mut arena,
        ExprKind::Loop {
            label: Name::EMPTY,
            body,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "loop 42" = 4 + 1 + 2 = 7
    assert_eq!(calc.width(loop_expr), 7);
}

#[test]
fn test_width_block_empty() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let block = make_expr(
        &mut arena,
        ExprKind::Block {
            stmts: StmtRange::EMPTY,
            result: ExprId::INVALID,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(block), 2); // "{}"
}

#[test]
fn test_width_block_with_result() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let result = make_expr(&mut arena, ExprKind::Int(42));
    let block = make_expr(
        &mut arena,
        ExprKind::Block {
            stmts: StmtRange::EMPTY,
            result,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "{ 42 }" = 2 + 2 + 2 = 6
    assert_eq!(calc.width(block), 6);
}

#[test]
fn test_width_tuple() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let one = make_expr(&mut arena, ExprKind::Int(1));
    let two = make_expr(&mut arena, ExprKind::Int(2));
    let items = arena.alloc_expr_list_inline(&[one, two]);
    let tuple = make_expr(&mut arena, ExprKind::Tuple(items));
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "(1, 2)" = 1 + 1 + 2 + 1 + 1 = 6
    assert_eq!(calc.width(tuple), 6);
}

#[test]
fn test_width_range() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let start = make_expr(&mut arena, ExprKind::Int(0));
    let end = make_expr(&mut arena, ExprKind::Int(10));
    let range = make_expr(
        &mut arena,
        ExprKind::Range {
            start,
            end,
            step: ExprId::INVALID,
            inclusive: false,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "0..10" = 1 + 2 + 2 = 5
    assert_eq!(calc.width(range), 5);
}

#[test]
fn test_width_range_inclusive() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let start = make_expr(&mut arena, ExprKind::Int(0));
    let end = make_expr(&mut arena, ExprKind::Int(10));
    let range = make_expr(
        &mut arena,
        ExprKind::Range {
            start,
            end,
            step: ExprId::INVALID,
            inclusive: true,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "0..=10" = 1 + 3 + 2 = 6
    assert_eq!(calc.width(range), 6);
}

#[test]
fn test_always_stacked_match() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let scrutinee = make_expr(&mut arena, ExprKind::Int(1));
    let arms = arena.alloc_arms([]);
    let match_expr = make_expr(&mut arena, ExprKind::Match { scrutinee, arms });

    let mut calc = WidthCalculator::new(&arena, &interner);
    assert_eq!(calc.width(match_expr), ALWAYS_STACKED);
}

#[test]
fn test_always_stacked_try() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let result = make_expr(&mut arena, ExprKind::Int(1));
    let stmts = StmtRange::EMPTY;
    let seq_id = arena.alloc_function_seq(FunctionSeq::Try {
        stmts,
        result,
        span: Span::new(0, 1),
    });
    let try_expr = make_expr(&mut arena, ExprKind::FunctionSeq(seq_id));

    let mut calc = WidthCalculator::new(&arena, &interner);
    assert_eq!(calc.width(try_expr), ALWAYS_STACKED);
}

#[test]
fn test_always_stacked_error() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let error = make_expr(&mut arena, ExprKind::Error);
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert_eq!(calc.width(error), ALWAYS_STACKED);
}

#[test]
#[expect(
    clippy::cast_possible_truncation,
    reason = "Test arena indices will never exceed u32::MAX"
)]
fn test_always_stacked_block_with_stmts() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // A block with statements is always stacked
    let value = make_expr(&mut arena, ExprKind::Int(1));
    // Allocate a statement and create a range for it
    let stmt = Stmt::new(StmtKind::Expr(value), Span::new(0, 1));
    let stmt_id = arena.alloc_stmt(stmt);
    let stmts = arena.alloc_stmt_range(stmt_id.index() as u32, 1);
    let block = make_expr(
        &mut arena,
        ExprKind::Block {
            stmts,
            result: ExprId::INVALID,
        },
    );

    let mut calc = WidthCalculator::new(&arena, &interner);
    assert_eq!(calc.width(block), ALWAYS_STACKED);
}

#[test]
fn test_always_stacked_propagation() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // A binary expression containing always-stacked subexpr should propagate
    let scrutinee = make_expr(&mut arena, ExprKind::Int(1));
    let arms = arena.alloc_arms([]);
    let match_expr = make_expr(&mut arena, ExprKind::Match { scrutinee, arms });

    let right = make_expr(&mut arena, ExprKind::Int(2));
    let binary = make_expr(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: match_expr,
            right,
        },
    );

    let mut calc = WidthCalculator::new(&arena, &interner);
    assert_eq!(calc.width(binary), ALWAYS_STACKED);
}

#[test]
fn test_width_caching() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let expr = make_expr(&mut arena, ExprKind::Int(42));
    let mut calc = WidthCalculator::new(&arena, &interner);

    assert!(!calc.is_cached(expr));
    assert_eq!(calc.cache_size(), 0);

    let width = calc.width(expr);
    assert_eq!(width, 2);
    assert!(calc.is_cached(expr));
    assert_eq!(calc.cache_size(), 1);

    // Second call should use cache
    let width2 = calc.width(expr);
    assert_eq!(width2, 2);
    assert_eq!(calc.cache_size(), 1);
}

#[test]
fn test_clear_cache() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let expr = make_expr(&mut arena, ExprKind::Int(42));
    let mut calc = WidthCalculator::new(&arena, &interner);

    let _ = calc.width(expr);
    assert_eq!(calc.cache_size(), 1);

    calc.clear_cache();
    assert_eq!(calc.cache_size(), 0);
    assert!(!calc.is_cached(expr));
}

#[test]
fn test_with_capacity() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();

    let calc = WidthCalculator::with_capacity(&arena, &interner, 100);
    assert_eq!(calc.cache_size(), 0);
}

#[test]
fn test_deeply_nested_binary() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    // Create a deeply nested expression: 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1
    let mut expr = make_expr(&mut arena, ExprKind::Int(1));
    for _ in 0..9 {
        let right = make_expr(&mut arena, ExprKind::Int(1));
        expr = make_expr(
            &mut arena,
            ExprKind::Binary {
                op: BinaryOp::Add,
                left: expr,
                right,
            },
        );
    }

    let mut calc = WidthCalculator::new(&arena, &interner);
    // 10 x "1" = 10, 9 x " + " = 27, total = 37
    assert_eq!(calc.width(expr), 37);
}

#[test]
fn test_width_break_labeled() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let label = interner.intern("outer");
    let brk = make_expr(
        &mut arena,
        ExprKind::Break {
            label,
            value: ExprId::INVALID,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "break:outer" = 5 + 1 + 5 = 11
    assert_eq!(calc.width(brk), 11);
}

#[test]
fn test_width_break_labeled_with_value() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let label = interner.intern("outer");
    let value = make_expr(&mut arena, ExprKind::Int(42));
    let brk = make_expr(&mut arena, ExprKind::Break { label, value });
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "break:outer 42" = 5 + 1 + 5 + 1 + 2 = 14
    assert_eq!(calc.width(brk), 14);
}

#[test]
fn test_width_continue_labeled() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let label = interner.intern("inner");
    let cont = make_expr(
        &mut arena,
        ExprKind::Continue {
            label,
            value: ExprId::INVALID,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "continue:inner" = 8 + 1 + 5 = 14
    assert_eq!(calc.width(cont), 14);
}

#[test]
fn test_width_loop_labeled() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let label = interner.intern("main");
    let body = make_expr(&mut arena, ExprKind::Int(42));
    let loop_expr = make_expr(&mut arena, ExprKind::Loop { label, body });
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "loop:main 42" = 4 + 1 + 4 + 1 + 2 = 12
    assert_eq!(calc.width(loop_expr), 12);
}

#[test]
fn test_width_for_labeled() {
    let mut arena = ExprArena::new();
    let interner = StringInterner::new();

    let label = interner.intern("row");
    let binding_name = interner.intern("x");
    let pattern = arena.alloc_binding_pattern(ori_ir::BindingPattern::Name {
        name: binding_name,
        mutable: ori_ir::Mutability::Mutable,
    });
    let iter = make_expr(&mut arena, ExprKind::Ident(interner.intern("items")));
    let body = make_expr(&mut arena, ExprKind::Ident(interner.intern("x")));
    let for_expr = make_expr(
        &mut arena,
        ExprKind::For {
            label,
            pattern,
            iter,
            guard: ExprId::INVALID,
            body,
            is_yield: false,
        },
    );
    let mut calc = WidthCalculator::new(&arena, &interner);

    // "for:row x in items do x" = 3 + 1 + 3 + 1 + 1 + 4 + 5 + 4 + 1 = 23
    assert_eq!(calc.width(for_expr), 23);
}

// Structural enforcement: no wildcard arms in the dispatch table

/// Verify that `calculate_width()` has no wildcard match arm — the third of
/// the three parallel `ExprKind` dispatch tables the `formatter/broken/mod.rs`
/// doc-comment invariant names (`emit_broken` / `emit_inline` /
/// `calculate_width`); mirrors `broken_dispatch_has_no_wildcard` /
/// `inline_dispatch_has_no_wildcard` for this sibling table.
#[test]
fn calculate_width_dispatch_has_no_wildcard() {
    let source = include_str!("mod.rs");
    assert!(
        !has_wildcard_match_arm(source),
        "calculate_width() must not have a wildcard `_ =>` arm — \
         every ExprKind variant must be handled explicitly"
    );
}
