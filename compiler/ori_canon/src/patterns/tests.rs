use ori_ir::ast::patterns::MatchPattern;
use ori_ir::ast::Expr;
use ori_ir::canon::tree::{DecisionTree, TestKind};
use ori_ir::{ExprArena, ExprKind, SharedInterner, Span};
use ori_types::{Idx, TypeCheckResult, TypedModule};

use crate::lower;

// Helpers

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

// Tests

#[test]
fn match_wildcard_produces_leaf() {
    // match x { _ -> 42 }
    let mut arena = ExprArena::new();
    let scrutinee = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::new(0, 1)));
    let body = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(10, 12)));

    let arm = ori_ir::ast::patterns::MatchArm {
        pattern: MatchPattern::Wildcard,
        guard: None,
        body,
        span: Span::new(5, 12),
    };
    let arms_range = arena.alloc_arms([arm]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Match {
            scrutinee,
            arms: arms_range,
        },
        Span::new(0, 13),
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);

    // The match should produce a non-Fail decision tree.
    assert!(!result.decision_trees.is_empty());
    let tree = result
        .decision_trees
        .get(ori_ir::canon::DecisionTreeId::new(0));
    assert!(
        matches!(tree, DecisionTree::Leaf { arm_index: 0, .. }),
        "expected Leaf(0), got {tree:?}"
    );
}

#[test]
fn match_bool_produces_switch() {
    // match b { true -> 1, false -> 0 }
    let mut arena = ExprArena::new();
    let scrutinee = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(0, 4)));
    let body1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(20, 21)));
    let body2 = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::new(35, 36)));

    let lit_true = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(8, 12)));
    let lit_false = arena.alloc_expr(Expr::new(ExprKind::Bool(false), Span::new(25, 30)));

    let arms_range = arena.alloc_arms([
        ori_ir::ast::patterns::MatchArm {
            pattern: MatchPattern::Literal(lit_true),
            guard: None,
            body: body1,
            span: Span::new(8, 21),
        },
        ori_ir::ast::patterns::MatchArm {
            pattern: MatchPattern::Literal(lit_false),
            guard: None,
            body: body2,
            span: Span::new(25, 36),
        },
    ]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Match {
            scrutinee,
            arms: arms_range,
        },
        Span::new(0, 37),
    ));

    // expr_types: [0]=Bool(scrutinee), [1]=Int(body1), [2]=Int(body2),
    //             [3]=Bool(lit_true), [4]=Bool(lit_false), [5]=Int(match)
    let type_result = test_type_result(vec![
        Idx::BOOL,
        Idx::INT,
        Idx::INT,
        Idx::BOOL,
        Idx::BOOL,
        Idx::INT,
    ]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);

    let tree = result
        .decision_trees
        .get(ori_ir::canon::DecisionTreeId::new(0));
    if let DecisionTree::Switch {
        test_kind, edges, ..
    } = tree
    {
        assert_eq!(*test_kind, TestKind::BoolEq);
        assert_eq!(edges.len(), 2);
    } else {
        panic!("expected Switch, got {tree:?}");
    }
}

#[test]
fn match_binding_produces_leaf_with_binding() {
    // match x { v -> v }
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let name_v = interner.intern("v");

    let scrutinee = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::new(0, 1)));
    let body = arena.alloc_expr(Expr::new(ExprKind::Ident(name_v), Span::new(10, 11)));

    let arms_range = arena.alloc_arms([ori_ir::ast::patterns::MatchArm {
        pattern: MatchPattern::Binding(name_v),
        guard: None,
        body,
        span: Span::new(5, 11),
    }]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Match {
            scrutinee,
            arms: arms_range,
        },
        Span::new(0, 12),
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();

    let result = lower(&arena, &type_result, &pool, root, &interner);

    let tree = result
        .decision_trees
        .get(ori_ir::canon::DecisionTreeId::new(0));
    if let DecisionTree::Leaf {
        arm_index,
        bindings,
    } = tree
    {
        assert_eq!(*arm_index, 0);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].0, name_v);
    } else {
        panic!("expected Leaf with binding, got {tree:?}");
    }
}

#[test]
fn match_int_with_default() {
    // match n { 1 -> a, 2 -> b, _ -> c }
    let mut arena = ExprArena::new();
    let scrutinee = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::new(0, 1)));
    let body_a = arena.alloc_expr(Expr::new(ExprKind::Int(10), Span::DUMMY));
    let body_b = arena.alloc_expr(Expr::new(ExprKind::Int(20), Span::DUMMY));
    let body_c = arena.alloc_expr(Expr::new(ExprKind::Int(30), Span::DUMMY));
    let lit_1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
    let lit_2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::DUMMY));

    let arms_range = arena.alloc_arms([
        ori_ir::ast::patterns::MatchArm {
            pattern: MatchPattern::Literal(lit_1),
            guard: None,
            body: body_a,
            span: Span::DUMMY,
        },
        ori_ir::ast::patterns::MatchArm {
            pattern: MatchPattern::Literal(lit_2),
            guard: None,
            body: body_b,
            span: Span::DUMMY,
        },
        ori_ir::ast::patterns::MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: body_c,
            span: Span::DUMMY,
        },
    ]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Match {
            scrutinee,
            arms: arms_range,
        },
        Span::DUMMY,
    ));

    // 7 expressions: scrutinee, body_a, body_b, body_c, lit_1, lit_2, match
    let type_result = test_type_result(vec![
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::INT,
    ]);
    let pool = ori_types::Pool::new();
    let interner = test_interner();

    let result = lower(&arena, &type_result, &pool, root, &interner);

    let tree = result
        .decision_trees
        .get(ori_ir::canon::DecisionTreeId::new(0));
    if let DecisionTree::Switch {
        test_kind,
        edges,
        default,
        ..
    } = tree
    {
        assert_eq!(*test_kind, TestKind::IntEq);
        assert_eq!(edges.len(), 2);
        assert!(default.is_some());
    } else {
        panic!("expected Switch, got {tree:?}");
    }
}

#[test]
fn match_with_guard() {
    // match x { v if guard -> 1, _ -> 0 }
    let mut arena = ExprArena::new();
    let interner = test_interner();
    let name_v = interner.intern("v");

    let scrutinee = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::DUMMY));
    let guard = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::DUMMY));
    let body1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::DUMMY));
    let body2 = arena.alloc_expr(Expr::new(ExprKind::Int(0), Span::DUMMY));

    let arms_range = arena.alloc_arms([
        ori_ir::ast::patterns::MatchArm {
            pattern: MatchPattern::Binding(name_v),
            guard: Some(guard),
            body: body1,
            span: Span::DUMMY,
        },
        ori_ir::ast::patterns::MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: body2,
            span: Span::DUMMY,
        },
    ]);
    let root = arena.alloc_expr(Expr::new(
        ExprKind::Match {
            scrutinee,
            arms: arms_range,
        },
        Span::DUMMY,
    ));

    let type_result = test_type_result(vec![Idx::INT, Idx::BOOL, Idx::INT, Idx::INT, Idx::INT]);
    let pool = ori_types::Pool::new();

    let result = lower(&arena, &type_result, &pool, root, &interner);

    let tree = result
        .decision_trees
        .get(ori_ir::canon::DecisionTreeId::new(0));
    assert!(
        matches!(tree, DecisionTree::Guard { arm_index: 0, .. }),
        "expected Guard, got {tree:?}"
    );
}
