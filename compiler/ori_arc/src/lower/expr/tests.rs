use rustc_hash::FxHashMap;

use ori_ir::canon::{
    CanArena, CanNamedExpr, CanNode, CanParam, CanonResult, GenericConstValue, MonoConstBinding,
};
use ori_ir::{FunctionExpKind, Name, Span, StringInterner, TypeId};
use ori_types::Idx;
use ori_types::Pool;

use crate::ir::{ArcInstr, ArcTerminator, ArcValue, LitValue, PrimOp};

use super::super::{lower_function_can, ArcLoweringInput, ArcProblem};

fn lower_single_expr_with_bindings(
    canon: &CanonResult,
    body: ori_ir::canon::CanId,
    ty: Idx,
    interner: &StringInterner,
    const_bindings: Option<&[MonoConstBinding]>,
) -> (crate::ir::ArcFunction, Vec<ArcProblem>) {
    let pool = Pool::new();
    let mut problems = Vec::new();
    let name = interner.intern("test_function");
    let (func, _lambdas) = lower_function_can(
        ArcLoweringInput {
            name,
            params: &[],
            return_type: ty,
            body,
            canon,
            interner,
            pool: &pool,
            type_subst: None,
            const_bindings,
            is_fbip: false,
        },
        &mut problems,
    );
    (func, problems)
}

/// Helper: create a lowerer with a single canonical expression body.
fn lower_single_expr(
    canon: &CanonResult,
    body: ori_ir::canon::CanId,
    ty: Idx,
) -> crate::ir::ArcFunction {
    let interner = StringInterner::new();
    let (func, problems) = lower_single_expr_with_bindings(canon, body, ty, &interner, None);
    assert!(problems.is_empty(), "unexpected problems: {problems:?}");
    func
}

fn make_canon(kind: ori_ir::canon::CanExpr, ty: Idx) -> (CanArena, CanonResult) {
    let mut arena = CanArena::with_capacity(100);
    let node = CanNode::new(kind, Span::new(0, 10), TypeId::from_raw(ty.raw()));
    let body = arena.push(node);
    let canon = CanonResult::new(arena, body);
    // Reborrow from canon
    (CanArena::with_capacity(0), canon)
}

#[test]
fn lower_int_literal() {
    let (_, canon) = make_canon(ori_ir::canon::CanExpr::Int(42), Idx::INT);
    let body = canon.root;
    let func = lower_single_expr(&canon, body, Idx::INT);
    assert_eq!(func.blocks.len(), 1);
    assert_eq!(func.blocks[0].body.len(), 1);

    if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
        assert_eq!(*value, ArcValue::Literal(LitValue::Int(42)));
    } else {
        panic!("expected Let instruction");
    }
    assert!(matches!(
        func.blocks[0].terminator,
        ArcTerminator::Return { .. }
    ));
}

#[test]
fn lower_bool_literal() {
    let (_, canon) = make_canon(ori_ir::canon::CanExpr::Bool(true), Idx::BOOL);
    let body = canon.root;
    let func = lower_single_expr(&canon, body, Idx::BOOL);
    if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
        assert_eq!(*value, ArcValue::Literal(LitValue::Bool(true)));
    } else {
        panic!("expected Let");
    }
}

#[test]
fn lower_unit_literal() {
    let (_, canon) = make_canon(ori_ir::canon::CanExpr::Unit, Idx::UNIT);
    let body = canon.root;
    let func = lower_single_expr(&canon, body, Idx::UNIT);
    if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
        assert_eq!(*value, ArcValue::Literal(LitValue::Unit));
    } else {
        panic!("expected Let");
    }
}

#[test]
fn lower_constant_pool_value() {
    use ori_ir::canon::{ConstValue, ConstantPool};

    let mut arena = CanArena::with_capacity(100);
    let mut constants = ConstantPool::new();
    let cid = constants.intern(ConstValue::Int(99));
    let node = CanNode::new(
        ori_ir::canon::CanExpr::Constant(cid),
        Span::new(0, 5),
        TypeId::from_raw(Idx::INT.raw()),
    );
    let body = arena.push(node);
    let canon = CanonResult {
        constants,
        ..CanonResult::new(arena, body)
    };

    let func = lower_single_expr(&canon, body, Idx::INT);
    if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
        assert_eq!(*value, ArcValue::Literal(LitValue::Int(99)));
    } else {
        panic!("expected Let with constant value");
    }
}

