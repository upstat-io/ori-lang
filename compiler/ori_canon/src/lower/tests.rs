use super::*;
use ori_ir::ast::{BinaryOp, Expr};
use ori_ir::canon::ConstEvalProblemKind;
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

fn test_const_def(name: Name, value: ExprId) -> ori_ir::ConstDef {
    ori_ir::ConstDef {
        name,
        ty: None,
        value,
        span: Span::DUMMY,
        visibility: ori_ir::Visibility::Public,
        target_attr: None,
        cfg_attr: None,
    }
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
fn enum_variant_shadows_same_named_type_reference() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let error = interner.intern("Error");
    let log_level = interner.intern("LogLevel");
    let root = arena.alloc_expr(Expr::new(ExprKind::Ident(error), Span::DUMMY));

    let mut pool = ori_types::Pool::new();
    let enum_type = pool.enum_type(
        log_level,
        &[ori_types::EnumVariant {
            name: error,
            field_types: vec![Idx::STR],
        }],
    );
    let constructor_type = pool.function(&[Idx::STR], enum_type);

    let mut typed = TypedModule::new();
    typed.expr_types.push(constructor_type);
    typed.types.push(ori_types::TypeEntry {
        name: error,
        idx: Idx::from_raw(64),
        kind: ori_types::TypeKind::Struct(ori_types::StructDef {
            fields: Vec::new(),
            category: ori_types::ValueCategory::default(),
        }),
        span: Span::DUMMY,
        type_params: Vec::new(),
        visibility: ori_types::Visibility::Public,
        merkle_hash: 0,
        repr: None,
        burden: None,
    });

    let result = lower(&arena, &TypeCheckResult::ok(typed), &pool, root, &interner);
    assert_eq!(
        *result.arena.kind(result.root),
        CanExpr::Ident(error),
        "the type-checker-selected enum variant must outrank a same-named type"
    );
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
fn lower_tailless_try_wraps_void_block_in_ok() {
    let mut arena = ExprArena::new();
    let stmts = arena.alloc_stmt_range(0, 0);
    let seq = arena.alloc_function_seq(ori_ir::FunctionSeq::Try {
        stmts,
        result: ExprId::INVALID,
        span: Span::DUMMY,
    });
    let root = arena.alloc_expr(Expr::new(ExprKind::FunctionSeq(seq), Span::DUMMY));

    let mut pool = ori_types::Pool::new();
    let result_unit_str = pool.result(Idx::UNIT, Idx::STR);
    let type_result = test_type_result(vec![result_unit_str]);
    let result = lower(&arena, &type_result, &pool, root, &test_interner());

    let CanExpr::Ok(block) = *result.arena.kind(result.root) else {
        panic!("tail-less Result try must synthesize Ok(void)")
    };
    assert_eq!(
        result.arena.ty(result.root),
        TypeId::from_raw(result_unit_str.raw())
    );
    assert_eq!(result.arena.ty(block), TypeId::UNIT);
    assert!(matches!(
        result.arena.kind(block),
        CanExpr::Block { result, .. } if !result.is_valid()
    ));
}

#[test]
fn lower_bare_option_try_tail_wraps_block_in_some() {
    let mut arena = ExprArena::new();
    let tail = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::DUMMY));
    let stmts = arena.alloc_stmt_range(0, 0);
    let seq = arena.alloc_function_seq(ori_ir::FunctionSeq::Try {
        stmts,
        result: tail,
        span: Span::DUMMY,
    });
    let root = arena.alloc_expr(Expr::new(ExprKind::FunctionSeq(seq), Span::DUMMY));

    let mut pool = ori_types::Pool::new();
    let option_int = pool.option(Idx::INT);
    let type_result = test_type_result(vec![Idx::INT, option_int]);
    let result = lower(&arena, &type_result, &pool, root, &test_interner());

    let CanExpr::Some(block) = *result.arena.kind(result.root) else {
        panic!("bare Option try tail must synthesize Some(payload)")
    };
    assert_eq!(result.arena.ty(block), TypeId::INT);
    assert!(matches!(result.arena.kind(block), CanExpr::Block { .. }));
}

