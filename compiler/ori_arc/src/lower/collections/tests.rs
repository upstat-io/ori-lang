use ori_ir::canon::{CanArena, CanExpr, CanNode, CanonResult};
use ori_ir::{Name, Span, StringInterner, TypeId};
use ori_types::{Idx, Pool};

use crate::ir::{ArcInstr, CtorKind};

#[test]
fn lower_tuple() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    let a = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(1, 2),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let b = arena.push(CanNode::new(
        CanExpr::Int(2),
        Span::new(4, 5),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let exprs = arena.push_expr_list(&[a, b]);
    let tup = arena.push(CanNode::new(
        CanExpr::Tuple(exprs),
        Span::new(0, 6),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: tup,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        tup,
        &canon,
        &interner,
        &pool,
        &mut problems,
    );

    assert!(problems.is_empty());
    let last = &func.blocks[0].body[2];
    assert!(matches!(
        last,
        ArcInstr::Construct {
            ctor: CtorKind::Tuple,
            ..
        }
    ));
}

#[test]
fn lower_none() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    let none_id = arena.push(CanNode::new(
        CanExpr::None,
        Span::new(0, 4),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: none_id,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::UNIT,
        none_id,
        &canon,
        &interner,
        &pool,
        &mut problems,
    );

    assert!(problems.is_empty());
    let last = &func.blocks[0].body[0];
    assert!(matches!(
        last,
        ArcInstr::Construct {
            ctor: CtorKind::EnumVariant { variant: 1, .. },
            ..
        }
    ));
}