#[test]
fn lower_named_const_uses_exact_mono_binding() {
    let interner = StringInterner::new();
    let const_name = interner.intern("N");
    let (_, canon) = make_canon(ori_ir::canon::CanExpr::Const(const_name), Idx::INT);
    let bindings = [MonoConstBinding {
        name: const_name,
        value: GenericConstValue::Int(7),
    }];

    let (func, problems) =
        lower_single_expr_with_bindings(&canon, canon.root, Idx::INT, &interner, Some(&bindings));

    assert!(problems.is_empty(), "unexpected problems: {problems:?}");
    assert!(matches!(
        &func.blocks[0].body[0],
        ArcInstr::Let {
            value: ArcValue::Literal(LitValue::Int(7)),
            ..
        }
    ));
}

#[test]
fn lower_generic_const_ident_uses_exact_mono_binding() {
    let interner = StringInterner::new();
    let const_name = interner.intern("N");
    let (_, canon) = make_canon(ori_ir::canon::CanExpr::Ident(const_name), Idx::INT);
    let bindings = [MonoConstBinding {
        name: const_name,
        value: GenericConstValue::Int(7),
    }];

    let (func, problems) =
        lower_single_expr_with_bindings(&canon, canon.root, Idx::INT, &interner, Some(&bindings));

    assert!(problems.is_empty(), "unexpected problems: {problems:?}");
    assert!(matches!(
        &func.blocks[0].body[0],
        ArcInstr::Let {
            value: ArcValue::Literal(LitValue::Int(7)),
            ..
        }
    ));
}

#[test]
fn lexical_ident_shadows_same_named_mono_const_binding() {
    let interner = StringInterner::new();
    let name = interner.intern("N");
    let (_, canon) = make_canon(ori_ir::canon::CanExpr::Ident(name), Idx::INT);
    let bindings = [MonoConstBinding {
        name,
        value: GenericConstValue::Int(7),
    }];
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _lambdas) = lower_function_can(
        ArcLoweringInput {
            name: interner.intern("shadow"),
            params: &[(name, Idx::INT)],
            return_type: Idx::INT,
            body: canon.root,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: Some(&bindings),
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty(), "unexpected problems: {problems:?}");
    assert!(matches!(
        &func.blocks[0].body[0],
        ArcInstr::Let {
            value: ArcValue::Var(var),
            ..
        } if *var == func.params[0].var
    ));
}

#[test]
fn lower_unbound_named_const_reports_canon_invariant_violation() {
    let interner = StringInterner::new();
    let const_name = interner.intern("MISSING");
    let (_, canon) = make_canon(ori_ir::canon::CanExpr::Const(const_name), Idx::INT);

    let (func, problems) =
        lower_single_expr_with_bindings(&canon, canon.root, Idx::INT, &interner, None);

    assert!(matches!(
        problems.as_slice(),
        [ArcProblem::InternalError { message, span }]
            if message.contains("MISSING")
                && message.contains("without an exact monomorphization binding")
                && *span == Span::new(0, 10)
    ));
    assert!(matches!(
        &func.blocks[0].body[0],
        ArcInstr::Let {
            value: ArcValue::Literal(LitValue::Unit),
            ..
        }
    ));
}

#[test]
fn lower_binary_op() {
    let mut arena = CanArena::with_capacity(100);
    let left = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Int(1),
        Span::new(0, 1),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let right = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Int(2),
        Span::new(4, 5),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let add = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Binary {
            op: ori_ir::BinaryOp::Add,
            left,
            right,
        },
        Span::new(0, 5),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult::new(arena, add);

    let func = lower_single_expr(&canon, add, Idx::INT);

    // Should have: let v0 = 1, let v1 = 2, let v2 = Add(v0, v1), return v2
    assert_eq!(func.blocks[0].body.len(), 3);
    if let ArcInstr::Let { value, .. } = &func.blocks[0].body[2] {
        assert!(matches!(
            value,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                ..
            }
        ));
    } else {
        panic!("expected PrimOp");
    }
}