#[test]
fn lower_wrapped_try_tail_is_not_double_wrapped() {
    let mut arena = ExprArena::new();
    let value = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::DUMMY));
    let wrapped = arena.alloc_expr(Expr::new(ExprKind::Ok(value), Span::DUMMY));
    let stmts = arena.alloc_stmt_range(0, 0);
    let seq = arena.alloc_function_seq(ori_ir::FunctionSeq::Try {
        stmts,
        result: wrapped,
        span: Span::DUMMY,
    });
    let root = arena.alloc_expr(Expr::new(ExprKind::FunctionSeq(seq), Span::DUMMY));

    let mut pool = ori_types::Pool::new();
    let result_int_str = pool.result(Idx::INT, Idx::STR);
    let type_result = test_type_result(vec![Idx::INT, result_int_str, result_int_str]);
    let result = lower(&arena, &type_result, &pool, root, &test_interner());

    let CanExpr::Block { result: tail, .. } = *result.arena.kind(result.root) else {
        panic!("already wrapped try tail must remain a single carrier")
    };
    assert!(matches!(result.arena.kind(tail), CanExpr::Ok(_)));
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
fn lower_while_desugars_to_loop_with_break_guard() {
    // `while c do body` canonicalizes to a loop with an `if !c then break`
    // guard, eliminating `ExprKind::While` from backend input.
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let cond_name = interner.intern("c");

    let cond = arena.alloc_expr(Expr::new(ExprKind::Ident(cond_name), Span::new(6, 7)));
    let body = arena.alloc_expr(Expr::new(ExprKind::Unit, Span::new(11, 13)));
    let root = arena.alloc_expr(Expr::new(
        ExprKind::While {
            label: Name::EMPTY,
            cond,
            body,
        },
        Span::new(0, 13),
    ));

    // expr_types: [0]=Ident(c):bool, [1]=Unit body, [2]=While:void.
    let type_result = test_type_result(vec![Idx::BOOL, Idx::UNIT, Idx::UNIT]);

    let pool = ori_types::Pool::new();
    let result = lower(&arena, &type_result, &pool, root, &interner);
    assert!(result.root.is_valid());

    // Root is a Loop whose body is the guard-block.
    let loop_body = match result.arena.kind(result.root) {
        CanExpr::Loop { body, label } => {
            assert_eq!(*label, Name::EMPTY);
            *body
        }
        other => panic!("expected Loop, got {other:?}"),
    };
    assert_eq!(result.arena.ty(result.root), TypeId::UNIT);

    // Loop body is a Block { stmts: [if !c then break], result: body }.
    let (stmts, block_result) = match result.arena.kind(loop_body) {
        CanExpr::Block { stmts, result } => (*stmts, *result),
        other => panic!("expected Block loop body, got {other:?}"),
    };
    let stmt_list = result.arena.get_expr_list(stmts);
    assert_eq!(stmt_list.len(), 1, "single break-guard statement");

    // The guard is `if (!c) then break`.
    let guard = stmt_list[0];
    match result.arena.kind(guard) {
        CanExpr::If {
            cond: g_cond,
            then_branch,
            else_branch,
        } => {
            // Condition is `!c` (Unary Not over the lowered cond).
            match result.arena.kind(*g_cond) {
                CanExpr::Unary {
                    op: ori_ir::UnaryOp::Not,
                    ..
                } => {}
                other => panic!("expected Unary(Not) guard condition, got {other:?}"),
            }
            // Then-branch is a bare break (no value, no else).
            assert!(
                matches!(result.arena.kind(*then_branch), CanExpr::Break { value, .. } if !value.is_valid()),
                "guard then-branch is a valueless break"
            );
            assert!(!else_branch.is_valid(), "while-guard has no else branch");
        }
        other => panic!("expected If guard, got {other:?}"),
    }

    // Block result is the lowered loop body (Unit here).
    assert_eq!(*result.arena.kind(block_result), CanExpr::Unit);
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

#[test]
fn lower_index_preserves_selected_method_producer_handle() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let receiver_name = interner.intern("value");
    let receiver = arena.alloc_expr(Expr::new(ExprKind::Ident(receiver_name), Span::DUMMY));
    let index = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::DUMMY));
    let root = arena.alloc_expr(Expr::new(ExprKind::Index { receiver, index }, Span::DUMMY));
    let producer = ori_ir::canon::MethodProducerId::new(0);
    let mut typed = TypedModule::new();
    typed.expr_types.extend([Idx::INT, Idx::INT, Idx::STR]);
    typed
        .index_dispatch_map
        .insert(root, ori_ir::canon::IndexDispatch::Selected(producer));
    let type_result = TypeCheckResult::ok(typed);

    let result = lower(
        &arena,
        &type_result,
        &ori_types::Pool::new(),
        root,
        &interner,
    );

    assert!(matches!(
        result.arena.kind(result.root),
        CanExpr::Index {
            dispatch: ori_ir::canon::IndexDispatch::Selected(selected),
            ..
        } if *selected == producer
    ));
}

