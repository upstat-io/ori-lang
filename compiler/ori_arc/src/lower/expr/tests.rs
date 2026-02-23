use ori_ir::canon::{CanArena, CanNamedExpr, CanNode, CanonResult};
use ori_ir::{FunctionExpKind, Name, Span, StringInterner, TypeId};
use ori_types::Idx;
use ori_types::Pool;

use crate::ir::{ArcInstr, ArcTerminator, ArcValue, LitValue, PrimOp};

use super::super::lower_function_can;

/// Helper: create a lowerer with a single canonical expression body.
fn lower_single_expr(
    canon: &CanonResult,
    body: ori_ir::canon::CanId,
    ty: Idx,
) -> crate::ir::ArcFunction {
    let interner = StringInterner::new();
    let pool = Pool::new();

    let mut problems = Vec::new();
    let name = Name::from_raw(1);
    let (func, _lambdas) = lower_function_can(
        name,
        &[],
        ty,
        body,
        canon,
        &interner,
        &pool,
        &mut problems,
        false,
    );
    assert!(problems.is_empty(), "unexpected problems: {problems:?}");
    func
}

fn make_canon(kind: ori_ir::canon::CanExpr, ty: Idx) -> (CanArena, CanonResult) {
    let mut arena = CanArena::with_capacity(100);
    let node = CanNode::new(kind, Span::new(0, 10), TypeId::from_raw(ty.raw()));
    let body = arena.push(node);
    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: body,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };
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
        arena,
        constants,
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: body,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let func = lower_single_expr(&canon, body, Idx::INT);
    if let ArcInstr::Let { value, .. } = &func.blocks[0].body[0] {
        assert_eq!(*value, ArcValue::Literal(LitValue::Int(99)));
    } else {
        panic!("expected Let with constant value");
    }
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

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: add,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

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

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: neg,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

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

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: await_id,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        await_id,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
    );

    // Await is now transparent — just evaluates inner expression, no problems
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

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: body,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        Name::from_raw(1),
        &[(param_name, Idx::INT)],
        Idx::INT,
        body,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
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
    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: body,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let interner = StringInterner::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        Name::from_raw(1),
        &[],
        func_ty,
        body,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
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
fn lower_with_capability_is_transparent() {
    let mut arena = CanArena::with_capacity(100);
    let inner = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::Int(42),
        Span::new(5, 10),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let with_cap = arena.push(CanNode::new(
        ori_ir::canon::CanExpr::WithCapability {
            capability: Name::from_raw(300),
            provider: inner, // provider is unused at codegen level
            body: inner,
        },
        Span::new(0, 15),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: with_cap,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let func = lower_single_expr(&canon, with_cap, Idx::INT);

    // WithCapability is transparent — just evaluates the body
    assert!(func.blocks[0].body.iter().any(|instr| {
        matches!(
            instr,
            ArcInstr::Let {
                value: ArcValue::Literal(LitValue::Int(42)),
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

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: fmt,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::STR,
        fmt,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
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

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: panic_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        panic_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
    );

    assert!(problems.is_empty());
    // Panic lowers to: Apply(ori_panic, [msg]) + Unreachable terminator
    let has_unreachable = func
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, ArcTerminator::Unreachable));
    assert!(
        has_unreachable,
        "panic should produce Unreachable terminator"
    );

    let panic_fn = interner.intern("ori_panic");
    let has_panic_call = func.blocks.iter().any(|b| {
        b.body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::Apply { func: name, .. } if *name == panic_fn))
    });
    assert!(has_panic_call, "should call ori_panic runtime function");
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

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: todo_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (func, _) = lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        todo_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
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

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: spawn_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut problems = Vec::new();
    let (_, _) = lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        spawn_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
        false,
    );
}