#[test]
fn lower_unary_op() {
    let mut arena = CanArena::with_capacity(100);
    let operand = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Int(5),
        Span::new(1, 2),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let neg = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Unary {
            op: ori_ir::UnaryOp::Neg,
            operand,
        },
        Span::new(0, 2),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult::new(arena, neg);

    let func = lower_single_expr(&canon, neg, Idx::INT);

    assert_eq!(func.blocks[0].body.len(), 2);
    if let ArcInstr::Let { value, .. } = &func.blocks[0].body[1] {
        assert!(matches!(
            value,
            ArcValue::PrimOp {
                op: PrimOp::Unary(ori_ir::UnaryOp::Neg),
                ..
            }
        ));
    } else {
        panic!("expected PrimOp");
    }
}

#[test]
fn lower_await_is_transparent() {
    let mut arena = CanArena::with_capacity(100);
    let inner = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Unit,
        Span::new(6, 10),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));
    let await_id = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Await(inner),
        Span::new(0, 10),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult::new(arena, await_id);

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::UNIT,
            body: await_id,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    // The lowered fixture evaluates its inner expression without ARC problems.
    assert_eq!(problems.len(), 0);
    // The function should have lowered the inner unit expression
    assert!(!func.blocks.is_empty());
}

#[test]
fn lower_function_with_params() {
    let mut arena = CanArena::with_capacity(100);
    let param_name = Name::from_raw(100);
    let body = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Ident(param_name),
        Span::new(0, 1),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult::new(arena, body);

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[(param_name, Idx::INT)],
            return_type: Idx::INT,
            body,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert_eq!(func.params.len(), 1);
    assert_eq!(func.params[0].ty, Idx::INT);
    assert!(!func.blocks[0].body.is_empty());
}

#[test]
fn lower_str_literal() {
    let interner = StringInterner::new();
    let hello = interner.intern("hello");
    let (_, canon) = make_canon(ori_ir::canon::CanExpr::Str(hello), Idx::STR);
    let body = canon.root;
    let func = lower_single_expr(&canon, body, Idx::STR);

    if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
        assert_eq!(*value, ArcValue::Literal(LitValue::String(hello)));
    } else {
        panic!("expected Let with String literal");
    }
}

#[test]
fn lower_function_ref_emits_partial_apply() {
    let fn_name = Name::from_raw(200);
    let mut pool = Pool::new();
    let func_ty = pool.function(&[Idx::INT], Idx::BOOL);

    let mut arena = CanArena::with_capacity(100);
    let node = CanNode::new(
        ori_ir::canon::CanExpr::FunctionRef(fn_name),
        Span::new(0, 5),
        TypeId::from_raw(func_ty.raw()),
    );
    let body = arena.push(node);
    let canon = CanonResult::new(arena, body);

    let interner = StringInterner::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: func_ty,
            body,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty());
    // FunctionRef lowers to PartialApply with empty captures
    let has_partial_apply = func.blocks[0].body.iter().any(|instr| {
        matches!(instr, ArcInstr::PartialApply { func: name, args, .. } if *name == fn_name && args.is_empty())
    });
    assert!(
        has_partial_apply,
        "expected PartialApply with empty captures"
    );
}

#[test]
fn lambda_target_preserves_callable_signature_identity() {
    let interner = StringInterner::new();
    let parent_name = interner.intern("nominal_lambda_parent");
    let parameter_name = interner.intern("color");
    let color_name = interner.intern("Color");
    let red_name = interner.intern("Red");

    let mut pool = Pool::new();
    let named_color = pool.named(color_name);
    let concrete_color = pool.enum_type(
        color_name,
        &[ori_types::EnumVariant {
            name: red_name,
            field_types: vec![],
        }],
    );
    pool.set_resolution(named_color, concrete_color);
    let lambda_type = pool.function(&[named_color], named_color);

    let mut arena = CanArena::with_capacity(3);
    let body = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Ident(parameter_name),
        Span::new(10, 15),
        TypeId::from_raw(concrete_color.raw()),
    ));
    let params = arena.push_params(&[CanParam {
        name: parameter_name,
        default: ori_ir::canon::CanId::INVALID,
    }]);
    let lambda = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Lambda { params, body },
        Span::new(0, 15),
        TypeId::from_raw(lambda_type.raw()),
    ));
    let canon = CanonResult::new(arena, lambda);

    let mut problems = Vec::new();
    let (parent, lambdas) = lower_function_can(
        ArcLoweringInput {
            name: parent_name,
            params: &[],
            return_type: lambda_type,
            body: lambda,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty(), "unexpected problems: {problems:?}");
    assert_eq!(lambdas.len(), 1);
    assert_ne!(named_color, concrete_color);
    assert_eq!(
        pool.function_params(parent.var_type(crate::ArcVarId::new(0)))[0],
        named_color
    );
    assert_eq!(
        lambdas[0].params[0].ty, named_color,
        "the target parameter must retain the closure signature's nominal type identity"
    );
    assert_eq!(
        lambdas[0].return_type, named_color,
        "the target result must use the declared closure result rather than a narrower body type"
    );
}