#[test]
fn lower_range_eager_map_materializes_adapter_and_collect() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let range_name = interner.intern("r");
    let transform_name = interner.intern("transform");
    let map_name = interner.intern("map");
    let iter_name = interner.intern("iter");
    let collect_name = interner.intern("collect");

    let receiver = arena.alloc_expr(Expr::new(ExprKind::Ident(range_name), Span::DUMMY));
    let transform = arena.alloc_expr(Expr::new(ExprKind::Ident(transform_name), Span::DUMMY));
    let args = arena.alloc_expr_list([transform]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::MethodCall {
            receiver,
            method: map_name,
            args,
        },
        Span::DUMMY,
    ));

    let mut pool = ori_types::Pool::new();
    let range_int = pool.range(Idx::INT);
    let transform_ty = pool.function(&[Idx::INT], Idx::INT);
    let iter_int = pool.double_ended_iterator(Idx::INT);
    let list_int = pool.list(Idx::INT);

    let mut typed = TypedModule::new();
    typed.expr_types = vec![range_int, transform_ty, list_int];
    typed.iter_route_map.insert(
        root,
        ori_types::IterMethodRoute {
            iter_ty: Some(iter_int),
            adapter_ty: Some(iter_int),
            collect_ty: None,
        },
    );

    let result = lower(&arena, &TypeCheckResult::ok(typed), &pool, root, &interner);
    let CanExpr::MethodCall {
        receiver: map_call,
        method,
        args,
    } = *result.arena.kind(result.root)
    else {
        panic!("Range.map route should end in collect")
    };
    assert_eq!(method, collect_name);
    assert!(result.arena.get_expr_list(args).is_empty());
    assert_eq!(
        result.arena.ty(result.root),
        TypeId::from_raw(list_int.raw())
    );

    let CanExpr::MethodCall {
        receiver: iter_call,
        method,
        ..
    } = *result.arena.kind(map_call)
    else {
        panic!("collect receiver should be the map adapter")
    };
    assert_eq!(method, map_name);
    assert_eq!(result.arena.ty(map_call), TypeId::from_raw(iter_int.raw()));

    let CanExpr::MethodCall {
        receiver: lowered_range,
        method,
        args,
    } = *result.arena.kind(iter_call)
    else {
        panic!("map receiver should be the materialized Range iterator")
    };
    assert_eq!(method, iter_name);
    assert!(result.arena.get_expr_list(args).is_empty());
    assert_eq!(result.arena.ty(iter_call), TypeId::from_raw(iter_int.raw()));
    assert_eq!(
        *result.arena.kind(lowered_range),
        CanExpr::Ident(range_name)
    );
}

