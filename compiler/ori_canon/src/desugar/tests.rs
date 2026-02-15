use super::*;
use crate::lower::lower;
use ori_ir::ast::{CallArg, Expr, TemplatePart};
use ori_ir::{ExprArena, ExprKind, SharedInterner, Span};
use ori_types::{Idx, TypeCheckResult, TypedModule};

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

#[test]
fn desugar_call_named_source_order_fallback() {
    // When no function signature is available, args stay in source order.
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let func_name = interner.intern("unknown_fn");
    let arg_a_name = interner.intern("a");
    let arg_b_name = interner.intern("b");

    let func = arena.alloc_expr(Expr::new(ExprKind::Ident(func_name), Span::new(0, 3)));
    let val1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(4, 5)));
    let val2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(8, 9)));

    let args = arena.alloc_call_args([
        CallArg {
            name: Some(arg_b_name),
            value: val2,
            is_spread: false,
            span: Span::new(7, 10),
        },
        CallArg {
            name: Some(arg_a_name),
            value: val1,
            is_spread: false,
            span: Span::new(4, 6),
        },
    ]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::CallNamed { func, args },
        Span::new(0, 11),
    ));

    // No function sig → source order (b=2, a=1).
    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    match result.arena.kind(result.root) {
        CanExpr::Call { args, .. } => {
            let arg_list = result.arena.get_expr_list(*args);
            assert_eq!(arg_list.len(), 2);
            // Source order preserved: b=2 first, a=1 second.
            assert_eq!(*result.arena.kind(arg_list[0]), CanExpr::Int(2));
            assert_eq!(*result.arena.kind(arg_list[1]), CanExpr::Int(1));
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn desugar_template_literal_simple() {
    // `hello {name}!` → "hello".concat(name.to_str()).concat("!")
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let head = interner.intern("hello ");
    let tail = interner.intern("!");
    let var_name = interner.intern("name");

    let expr = arena.alloc_expr(Expr::new(ExprKind::Ident(var_name), Span::new(8, 12)));

    let parts = arena.alloc_template_parts([TemplatePart {
        expr,
        format_spec: Name::EMPTY,
        text_after: tail,
    }]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::TemplateLiteral { head, parts },
        Span::new(0, 14),
    ));

    // expr_types: [0]=Ident(name):str, [1]=TemplateLiteral:str
    let type_result = test_type_result(vec![Idx::STR, Idx::STR]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    // The root should be a concat chain. Since `name` is already str,
    // no to_str wrapping needed. Result is:
    // "hello ".concat(name).concat("!")
    // The final concat("!") is the root.
    match result.arena.kind(result.root) {
        CanExpr::MethodCall { method, .. } => {
            let concat = interner.intern("concat");
            assert_eq!(*method, concat);
        }
        other => panic!("expected MethodCall(concat), got {other:?}"),
    }
}

#[test]
fn desugar_list_with_spread_simple() {
    // [1, ...xs, 2] → [1].concat(xs).concat([2])
    let mut arena = ExprArena::new();
    let interner = test_interner();

    let xs_name = interner.intern("xs");

    let e1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(1, 2)));
    let xs = arena.alloc_expr(Expr::new(ExprKind::Ident(xs_name), Span::new(7, 9)));
    let e2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(11, 12)));

    let elements = arena.alloc_list_elements([
        ori_ir::ListElement::Expr {
            expr: e1,
            span: Span::new(1, 2),
        },
        ori_ir::ListElement::Spread {
            expr: xs,
            span: Span::new(4, 9),
        },
        ori_ir::ListElement::Expr {
            expr: e2,
            span: Span::new(11, 12),
        },
    ]);

    let root = arena.alloc_expr(Expr::new(
        ExprKind::ListWithSpread(elements),
        Span::new(0, 13),
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    // Root should be: [1].concat(xs).concat([2])
    // That's a chain of MethodCall(concat).
    match result.arena.kind(result.root) {
        CanExpr::MethodCall { method, .. } => {
            let concat = interner.intern("concat");
            assert_eq!(*method, concat);
        }
        other => panic!("expected MethodCall(concat), got {other:?}"),
    }
}
