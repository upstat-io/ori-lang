use ori_ir::canon::{CanArena, CanNode, CanonResult};
use ori_ir::{Name, Span, StringInterner, TypeId};
use ori_types::Idx;
use ori_types::Pool;

use crate::ir::{ArcInstr, ArcTerminator, ArcValue, LitValue, PrimOp};
use crate::lower::ArcProblem;

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
    let (func, _lambdas) =
        lower_function_can(name, &[], ty, body, canon, &interner, &pool, &mut problems);
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
fn lower_unsupported_expr_produces_problem() {
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
    let (_func, _) = lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        await_id,
        &canon,
        &interner,
        &pool,
        &mut problems,
    );

    assert_eq!(problems.len(), 1);
    assert!(matches!(
        &problems[0],
        ArcProblem::UnsupportedExpr { kind: "Await", .. }
    ));
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
    );

    assert_eq!(func.params.len(), 1);
    assert_eq!(func.params[0].ty, Idx::INT);
    assert!(!func.blocks[0].body.is_empty());
}