#[test]
fn lower_typed_set_collect_uses_protocol_without_rematerializing_iterator() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let receiver_name = interner.intern("items");
    let collect_name = interner.intern("collect");
    let collect_set_name = interner.intern("__collect_set");

    let receiver = arena.alloc_expr(Expr::new(ExprKind::Ident(receiver_name), Span::DUMMY));
    let args = arena.alloc_expr_list([]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::MethodCall {
            receiver,
            method: collect_name,
            args,
        },
        Span::DUMMY,
    ));

    let mut pool = ori_types::Pool::new();
    let iter_int = pool.iterator(Idx::INT);
    let set_int = pool.set(Idx::INT);
    let mut typed = TypedModule::new();
    typed.expr_types = vec![iter_int, set_int];
    typed.iter_route_map.insert(
        root,
        ori_types::IterMethodRoute {
            iter_ty: None,
            adapter_ty: None,
            collect_ty: Some(set_int),
        },
    );

    let result = lower(&arena, &TypeCheckResult::ok(typed), &pool, root, &interner);
    let CanExpr::MethodCall {
        receiver, method, ..
    } = *result.arena.kind(result.root)
    else {
        panic!("typed Set collect should remain a method call")
    };
    assert_eq!(method, collect_set_name);
    assert_eq!(*result.arena.kind(receiver), CanExpr::Ident(receiver_name));
}

#[test]
fn lower_user_collect_returning_set_preserves_selected_method_identity() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let receiver_name = interner.intern("collector");
    let collector_name = interner.intern("Collector");
    let collect_name = interner.intern("collect");

    let receiver = arena.alloc_expr(Expr::new(ExprKind::Ident(receiver_name), Span::DUMMY));
    let args = arena.alloc_expr_list([]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::MethodCall {
            receiver,
            method: collect_name,
            args,
        },
        Span::DUMMY,
    ));

    let mut pool = ori_types::Pool::new();
    let collector_ty = pool.struct_type(collector_name, &[]);
    let set_int = pool.set(Idx::INT);
    let typed = test_type_result(vec![collector_ty, set_int]);

    let result = lower(&arena, &typed, &pool, root, &interner);
    let CanExpr::MethodCall { method, .. } = *result.arena.kind(result.root) else {
        panic!("user collect call should remain a method call")
    };
    assert_eq!(
        method, collect_name,
        "a Set-shaped return cannot turn an unrelated user method into the iterator protocol"
    );
}

#[test]
fn module_constants_evaluate_in_dependency_order_and_freeze_uses() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let base = interner.intern("base");
    let derived = interner.intern("derived");
    let base_value = arena.alloc_expr(Expr::new(ExprKind::Int(20), Span::DUMMY));
    let base_ref = arena.alloc_expr(Expr::new(ExprKind::Const(base), Span::DUMMY));
    let ten = arena.alloc_expr(Expr::new(ExprKind::Int(10), Span::DUMMY));
    let derived_value = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: base_ref,
            right: ten,
        },
        Span::DUMMY,
    ));
    let use_derived = arena.alloc_expr(Expr::new(ExprKind::Const(derived), Span::DUMMY));
    let module = Module {
        // Deliberately reverse source/dependency order.
        consts: vec![
            test_const_def(derived, derived_value),
            test_const_def(base, base_value),
        ],
        ..Module::default()
    };
    let type_result = test_type_result(vec![Idx::INT; 5]);
    let pool = ori_types::Pool::new();
    let mut lowerer = Lowerer::new(&arena, &type_result.typed, &pool, &interner);

    let exported = super::named_constants::lower_named_constants(&mut lowerer, &module, &[]);
    assert!(lowerer.const_problems.is_empty());
    assert_eq!(
        exported
            .iter()
            .find(|constant| constant.name == derived)
            .map(|constant| &constant.value),
        Some(&ConstValue::Int(30))
    );

    let frozen = lowerer.lower_expr(use_derived);
    let CanExpr::Constant(id) = *lowerer.arena.kind(frozen) else {
        panic!("module constant use must freeze to CanExpr::Constant")
    };
    assert_eq!(*lowerer.constants.get(id), ConstValue::Int(30));
}