#[test]
fn lower_with_capability_binds_provider_for_body() {
    // In `with Cap = 42 in Cap`, the body reference must resolve to the bound
    // provider variable rather than the unbound-identifier unit fallback.
    let cap_name = Name::from_raw(300);
    let mut arena = CanArena::with_capacity(100);
    let provider = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Int(42),
        Span::new(5, 10),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let body = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Ident(cap_name),
        Span::new(11, 14),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let with_cap = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::WithCapability {
            capability: cap_name,
            provider,
            body,
        },
        Span::new(0, 15),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult::new(arena, with_cap);

    let func = lower_single_expr(&canon, with_cap, Idx::INT);

    // The provider (Int(42)) is lowered into a defining instruction.
    assert!(func.blocks[0].body.iter().any(|instr| {
        matches!(
            instr,
            ArcInstr::Let {
                value: ArcValue::Literal(LitValue::Int(42)),
                ..
            }
        )
    }));

    // The body's capability ref resolves to the bound provider var
    // (a `Let Var(_)`), proving the binding propagated — NOT a Unit fallthrough.
    assert!(func.blocks[0].body.iter().any(|instr| {
        matches!(
            instr,
            ArcInstr::Let {
                value: ArcValue::Var(_),
                ..
            }
        )
    }));
}

#[test]
fn lower_format_with_dispatches_to_runtime() {
    let interner = StringInterner::new();
    let spec = interner.intern(">10");

    let mut arena = CanArena::with_capacity(100);
    let inner = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Int(42),
        Span::new(1, 3),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let fmt = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::FormatWith { expr: inner, spec },
        Span::new(0, 10),
        TypeId::from_raw(Idx::STR.raw()),
    ));

    let canon = CanonResult::new(arena, fmt);

    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::STR,
            body: fmt,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty());
    // Should emit Apply to ori_format_int (since inner is Int)
    let format_fn = interner.intern("ori_format_int");
    let has_apply = func.blocks[0].body.iter().any(|instr| {
        matches!(instr, ArcInstr::Apply { func: name, args, .. } if *name == format_fn && args.len() == 2)
    });
    assert!(has_apply, "expected Apply to ori_format_int with 2 args");
}

#[test]
fn lower_function_exp_panic_emits_unreachable() {
    let interner = StringInterner::new();
    let msg_name = interner.intern("message");

    let mut arena = CanArena::with_capacity(100);
    let msg = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Str(interner.intern("boom")),
        Span::new(7, 13),
        TypeId::from_raw(Idx::STR.raw()),
    ));
    let props = arena.push_named_exprs(&[CanNamedExpr {
        name: msg_name,
        value: msg,
    }]);
    let panic_expr = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::FunctionExp {
            kind: FunctionExpKind::Panic,
            props,
        },
        Span::new(0, 14),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult::new(arena, panic_expr);

    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::UNIT,
            body: panic_expr,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty());
    // Panic lowers to: Invoke(ori_panic, [msg]) with normal → Unreachable
    // and unwind → Resume (for cleanup landing pad support).
    let has_unreachable = func
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, ArcTerminator::Unreachable));
    assert!(
        has_unreachable,
        "panic should produce Unreachable terminator"
    );

    let panic_fn = interner.intern("ori_panic");
    let has_panic_invoke = func.blocks.iter().any(|b| {
        matches!(
            &b.terminator,
            ArcTerminator::Invoke { func: name, .. } if *name == panic_fn
        )
    });
    assert!(
        has_panic_invoke,
        "should invoke ori_panic runtime function (not Apply — panic can unwind)"
    );
}

