use ori_ir::canon::{CanArena, CanBindingPattern, CanExpr, CanNode, CanonResult};
use ori_ir::{Name, Span, StringInterner, TypeId};
use ori_types::Idx;
use ori_types::Pool;

#[test]
fn bind_name_pattern() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let mut arena = CanArena::with_capacity(200);

    let x_name = Name::from_raw(100);
    let pat = arena.push_binding_pattern(CanBindingPattern::Name {
        name: x_name,
        mutable: false,
    });
    let init = arena.push(CanNode::new(
        CanExpr::Int(42),
        Span::new(10, 12),
        TypeId::from_raw(Idx::INT.raw()),
    ));

    let let_expr = arena.push(CanNode::new(
        CanExpr::Let {
            pattern: pat,
            init,
            mutable: false,
        },
        Span::new(0, 12),
        TypeId::from_raw(Idx::UNIT.raw()),
    ));

    let x_ref = arena.push(CanNode::new(
        CanExpr::Ident(x_name),
        Span::new(14, 15),
        TypeId::from_raw(Idx::INT.raw()),
    ));
    let stmts = arena.push_expr_list(&[let_expr]);
    let block = arena.push(CanNode::new(
        CanExpr::Block {
            stmts,
            result: x_ref,
        },
        Span::new(0, 16),
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
    assert!(func.blocks[0].body.len() >= 2);
}