#[test]
fn local_constant_shadows_same_named_selected_import() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let value_name = interner.intern("value");
    let local_value = arena.alloc_expr(Expr::new(ExprKind::Int(7), Span::DUMMY));
    let use_value = arena.alloc_expr(Expr::new(ExprKind::Const(value_name), Span::DUMMY));
    let module = Module {
        consts: vec![test_const_def(value_name, local_value)],
        ..Module::default()
    };
    let imported = [NamedConstValue {
        name: value_name,
        value: ConstValue::Int(30),
    }];
    let type_result = test_type_result(vec![Idx::INT; 2]);
    let pool = ori_types::Pool::new();
    let mut lowerer = Lowerer::new(&arena, &type_result.typed, &pool, &interner);

    super::named_constants::lower_named_constants(&mut lowerer, &module, &imported);
    let frozen = lowerer.lower_expr(use_value);
    let CanExpr::Constant(id) = *lowerer.arena.kind(frozen) else {
        panic!("shadowing local constant must freeze")
    };
    assert_eq!(*lowerer.constants.get(id), ConstValue::Int(7));
}

#[test]
fn module_constant_cycle_is_a_structured_problem() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let a = interner.intern("a");
    let b = interner.intern("b");
    let a_value = arena.alloc_expr(Expr::new(ExprKind::Const(b), Span::DUMMY));
    let b_value = arena.alloc_expr(Expr::new(ExprKind::Const(a), Span::DUMMY));
    let module = Module {
        consts: vec![test_const_def(a, a_value), test_const_def(b, b_value)],
        ..Module::default()
    };
    let type_result = test_type_result(vec![Idx::INT; 2]);
    let pool = ori_types::Pool::new();
    let mut lowerer = Lowerer::new(&arena, &type_result.typed, &pool, &interner);

    let exported = super::named_constants::lower_named_constants(&mut lowerer, &module, &[]);
    assert!(exported.is_empty());
    assert!(lowerer.const_problems.iter().any(|problem| matches!(
        problem.kind,
        ConstEvalProblemKind::CircularDependency { .. }
    )));
}

#[test]
fn module_constant_dependency_failure_marks_dependent_unavailable() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let source = interner.intern("source");
    let dependent = interner.intern("dependent");
    let one = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
    let items = arena.alloc_expr_list([one]);
    let unsupported = arena.alloc_expr(Expr::new(ExprKind::List(items), Span::DUMMY));
    let source_ref = arena.alloc_expr(Expr::new(ExprKind::Const(source), Span::DUMMY));
    let module = Module {
        consts: vec![
            test_const_def(source, unsupported),
            test_const_def(dependent, source_ref),
        ],
        ..Module::default()
    };
    let mut pool = ori_types::Pool::new();
    let list_int = pool.list(Idx::INT);
    let type_result = test_type_result(vec![Idx::INT, list_int, Idx::INT]);
    let mut lowerer = Lowerer::new(&arena, &type_result.typed, &pool, &interner);

    let exported = super::named_constants::lower_named_constants(&mut lowerer, &module, &[]);

    assert!(exported.is_empty());
    assert!(lowerer.const_problems.iter().any(|problem| {
        problem.name == dependent
            && matches!(
                problem.kind,
                ConstEvalProblemKind::UnresolvedReference { reference }
                    if reference == source
            )
    }));
}

#[test]
fn composite_module_constant_reports_unsupported_instead_of_unit() {
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let values = interner.intern("values");
    let one = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
    let items = arena.alloc_expr_list([one]);
    let list = arena.alloc_expr(Expr::new(ExprKind::List(items), Span::DUMMY));
    let module = Module {
        consts: vec![test_const_def(values, list)],
        ..Module::default()
    };
    let mut pool = ori_types::Pool::new();
    let list_int = pool.list(Idx::INT);
    let type_result = test_type_result(vec![Idx::INT, list_int]);
    let mut lowerer = Lowerer::new(&arena, &type_result.typed, &pool, &interner);

    let exported = super::named_constants::lower_named_constants(&mut lowerer, &module, &[]);
    assert!(exported.is_empty());
    assert!(lowerer.const_problems.iter().any(|problem| matches!(
        problem.kind,
        ConstEvalProblemKind::UnsupportedExpression {
            form: "composite value"
        }
    )));
    assert!(
        !matches!(lowerer.named_constants.get(&values), Some(ConstValue::Unit)),
        "unsupported composite constants must never default to unit"
    );
}