#[test]
fn lower_function_exp_todo_emits_unreachable() {
    let mut arena = CanArena::with_capacity(100);
    let todo_expr = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::FunctionExp {
            kind: FunctionExpKind::Todo,
            props: ori_ir::canon::CanNamedExprRange::EMPTY,
        },
        Span::new(0, 4),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult::new(arena, todo_expr);

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::UNIT,
            body: todo_expr,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty());
    // Todo emits: string("not yet implemented") + ori_panic_cstr + Unreachable
    let has_unreachable = func
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, ArcTerminator::Unreachable));
    assert!(
        has_unreachable,
        "todo should produce Unreachable terminator"
    );
}

#[test]
#[should_panic(
    expected = "post-0.1 concurrency feature `spawn` should be rejected by type checker"
)]
fn lower_post_01_concurrency_panics() {
    // Post-0.1 concurrency features are gated at the type checker (E2040).
    // If they somehow reach the lowerer, it should panic (unreachable).
    let mut arena = CanArena::with_capacity(100);
    let spawn_expr = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::FunctionExp {
            kind: FunctionExpKind::Spawn,
            props: ori_ir::canon::CanNamedExprRange::EMPTY,
        },
        Span::new(0, 5),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult::new(arena, spawn_expr);

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (_, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::UNIT,
            body: spawn_expr,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );
}

// Type substitution tests

#[test]
fn type_subst_replaces_generic_type_with_concrete() {
    // Simulate a generic function body: a single `Int(42)` expression whose
    // canonical type is a "generic" Idx. The substitution map rewrites it
    // to Idx::INT, verifying that resolve_body_type() is applied.

    // Use a high Idx value as the "generic" type placeholder.
    let generic_ty = Idx::from_raw(9999);

    let mut arena = CanArena::with_capacity(10);
    let node = CanNode::new(
        ori_ir::canon::CanExpr::Int(42),
        Span::new(0, 2),
        TypeId::from_raw(generic_ty.raw()),
    );
    let body = arena.push(node);

    let canon = CanonResult::new(arena, body);

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();

    // Build the substitution map: generic_ty → INT
    let mut subst = FxHashMap::default();
    subst.insert(generic_ty, Idx::INT);

    let (func, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: Idx::INT,
            body,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: Some(&subst),
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty(), "unexpected problems: {problems:?}");

    // The Int(42) instruction should have ty == Idx::INT, not generic_ty
    let instrs = &func.blocks[0].body;
    assert!(!instrs.is_empty(), "expected at least one instruction");

    let last = &instrs[instrs.len() - 1];
    match last {
        ArcInstr::Let { ty, value, .. } => {
            assert_eq!(*ty, Idx::INT, "type should be substituted to INT");
            assert!(
                matches!(value, ArcValue::Literal(LitValue::Int(42))),
                "value should be Int(42), got {value:?}"
            );
        }
        other => panic!("expected Let instruction, got {other:?}"),
    }
}

#[test]
fn type_subst_none_leaves_types_unchanged() {
    // Lowering with None substitution map should preserve original types.
    // Use a valid pool Idx (FLOAT is pre-interned) so pool lookups don't OOB.
    let ty = Idx::FLOAT;

    let mut arena = CanArena::with_capacity(10);
    let node = CanNode::new(
        ori_ir::canon::CanExpr::Int(7),
        Span::new(0, 1),
        TypeId::from_raw(ty.raw()),
    );
    let body = arena.push(node);

    let canon = CanonResult::new(arena, body);

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();

    let (func, _) = lower_function_can(
        ArcLoweringInput {
            name: Name::from_raw(1),
            params: &[],
            return_type: ty,
            body,
            canon: &canon,
            interner: &interner,
            pool: &pool,
            type_subst: None,
            const_bindings: None,
            is_fbip: false,
        },
        &mut problems,
    );

    assert!(problems.is_empty());
    let instrs = &func.blocks[0].body;
    let last = &instrs[instrs.len() - 1];
    match last {
        ArcInstr::Let { ty: instr_ty, .. } => {
            assert_eq!(
                *instr_ty,
                Idx::FLOAT,
                "type should be preserved as FLOAT without substitution"
            );
        }
        other => panic!("expected Let instruction, got {other:?}"),
    }
}
