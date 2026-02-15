use super::*;
use ori_ir::ast::{BinaryOp, Expr};
use ori_ir::{ExprKind, StringInterner};
use ori_types::Idx;

/// Create a minimal `TypeCheckResult` for testing.
fn test_type_result(expr_types: Vec<Idx>) -> TypeCheckResult {
    let mut typed = TypedModule::new();
    for idx in expr_types {
        typed.expr_types.push(idx);
    }
    TypeCheckResult::ok(typed)
}

/// Create a shared interner for testing.
fn test_interner() -> StringInterner {
    StringInterner::new()
}

#[test]
fn lower_int_literal() {
    let mut arena = ExprArena::new();
    let root = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(0, 2)));

    let type_result = test_type_result(vec![Idx::INT]);
    let interner = test_interner();

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());
    assert_eq!(*result.arena.kind(result.root), CanExpr::Int(42));
    assert_eq!(result.arena.ty(result.root), TypeId::INT);
}

#[test]
fn lower_bool_literal() {
    let mut arena = ExprArena::new();
    let root = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(0, 4)));

    let type_result = test_type_result(vec![Idx::BOOL]);
    let interner = test_interner();

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert_eq!(*result.arena.kind(result.root), CanExpr::Bool(true));
    assert_eq!(result.arena.ty(result.root), TypeId::BOOL);
}

#[test]
fn lower_string_literal() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let name = interner.intern("hello");
    let root = arena.alloc_expr(Expr::new(ExprKind::String(name), Span::new(0, 7)));

    let type_result = test_type_result(vec![Idx::STR]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert_eq!(*result.arena.kind(result.root), CanExpr::Str(name));
    assert_eq!(result.arena.ty(result.root), TypeId::STR);
}

#[test]
fn lower_unit() {
    let mut arena = ExprArena::new();
    let root = arena.alloc_expr(Expr::new(ExprKind::Unit, Span::DUMMY));

    let type_result = test_type_result(vec![Idx::UNIT]);
    let interner = test_interner();

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert_eq!(*result.arena.kind(result.root), CanExpr::Unit);
    assert_eq!(result.arena.ty(result.root), TypeId::UNIT);
}

#[test]
fn lower_binary_add() {
    // 1 + 2 with two literals gets constant-folded to Constant(3).
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
    let interner = test_interner();

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    // Constant folding: 1 + 2 → Constant(3).
    match result.arena.kind(result.root) {
        CanExpr::Constant(cid) => {
            assert_eq!(
                *result.constants.get(*cid),
                ori_ir::canon::ConstValue::Int(3)
            );
        }
        other => panic!("expected Constant(3), got {other:?}"),
    }
}

#[test]
fn lower_binary_add_runtime() {
    // x + 1 with a runtime variable stays as Binary (not folded).
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let name_x = interner.intern("x");

    let left = arena.alloc_expr(Expr::new(ExprKind::Ident(name_x), Span::new(0, 1)));
    let right = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(4, 5)));
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
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    // Runtime operand: stays as Binary node.
    match result.arena.kind(result.root) {
        CanExpr::Binary { op, .. } => {
            assert_eq!(*op, BinaryOp::Add);
        }
        other => panic!("expected Binary, got {other:?}"),
    }
}

#[test]
fn lower_ok_with_value() {
    let mut arena = ExprArena::new();
    let inner = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(3, 5)));
    let root = arena.alloc_expr(Expr::new(ExprKind::Ok(inner), Span::new(0, 6)));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT]);
    let interner = test_interner();

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Ok(inner_id) => {
            assert!(inner_id.is_valid());
            assert_eq!(*result.arena.kind(*inner_id), CanExpr::Int(42));
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[test]
fn lower_break_no_value() {
    let mut arena = ExprArena::new();
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Break {
            label: Name::EMPTY,
            value: ExprId::INVALID,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::NEVER]);
    let interner = test_interner();

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Break { value: val, .. } => assert!(!val.is_valid()),
        other => panic!("expected Break, got {other:?}"),
    }
}

#[test]
fn lower_list_literal() {
    let mut arena = ExprArena::new();
    let e1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(1, 2)));
    let e2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(4, 5)));
    let e3 = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::new(7, 8)));
    let elems = arena.alloc_expr_list([e1, e2, e3]);
    let root = arena.alloc_expr(Expr::new(ExprKind::List(elems), Span::new(0, 9)));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT]);
    let interner = test_interner();

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::List(range) => {
            let items = result.arena.get_expr_list(*range);
            assert_eq!(items.len(), 3);
            assert_eq!(*result.arena.kind(items[0]), CanExpr::Int(1));
            assert_eq!(*result.arena.kind(items[1]), CanExpr::Int(2));
            assert_eq!(*result.arena.kind(items[2]), CanExpr::Int(3));
        }
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn lower_template_full() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let name = interner.intern("hello world");
    let root = arena.alloc_expr(Expr::new(ExprKind::TemplateFull(name), Span::new(0, 13)));

    let type_result = test_type_result(vec![Idx::STR]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    // TemplateFull desugars to Str.
    assert_eq!(*result.arena.kind(result.root), CanExpr::Str(name));
}

#[test]
fn lower_invalid_root_returns_empty() {
    let arena = ExprArena::new();
    let type_result = test_type_result(vec![]);
    let interner = test_interner();

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, ExprId::INVALID, &interner);
    assert!(!result.root.is_valid());
    assert!(result.arena.is_empty());
}

#[test]
fn lower_call_positional() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let func_name = interner.intern("foo");

    let func = arena.alloc_expr(Expr::new(ExprKind::Ident(func_name), Span::new(0, 3)));
    let arg = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(4, 6)));
    let args = arena.alloc_expr_list([arg]);
    let root = arena.alloc_expr(Expr::new(ExprKind::Call { func, args }, Span::new(0, 7)));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Call { func, args } => {
            assert_eq!(*result.arena.kind(*func), CanExpr::Ident(func_name));
            let arg_list = result.arena.get_expr_list(*args);
            assert_eq!(arg_list.len(), 1);
            assert_eq!(*result.arena.kind(arg_list[0]), CanExpr::Int(42));
        }
        other => panic!("expected Call, got {other:?}"),
    }
}
