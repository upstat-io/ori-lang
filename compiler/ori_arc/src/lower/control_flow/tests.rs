use ori_ir::canon::{CanArena, CanBindingPattern, CanExpr, CanNode, CanonResult};
use ori_ir::{Name, Span, StringInterner, TypeId};
use ori_types::Idx;
use ori_types::Pool;

use crate::ir::ArcTerminator;

#[test]
fn lower_block_with_let() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    // { let x = 1; x + 2 }
    let lit1 = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(10, 11),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let x_name = Name::from_raw(100);
    let pat = arena.push_binding_pattern(CanBindingPattern::Name {
        name: x_name,
        mutable: false,
    });

    let let_expr = arena.push(CanNode::new(
        CanExpr::Let {
            pattern: pat,
            init: lit1,
            mutable: false,
        },
        Span::new(2, 12),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let x_ref = arena.push(CanNode::new(
        CanExpr::Ident(x_name),
        Span::new(14, 15),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let lit2 = arena.push(CanNode::new(
        CanExpr::Int(2),
        Span::new(18, 19),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let add = arena.push(CanNode::new(
        CanExpr::Binary {
            op: ori_ir::BinaryOp::Add,
            left: x_ref,
            right: lit2,
        },
        Span::new(14, 19),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let stmts = arena.push_expr_list(&[let_expr]);
    let block = arena.push(CanNode::new(
        CanExpr::Block { stmts, result: add },
        Span::new(0, 20),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: block,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        block,
        &canon,
        &interner,
        &pool,
        &mut problems,
    );

    assert!(problems.is_empty(), "problems: {problems:?}");
    assert!(func.blocks[0].body.len() >= 3);
}

#[test]
fn lower_if_else_produces_four_blocks() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    let cond = arena.push(CanNode::new(
        CanExpr::Bool(true),
        Span::new(3, 7),
        TypeId::from_raw(Idx::BOOL.raw()),
    ));
    let then_val = arena.push(CanNode::new(
        CanExpr::Int(1),
        Span::new(10, 11),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let else_val = arena.push(CanNode::new(
        CanExpr::Int(2),
        Span::new(17, 18),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let if_expr = arena.push(CanNode::new(
        CanExpr::If {
            cond,
            then_branch: then_val,
            else_branch: else_val,
        },
        Span::new(0, 19),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: if_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        if_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
    );

    assert!(problems.is_empty());
    assert_eq!(func.blocks.len(), 4);
    assert!(matches!(
        func.blocks[0].terminator,
        ArcTerminator::Branch { .. }
    ));
    assert!(!func.blocks[3].params.is_empty());
}

#[test]
fn lower_loop_produces_header_and_exit() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    // loop { break 42 }
    let lit42 = arena.push(CanNode::new(
        CanExpr::Int(42),
        Span::new(14, 16),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let break_expr = arena.push(CanNode::new(
        CanExpr::Break {
            label: Name::EMPTY,
            value: lit42,
        },
        Span::new(8, 16),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));
    let loop_expr = arena.push(CanNode::new(
        CanExpr::Loop {
            label: Name::EMPTY,
            body: break_expr,
        },
        Span::new(0, 18),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let canon = CanonResult {
        arena,
        constants: ori_ir::canon::ConstantPool::new(),
        decision_trees: ori_ir::canon::DecisionTreePool::default(),
        root: loop_expr,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };

    let mut problems = Vec::new();
    let (func, _) = super::super::super::lower_function_can(
        Name::from_raw(1),
        &[],
        Idx::INT,
        loop_expr,
        &canon,
        &interner,
        &pool,
        &mut problems,
    );

    assert!(problems.is_empty(), "problems: {problems:?}");
    assert!(func.blocks.len() >= 3);
}
