use super::*;
use crate::Pool;
use ori_ir::{
    ast::{Expr, ExprKind, MapEntry, MatchArm, MatchPattern, Param, Stmt, StmtKind},
    BindingPattern, ExprArena, ExprId, Name, Span,
};

// ========================================================================
// Test Helpers
// ========================================================================

/// Create a Name from a raw u32 for testing.
fn name(n: u32) -> Name {
    Name::from_raw(n)
}

/// Create a dummy span for test expressions.
fn span() -> Span {
    Span::DUMMY
}

/// Helper to build an expression and get its ID.
fn alloc(arena: &mut ExprArena, kind: ExprKind) -> ExprId {
    arena.alloc_expr(Expr::new(kind, span()))
}

// ========================================================================
// Literal Inference Tests
// ========================================================================

#[test]
fn test_infer_literal_int() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Int(42));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_float() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Float(3_14_f64.to_bits()));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::FLOAT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_bool() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let true_id = alloc(&mut arena, ExprKind::Bool(true));
    let false_id = alloc(&mut arena, ExprKind::Bool(false));

    assert_eq!(infer_expr(&mut engine, &arena, true_id), Idx::BOOL);
    assert_eq!(infer_expr(&mut engine, &arena, false_id), Idx::BOOL);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_str() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::String(name(1)));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::STR);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_char() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Char('a'));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::CHAR);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_unit() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Unit);
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::UNIT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_duration() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let expr_id = alloc(
        &mut arena,
        ExprKind::Duration {
            value: 100,
            unit: ori_ir::DurationUnit::Milliseconds,
        },
    );
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::DURATION);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_size() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let expr_id = alloc(
        &mut arena,
        ExprKind::Size {
            value: 1024,
            unit: ori_ir::SizeUnit::Kilobytes,
        },
    );
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::SIZE);
    assert!(!engine.has_errors());
}

// ========================================================================
// Binary Operator Tests
// ========================================================================

#[test]
fn test_infer_binary_arithmetic_int() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::Int(10));
    let right = alloc(&mut arena, ExprKind::Int(5));
    let add = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, add);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_binary_arithmetic_float() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::Float(1_5_f64.to_bits()));
    let right = alloc(&mut arena, ExprKind::Float(2_5_f64.to_bits()));
    let mul = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Mul,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, mul);

    assert_eq!(ty, Idx::FLOAT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_binary_comparison() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::Int(10));
    let right = alloc(&mut arena, ExprKind::Int(5));
    let lt = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Lt,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, lt);

    assert_eq!(ty, Idx::BOOL);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_binary_equality() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::String(name(1)));
    let right = alloc(&mut arena, ExprKind::String(name(2)));
    let eq = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Eq,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, eq);

    assert_eq!(ty, Idx::BOOL);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_binary_boolean_and() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::Bool(true));
    let right = alloc(&mut arena, ExprKind::Bool(false));
    let and_op = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::And,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, and_op);

    assert_eq!(ty, Idx::BOOL);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_binary_boolean_or() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::Bool(true));
    let right = alloc(&mut arena, ExprKind::Bool(false));
    let or_op = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Or,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, or_op);

    assert_eq!(ty, Idx::BOOL);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_binary_bitwise() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::Int(0xFF));
    let right = alloc(&mut arena, ExprKind::Int(0x0F));
    let bitand = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::BitAnd,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, bitand);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_binary_range() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::Int(1));
    let right = alloc(&mut arena, ExprKind::Int(10));
    let range = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Range,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, range);

    assert_eq!(engine.pool().tag(ty), Tag::Range);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_binary_type_mismatch_reports_error() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let left = alloc(&mut arena, ExprKind::Int(10));
    let right = alloc(&mut arena, ExprKind::String(name(1)));
    let add = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        },
    );

    let _ = infer_expr(&mut engine, &arena, add);

    assert!(engine.has_errors(), "Should report type mismatch error");
}

// ========================================================================
// Unary Operator Tests
// ========================================================================

#[test]
fn test_infer_unary_neg_int() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let operand = alloc(&mut arena, ExprKind::Int(42));
    let neg = alloc(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        },
    );

    let ty = infer_expr(&mut engine, &arena, neg);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_unary_neg_float() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let operand = alloc(&mut arena, ExprKind::Float(3_14_f64.to_bits()));
    let neg = alloc(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        },
    );

    let ty = infer_expr(&mut engine, &arena, neg);

    assert_eq!(ty, Idx::FLOAT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_unary_not() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let operand = alloc(&mut arena, ExprKind::Bool(true));
    let not = alloc(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::Not,
            operand,
        },
    );

    let ty = infer_expr(&mut engine, &arena, not);

    assert_eq!(ty, Idx::BOOL);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_unary_bitnot() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let operand = alloc(&mut arena, ExprKind::Int(0xFF));
    let bitnot = alloc(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::BitNot,
            operand,
        },
    );

    let ty = infer_expr(&mut engine, &arena, bitnot);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_unary_try_option() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'opt' to Option<int>
    let opt_ty = engine.infer_option(Idx::INT);
    engine.env_mut().bind(name(1), opt_ty);

    let operand = alloc(&mut arena, ExprKind::Ident(name(1)));
    let try_op = alloc(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::Try,
            operand,
        },
    );

    let ty = infer_expr(&mut engine, &arena, try_op);

    assert_eq!(ty, Idx::INT, "Try on Option<int> should yield int");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_unary_try_result() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'res' to Result<str, int>
    let res_ty = engine.infer_result(Idx::STR, Idx::INT);
    engine.env_mut().bind(name(1), res_ty);

    let operand = alloc(&mut arena, ExprKind::Ident(name(1)));
    let try_op = alloc(
        &mut arena,
        ExprKind::Unary {
            op: UnaryOp::Try,
            operand,
        },
    );

    let ty = infer_expr(&mut engine, &arena, try_op);

    assert_eq!(ty, Idx::STR, "Try on Result<str, _> should yield str");
    assert!(!engine.has_errors());
}

// ========================================================================
// Collection Inference Tests
// ========================================================================

#[test]
fn test_infer_empty_list() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let list = arena.alloc_expr_list_inline(&[]);
    let expr_id = alloc(&mut arena, ExprKind::List(list));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(engine.pool().tag(ty), Tag::List);
    // Element type should be a fresh variable
    let elem_ty = engine.pool().list_elem(ty);
    assert_eq!(engine.pool().tag(elem_ty), Tag::Var);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_list_homogeneous() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let e1 = alloc(&mut arena, ExprKind::Int(1));
    let e2 = alloc(&mut arena, ExprKind::Int(2));
    let e3 = alloc(&mut arena, ExprKind::Int(3));
    let list = arena.alloc_expr_list_inline(&[e1, e2, e3]);
    let expr_id = alloc(&mut arena, ExprKind::List(list));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(engine.pool().tag(ty), Tag::List);
    let elem_ty = engine.resolve(engine.pool().list_elem(ty));
    assert_eq!(elem_ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_list_heterogeneous_error() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let e1 = alloc(&mut arena, ExprKind::Int(1));
    let e2 = alloc(&mut arena, ExprKind::String(name(1)));
    let list = arena.alloc_expr_list_inline(&[e1, e2]);
    let expr_id = alloc(&mut arena, ExprKind::List(list));

    let _ = infer_expr(&mut engine, &arena, expr_id);

    assert!(
        engine.has_errors(),
        "Mixed int/str in list should report error"
    );
}

#[test]
fn test_infer_tuple() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let e1 = alloc(&mut arena, ExprKind::Int(42));
    let e2 = alloc(&mut arena, ExprKind::String(name(1)));
    let e3 = alloc(&mut arena, ExprKind::Bool(true));
    let tuple = arena.alloc_expr_list_inline(&[e1, e2, e3]);
    let expr_id = alloc(&mut arena, ExprKind::Tuple(tuple));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(engine.pool().tag(ty), Tag::Tuple);
    let elems = engine.pool().tuple_elems(ty);
    assert_eq!(elems.len(), 3);
    assert_eq!(elems[0], Idx::INT);
    assert_eq!(elems[1], Idx::STR);
    assert_eq!(elems[2], Idx::BOOL);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_empty_map() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let entries = arena.alloc_map_entries(std::iter::empty());
    let expr_id = alloc(&mut arena, ExprKind::Map(entries));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(engine.pool().tag(ty), Tag::Map);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_map_with_entries() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let k1 = alloc(&mut arena, ExprKind::String(name(1)));
    let v1 = alloc(&mut arena, ExprKind::Int(100));
    let k2 = alloc(&mut arena, ExprKind::String(name(2)));
    let v2 = alloc(&mut arena, ExprKind::Int(200));

    let entries = arena.alloc_map_entries([
        MapEntry {
            key: k1,
            value: v1,
            span: span(),
        },
        MapEntry {
            key: k2,
            value: v2,
            span: span(),
        },
    ]);
    let expr_id = alloc(&mut arena, ExprKind::Map(entries));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(engine.pool().tag(ty), Tag::Map);
    let key_ty = engine.resolve(engine.pool().map_key(ty));
    let val_ty = engine.resolve(engine.pool().map_value(ty));
    assert_eq!(key_ty, Idx::STR);
    assert_eq!(val_ty, Idx::INT);
    assert!(!engine.has_errors());
}

// ========================================================================
// If/Else Inference Tests
// ========================================================================

#[test]
fn test_infer_if_with_else() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let cond = alloc(&mut arena, ExprKind::Bool(true));
    let then_branch = alloc(&mut arena, ExprKind::Int(1));
    let else_branch = alloc(&mut arena, ExprKind::Int(2));

    let if_expr = alloc(
        &mut arena,
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        },
    );

    let ty = infer_expr(&mut engine, &arena, if_expr);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_if_without_else() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let cond = alloc(&mut arena, ExprKind::Bool(true));
    let then_branch = alloc(&mut arena, ExprKind::Unit);

    let if_expr = alloc(
        &mut arena,
        ExprKind::If {
            cond,
            then_branch,
            else_branch: ExprId::INVALID,
        },
    );

    let ty = infer_expr(&mut engine, &arena, if_expr);

    assert_eq!(ty, Idx::UNIT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_if_branch_mismatch() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let cond = alloc(&mut arena, ExprKind::Bool(true));
    let then_branch = alloc(&mut arena, ExprKind::Int(1));
    let else_branch = alloc(&mut arena, ExprKind::String(name(1)));

    let if_expr = alloc(
        &mut arena,
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        },
    );

    let _ = infer_expr(&mut engine, &arena, if_expr);

    assert!(
        engine.has_errors(),
        "Mismatched branches should report error"
    );
}

#[test]
fn test_infer_if_non_bool_condition() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let cond = alloc(&mut arena, ExprKind::Int(1)); // Not a bool!
    let then_branch = alloc(&mut arena, ExprKind::Unit);

    let if_expr = alloc(
        &mut arena,
        ExprKind::If {
            cond,
            then_branch,
            else_branch: ExprId::INVALID,
        },
    );

    let _ = infer_expr(&mut engine, &arena, if_expr);

    assert!(
        engine.has_errors(),
        "Non-bool condition should report error"
    );
}

// ========================================================================
// Match Expression Tests
// ========================================================================

#[test]
fn test_infer_match_simple() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let scrutinee = alloc(&mut arena, ExprKind::Int(42));
    let body1 = alloc(&mut arena, ExprKind::String(name(1)));
    let body2 = alloc(&mut arena, ExprKind::String(name(2)));

    // Pattern: _
    let pattern1 = arena.alloc_match_pattern(MatchPattern::Wildcard);
    let pattern2 = arena.alloc_match_pattern(MatchPattern::Wildcard);

    let arms = arena.alloc_arms([
        MatchArm {
            pattern: arena.get_match_pattern(pattern1).clone(),
            guard: None,
            body: body1,
            span: span(),
        },
        MatchArm {
            pattern: arena.get_match_pattern(pattern2).clone(),
            guard: None,
            body: body2,
            span: span(),
        },
    ]);

    let match_expr = alloc(&mut arena, ExprKind::Match { scrutinee, arms });

    let ty = infer_expr(&mut engine, &arena, match_expr);

    assert_eq!(ty, Idx::STR, "Match should return string type");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_match_with_binding() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let scrutinee = alloc(&mut arena, ExprKind::Int(42));

    // Use the bound variable 'x' in the body
    let x_ref = alloc(&mut arena, ExprKind::Ident(name(1)));

    // Pattern: x (binding)
    let pattern = arena.alloc_match_pattern(MatchPattern::Binding(name(1)));

    let arms = arena.alloc_arms([MatchArm {
        pattern: arena.get_match_pattern(pattern).clone(),
        guard: None,
        body: x_ref,
        span: span(),
    }]);

    let match_expr = alloc(&mut arena, ExprKind::Match { scrutinee, arms });

    let ty = infer_expr(&mut engine, &arena, match_expr);

    assert_eq!(
        ty,
        Idx::INT,
        "Binding 'x' should have int type from scrutinee"
    );
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_match_arm_type_mismatch() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let scrutinee = alloc(&mut arena, ExprKind::Int(42));
    let body1 = alloc(&mut arena, ExprKind::Int(1));
    let body2 = alloc(&mut arena, ExprKind::String(name(1))); // Type mismatch!

    let pattern1 = arena.alloc_match_pattern(MatchPattern::Wildcard);
    let pattern2 = arena.alloc_match_pattern(MatchPattern::Wildcard);

    let arms = arena.alloc_arms([
        MatchArm {
            pattern: arena.get_match_pattern(pattern1).clone(),
            guard: None,
            body: body1,
            span: span(),
        },
        MatchArm {
            pattern: arena.get_match_pattern(pattern2).clone(),
            guard: None,
            body: body2,
            span: span(),
        },
    ]);

    let match_expr = alloc(&mut arena, ExprKind::Match { scrutinee, arms });
    let _ = infer_expr(&mut engine, &arena, match_expr);

    assert!(
        engine.has_errors(),
        "Mismatched arm types should report error"
    );
}

// ========================================================================
// For Loop Tests
// ========================================================================

#[test]
fn test_infer_for_do() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'list' to [int]
    let list_ty = engine.infer_list(Idx::INT);
    engine.env_mut().bind(name(1), list_ty);

    let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
    let body = alloc(&mut arena, ExprKind::Unit);

    let for_expr = alloc(
        &mut arena,
        ExprKind::For {
            label: Name::EMPTY,
            binding: name(2), // 'x'
            iter,
            guard: ExprId::INVALID,
            body,
            is_yield: false,
        },
    );

    let ty = infer_expr(&mut engine, &arena, for_expr);

    assert_eq!(ty, Idx::UNIT, "For-do should return unit");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_for_yield() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'list' to [int]
    let list_ty = engine.infer_list(Idx::INT);
    engine.env_mut().bind(name(1), list_ty);

    let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
    // Use x (the bound element) in body
    let x_ref = alloc(&mut arena, ExprKind::Ident(name(2)));

    let for_expr = alloc(
        &mut arena,
        ExprKind::For {
            label: Name::EMPTY,
            binding: name(2), // 'x'
            iter,
            guard: ExprId::INVALID,
            body: x_ref,
            is_yield: true,
        },
    );

    let ty = infer_expr(&mut engine, &arena, for_expr);

    assert_eq!(
        engine.pool().tag(ty),
        Tag::List,
        "For-yield should return list"
    );
    let elem_ty = engine.resolve(engine.pool().list_elem(ty));
    assert_eq!(elem_ty, Idx::INT, "Yielded elements should be int");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_for_with_guard() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'list' to [int]
    let list_ty = engine.infer_list(Idx::INT);
    engine.env_mut().bind(name(1), list_ty);

    let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
    let guard = alloc(&mut arena, ExprKind::Bool(true));
    let body = alloc(&mut arena, ExprKind::Unit);

    let for_expr = alloc(
        &mut arena,
        ExprKind::For {
            label: Name::EMPTY,
            binding: name(2),
            iter,
            guard,
            body,
            is_yield: false,
        },
    );

    let ty = infer_expr(&mut engine, &arena, for_expr);

    assert_eq!(ty, Idx::UNIT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_for_guard_not_bool() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let list_ty = engine.infer_list(Idx::INT);
    engine.env_mut().bind(name(1), list_ty);

    let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
    let guard = alloc(&mut arena, ExprKind::Int(1)); // Not bool!
    let body = alloc(&mut arena, ExprKind::Unit);

    let for_expr = alloc(
        &mut arena,
        ExprKind::For {
            label: Name::EMPTY,
            binding: name(2),
            iter,
            guard,
            body,
            is_yield: false,
        },
    );

    let _ = infer_expr(&mut engine, &arena, for_expr);

    assert!(engine.has_errors(), "Non-bool guard should report error");
}

// ========================================================================
// Loop (Infinite) Tests
// ========================================================================

#[test]
fn test_infer_infinite_loop() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let body = alloc(&mut arena, ExprKind::Unit);
    let loop_expr = alloc(
        &mut arena,
        ExprKind::Loop {
            label: Name::EMPTY,
            body,
        },
    );

    let ty = infer_expr(&mut engine, &arena, loop_expr);

    // Infinite loop (no break) returns Never — it never terminates
    assert_eq!(ty, Idx::NEVER);
    assert!(!engine.has_errors());
}

// ========================================================================
// Identifier and Environment Tests
// ========================================================================

#[test]
fn test_infer_ident_bound() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    engine.env_mut().bind(name(1), Idx::INT);

    let expr_id = alloc(&mut arena, ExprKind::Ident(name(1)));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_ident_unbound() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Ident(name(999)));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::ERROR);
    assert!(
        engine.has_errors(),
        "Unbound identifier should report error"
    );
}

// ========================================================================
// Function Call Tests
// ========================================================================

#[test]
fn test_infer_call_simple() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'f' to (int) -> str
    let fn_ty = engine.infer_function(&[Idx::INT], Idx::STR);
    engine.env_mut().bind(name(1), fn_ty);

    let func = alloc(&mut arena, ExprKind::Ident(name(1)));
    let arg = alloc(&mut arena, ExprKind::Int(42));
    let args = arena.alloc_expr_list_inline(&[arg]);

    let call = alloc(&mut arena, ExprKind::Call { func, args });

    let ty = infer_expr(&mut engine, &arena, call);

    assert_eq!(ty, Idx::STR);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_call_arity_mismatch() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'f' to (int, int) -> str (expects 2 args)
    let fn_ty = engine.infer_function(&[Idx::INT, Idx::INT], Idx::STR);
    engine.env_mut().bind(name(1), fn_ty);

    let func = alloc(&mut arena, ExprKind::Ident(name(1)));
    let arg = alloc(&mut arena, ExprKind::Int(42));
    let args = arena.alloc_expr_list_inline(&[arg]); // Only 1 arg

    let call = alloc(&mut arena, ExprKind::Call { func, args });
    let ty = infer_expr(&mut engine, &arena, call);

    assert_eq!(ty, Idx::ERROR);
    assert!(engine.has_errors(), "Arity mismatch should report error");
}

#[test]
fn test_infer_call_arg_type_mismatch() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'f' to (int) -> str
    let fn_ty = engine.infer_function(&[Idx::INT], Idx::STR);
    engine.env_mut().bind(name(1), fn_ty);

    let func = alloc(&mut arena, ExprKind::Ident(name(1)));
    let arg = alloc(&mut arena, ExprKind::String(name(2))); // str, not int
    let args = arena.alloc_expr_list_inline(&[arg]);

    let call = alloc(&mut arena, ExprKind::Call { func, args });
    let _ = infer_expr(&mut engine, &arena, call);

    assert!(
        engine.has_errors(),
        "Argument type mismatch should report error"
    );
}

#[test]
fn test_infer_call_not_callable() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'x' to int (not callable)
    engine.env_mut().bind(name(1), Idx::INT);

    let func = alloc(&mut arena, ExprKind::Ident(name(1)));
    let args = arena.alloc_expr_list_inline(&[]);

    let call = alloc(&mut arena, ExprKind::Call { func, args });
    let ty = infer_expr(&mut engine, &arena, call);

    assert_eq!(ty, Idx::ERROR);
    assert!(
        engine.has_errors(),
        "Calling non-function should report error"
    );
}

// ========================================================================
// Lambda Tests
// ========================================================================

#[test]
fn test_infer_lambda_simple() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // |x| x (identity function)
    let body = alloc(&mut arena, ExprKind::Ident(name(1)));
    let params = arena.alloc_params([Param {
        name: name(1),
        pattern: None,
        ty: None,
        default: None,
        is_variadic: false,
        span: span(),
    }]);

    let lambda = alloc(
        &mut arena,
        ExprKind::Lambda {
            params,
            ret_ty: ori_ir::ParsedTypeId::INVALID,
            body,
        },
    );

    let ty = infer_expr(&mut engine, &arena, lambda);

    assert_eq!(engine.pool().tag(ty), Tag::Function);
    let params_ty = engine.pool().function_params(ty);
    assert_eq!(params_ty.len(), 1);
    // Parameter type is a fresh variable
    assert_eq!(engine.pool().tag(params_ty[0]), Tag::Var);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_lambda_with_body_int() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // |x| 42 (constant function returning int)
    let body = alloc(&mut arena, ExprKind::Int(42));
    let params = arena.alloc_params([Param {
        name: name(1),
        pattern: None,
        ty: None,
        default: None,
        is_variadic: false,
        span: span(),
    }]);

    let lambda = alloc(
        &mut arena,
        ExprKind::Lambda {
            params,
            ret_ty: ori_ir::ParsedTypeId::INVALID,
            body,
        },
    );

    let ty = infer_expr(&mut engine, &arena, lambda);

    assert_eq!(engine.pool().tag(ty), Tag::Function);
    let ret_ty = engine.pool().function_return(ty);
    assert_eq!(ret_ty, Idx::INT);
    assert!(!engine.has_errors());
}

// ========================================================================
// Block Tests
// ========================================================================

#[test]
fn test_infer_block_empty() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let stmts = arena.alloc_stmt_range(0, 0);
    let block = alloc(
        &mut arena,
        ExprKind::Block {
            stmts,
            result: ExprId::INVALID,
        },
    );

    let ty = infer_expr(&mut engine, &arena, block);

    assert_eq!(ty, Idx::UNIT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_block_with_result() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let result_expr = alloc(&mut arena, ExprKind::Int(42));
    let stmts = arena.alloc_stmt_range(0, 0);
    let block = alloc(
        &mut arena,
        ExprKind::Block {
            stmts,
            result: result_expr,
        },
    );

    let ty = infer_expr(&mut engine, &arena, block);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_block_with_let() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // { let x = 42; x }
    let init = alloc(&mut arena, ExprKind::Int(42));
    let pattern = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: true,
    });
    let _stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern,
            ty: ori_ir::ParsedTypeId::INVALID,
            init,
            mutable: false,
        },
        span: span(),
    });

    let result_expr = alloc(&mut arena, ExprKind::Ident(name(1)));
    let stmts = arena.alloc_stmt_range(0, 1);
    let block = alloc(
        &mut arena,
        ExprKind::Block {
            stmts,
            result: result_expr,
        },
    );

    let ty = infer_expr(&mut engine, &arena, block);

    assert_eq!(ty, Idx::INT, "Block should resolve x to int");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_block_let_with_type_annotation() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // { let x: int = 42; x }
    let init = alloc(&mut arena, ExprKind::Int(42));
    let pattern = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: true,
    });
    let int_ty = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let _stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern,
            ty: int_ty,
            init,
            mutable: false,
        },
        span: span(),
    });

    let result_expr = alloc(&mut arena, ExprKind::Ident(name(1)));
    let stmts = arena.alloc_stmt_range(0, 1);
    let block = alloc(
        &mut arena,
        ExprKind::Block {
            stmts,
            result: result_expr,
        },
    );

    let ty = infer_expr(&mut engine, &arena, block);

    assert_eq!(
        ty,
        Idx::INT,
        "Block let with annotation should resolve to int"
    );
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_block_let_annotation_list_type() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // { let xs: [int] = [1, 2, 3]; xs }
    let elem1 = alloc(&mut arena, ExprKind::Int(1));
    let elem2 = alloc(&mut arena, ExprKind::Int(2));
    let elem3 = alloc(&mut arena, ExprKind::Int(3));
    let list_exprs = arena.alloc_expr_list_inline(&[elem1, elem2, elem3]);
    let list = alloc(&mut arena, ExprKind::List(list_exprs));

    // Create [int] parsed type
    let int_type_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let list_annotation = ParsedType::List(int_type_id);

    let pattern = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: true,
    });
    let list_ty = arena.alloc_parsed_type(list_annotation);
    let _stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern,
            ty: list_ty,
            init: list,
            mutable: false,
        },
        span: span(),
    });

    let result_expr = alloc(&mut arena, ExprKind::Ident(name(1)));
    let stmts = arena.alloc_stmt_range(0, 1);
    let block = alloc(
        &mut arena,
        ExprKind::Block {
            stmts,
            result: result_expr,
        },
    );

    let ty = infer_expr(&mut engine, &arena, block);

    // Should be List<int>
    assert_eq!(engine.pool().tag(ty), Tag::List);
    let inner = engine.pool().list_elem(ty);
    assert_eq!(inner, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_block_let_annotation_type_mismatch() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // { let x: str = 42; x }
    // Type mismatch: annotated str but init is int
    let init = alloc(&mut arena, ExprKind::Int(42));
    let pattern = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: true,
    });
    let str_ty = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::STR));
    let _stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern,
            ty: str_ty,
            init,
            mutable: false,
        },
        span: span(),
    });

    let result_expr = alloc(&mut arena, ExprKind::Ident(name(1)));
    let stmts = arena.alloc_stmt_range(0, 1);
    let block = alloc(
        &mut arena,
        ExprKind::Block {
            stmts,
            result: result_expr,
        },
    );

    let ty = infer_expr(&mut engine, &arena, block);

    // The annotation type should be used (str), but an error should be reported
    assert_eq!(
        ty,
        Idx::STR,
        "Annotation type should be used even on mismatch"
    );
    assert!(engine.has_errors(), "Type mismatch should produce an error");
}

// ========================================================================
// Option/Result Constructor Tests
// ========================================================================

#[test]
fn test_infer_some() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let inner = alloc(&mut arena, ExprKind::Int(42));
    let some = alloc(&mut arena, ExprKind::Some(inner));

    let ty = infer_expr(&mut engine, &arena, some);

    assert_eq!(engine.pool().tag(ty), Tag::Option);
    let inner_ty = engine.pool().option_inner(ty);
    assert_eq!(inner_ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_none() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let none = alloc(&mut arena, ExprKind::None);

    let ty = infer_expr(&mut engine, &arena, none);

    assert_eq!(engine.pool().tag(ty), Tag::Option);
    // Inner type is a fresh variable
    let inner_ty = engine.pool().option_inner(ty);
    assert_eq!(engine.pool().tag(inner_ty), Tag::Var);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_ok() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let inner = alloc(&mut arena, ExprKind::String(name(1)));
    let ok = alloc(&mut arena, ExprKind::Ok(inner));

    let ty = infer_expr(&mut engine, &arena, ok);

    assert_eq!(engine.pool().tag(ty), Tag::Result);
    let ok_ty = engine.pool().result_ok(ty);
    assert_eq!(ok_ty, Idx::STR);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_err() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let inner = alloc(&mut arena, ExprKind::String(name(1)));
    let err = alloc(&mut arena, ExprKind::Err(inner));

    let ty = infer_expr(&mut engine, &arena, err);

    assert_eq!(engine.pool().tag(ty), Tag::Result);
    let err_ty = engine.pool().result_err(ty);
    assert_eq!(err_ty, Idx::STR);
    assert!(!engine.has_errors());
}

// ========================================================================
// Range Expression Tests
// ========================================================================

#[test]
fn test_infer_range_explicit() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let start = alloc(&mut arena, ExprKind::Int(1));
    let end = alloc(&mut arena, ExprKind::Int(10));

    let range = alloc(
        &mut arena,
        ExprKind::Range {
            start,
            end,
            step: ExprId::INVALID,
            inclusive: false,
        },
    );

    let ty = infer_expr(&mut engine, &arena, range);

    assert_eq!(engine.pool().tag(ty), Tag::Range);
    let elem_ty = engine.resolve(engine.pool().range_elem(ty));
    assert_eq!(elem_ty, Idx::INT);
    assert!(!engine.has_errors());
}

// ========================================================================
// Assignment Tests
// ========================================================================

#[test]
fn test_infer_assign() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'x' to int
    engine.env_mut().bind(name(1), Idx::INT);

    let target = alloc(&mut arena, ExprKind::Ident(name(1)));
    let value = alloc(&mut arena, ExprKind::Int(42));
    let assign = alloc(&mut arena, ExprKind::Assign { target, value });

    let ty = infer_expr(&mut engine, &arena, assign);

    assert_eq!(ty, Idx::UNIT, "Assignment returns unit");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_assign_type_mismatch() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    engine.env_mut().bind(name(1), Idx::INT);

    let target = alloc(&mut arena, ExprKind::Ident(name(1)));
    let value = alloc(&mut arena, ExprKind::String(name(2))); // str, not int
    let assign = alloc(&mut arena, ExprKind::Assign { target, value });

    let _ = infer_expr(&mut engine, &arena, assign);

    assert!(
        engine.has_errors(),
        "Assigning wrong type should report error"
    );
}

// ========================================================================
// Break/Continue Tests
// ========================================================================

#[test]
fn test_infer_break() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let break_expr = alloc(
        &mut arena,
        ExprKind::Break {
            label: Name::EMPTY,
            value: ExprId::INVALID,
        },
    );
    let ty = infer_expr(&mut engine, &arena, break_expr);

    assert_eq!(ty, Idx::NEVER, "Break returns never type");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_continue() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let continue_expr = alloc(
        &mut arena,
        ExprKind::Continue {
            label: Name::EMPTY,
            value: ExprId::INVALID,
        },
    );
    let ty = infer_expr(&mut engine, &arena, continue_expr);

    assert_eq!(ty, Idx::NEVER, "Continue returns never type");
    assert!(!engine.has_errors());
}

// ========================================================================
// Error Expression Test
// ========================================================================

#[test]
fn test_infer_error_expr() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    let error = alloc(&mut arena, ExprKind::Error);
    let ty = infer_expr(&mut engine, &arena, error);

    assert_eq!(ty, Idx::ERROR);
    assert!(!engine.has_errors(), "Error expr itself doesn't add errors");
}

// ========================================================================
// Coalesce Operator Tests
// ========================================================================

#[test]
fn test_infer_coalesce() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // Bind 'opt' to Option<int>
    let opt_ty = engine.infer_option(Idx::INT);
    engine.env_mut().bind(name(1), opt_ty);

    let left = alloc(&mut arena, ExprKind::Ident(name(1)));
    let right = alloc(&mut arena, ExprKind::Int(0));
    let coalesce = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Coalesce,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, coalesce);

    assert_eq!(ty, Idx::INT, "Option<int> ?? int = int");
    assert!(!engine.has_errors());
}

// ========================================================================
// Pattern Expression Tests (FunctionSeq)
// ========================================================================

#[test]
fn test_infer_function_seq_run() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // run(let x = 42, x + 1)
    let init = alloc(&mut arena, ExprKind::Int(42));
    let pattern = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: true,
    });
    let bindings = arena.alloc_seq_bindings([ori_ir::SeqBinding::Let {
        pattern,
        ty: ori_ir::ParsedTypeId::INVALID,
        value: init,
        mutable: false,
        span: Span::DUMMY,
    }]);

    // x + 1 where x is name(1)
    let x_ref = alloc(&mut arena, ExprKind::Ident(name(1)));
    let one = alloc(&mut arena, ExprKind::Int(1));
    let result = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: x_ref,
            right: one,
        },
    );

    let func_seq = ori_ir::FunctionSeq::Run {
        pre_checks: ori_ir::CheckRange::EMPTY,
        bindings,
        result,
        post_checks: ori_ir::CheckRange::EMPTY,
        span: Span::DUMMY,
    };
    let seq_id = arena.alloc_function_seq(func_seq);
    let expr_id = alloc(&mut arena, ExprKind::FunctionSeq(seq_id));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::INT, "run should return result type");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_function_seq_run_multiple_bindings() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // run(let x = 1, let y = "hello", y)
    let x_init = alloc(&mut arena, ExprKind::Int(1));
    let y_init = alloc(&mut arena, ExprKind::String(ori_ir::Name::from_raw(100)));

    let pattern1 = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: true,
    });
    let pattern2 = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(2),
        mutable: true,
    });
    let bindings = arena.alloc_seq_bindings([
        ori_ir::SeqBinding::Let {
            pattern: pattern1,
            ty: ori_ir::ParsedTypeId::INVALID,
            value: x_init,
            mutable: false,
            span: Span::DUMMY,
        },
        ori_ir::SeqBinding::Let {
            pattern: pattern2,
            ty: ori_ir::ParsedTypeId::INVALID,
            value: y_init,
            mutable: false,
            span: Span::DUMMY,
        },
    ]);

    let y_ref = alloc(&mut arena, ExprKind::Ident(name(2)));

    let func_seq = ori_ir::FunctionSeq::Run {
        pre_checks: ori_ir::CheckRange::EMPTY,
        bindings,
        result: y_ref,
        post_checks: ori_ir::CheckRange::EMPTY,
        span: Span::DUMMY,
    };
    let seq_id = arena.alloc_function_seq(func_seq);
    let expr_id = alloc(&mut arena, ExprKind::FunctionSeq(seq_id));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::STR, "run should return str from y");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_function_exp_print() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // print(value: "hello")
    let value = alloc(&mut arena, ExprKind::String(ori_ir::Name::from_raw(100)));
    let props = arena.alloc_named_exprs([ori_ir::NamedExpr {
        name: name(1), // "value"
        value,
        span: Span::DUMMY,
    }]);

    let func_exp = ori_ir::FunctionExp {
        kind: ori_ir::FunctionExpKind::Print,
        props,
        type_args: ori_ir::ParsedTypeRange::EMPTY,
        span: Span::DUMMY,
    };
    let exp_id = arena.alloc_function_exp(func_exp);
    let expr_id = alloc(&mut arena, ExprKind::FunctionExp(exp_id));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::UNIT, "print should return unit");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_function_exp_panic() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // panic(message: "oops")
    let message = alloc(&mut arena, ExprKind::String(ori_ir::Name::from_raw(100)));
    let props = arena.alloc_named_exprs([ori_ir::NamedExpr {
        name: name(1),
        value: message,
        span: Span::DUMMY,
    }]);

    let func_exp = ori_ir::FunctionExp {
        kind: ori_ir::FunctionExpKind::Panic,
        props,
        type_args: ori_ir::ParsedTypeRange::EMPTY,
        span: Span::DUMMY,
    };
    let exp_id = arena.alloc_function_exp(func_exp);
    let expr_id = alloc(&mut arena, ExprKind::FunctionExp(exp_id));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::NEVER, "panic should return never");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_function_exp_todo() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // todo() - no properties required
    let props = arena.alloc_named_exprs(std::iter::empty());

    let func_exp = ori_ir::FunctionExp {
        kind: ori_ir::FunctionExpKind::Todo,
        props,
        type_args: ori_ir::ParsedTypeRange::EMPTY,
        span: Span::DUMMY,
    };
    let exp_id = arena.alloc_function_exp(func_exp);
    let expr_id = alloc(&mut arena, ExprKind::FunctionExp(exp_id));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::NEVER, "todo should return never");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_function_exp_unreachable() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // unreachable()
    let props = arena.alloc_named_exprs(std::iter::empty());

    let func_exp = ori_ir::FunctionExp {
        kind: ori_ir::FunctionExpKind::Unreachable,
        props,
        type_args: ori_ir::ParsedTypeRange::EMPTY,
        span: Span::DUMMY,
    };
    let exp_id = arena.alloc_function_exp(func_exp);
    let expr_id = alloc(&mut arena, ExprKind::FunctionExp(exp_id));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::NEVER, "unreachable should return never");
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_function_exp_catch() {
    let interner = ori_ir::StringInterner::new();
    let expr_name = interner.intern("expr");

    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);
    let mut arena = ExprArena::new();

    // catch(expr: 42) → Result<int, str>
    let inner_expr = alloc(&mut arena, ExprKind::Int(42));

    let props = arena.alloc_named_exprs([ori_ir::NamedExpr {
        name: expr_name,
        value: inner_expr,
        span: Span::DUMMY,
    }]);

    let func_exp = ori_ir::FunctionExp {
        kind: ori_ir::FunctionExpKind::Catch,
        props,
        type_args: ori_ir::ParsedTypeRange::EMPTY,
        span: Span::DUMMY,
    };
    let exp_id = arena.alloc_function_exp(func_exp);
    let expr_id = alloc(&mut arena, ExprKind::FunctionExp(exp_id));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    // catch returns Result<T, str> where T is the expr type
    let resolved = engine.resolve(ty);
    assert_eq!(
        engine.pool().tag(resolved),
        Tag::Result,
        "catch should return Result type"
    );
    assert_eq!(
        engine.pool().result_ok(resolved),
        Idx::INT,
        "catch Result ok type should be int"
    );
    assert_eq!(
        engine.pool().result_err(resolved),
        Idx::STR,
        "catch Result err type should be str"
    );
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_function_exp_timeout() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // timeout(duration: ..., task: 42)
    let duration = alloc(&mut arena, ExprKind::Int(1000)); // milliseconds
    let task = alloc(&mut arena, ExprKind::Int(42));

    let props = arena.alloc_named_exprs([
        ori_ir::NamedExpr {
            name: name(1),
            value: duration,
            span: Span::DUMMY,
        },
        ori_ir::NamedExpr {
            name: name(2),
            value: task,
            span: Span::DUMMY,
        },
    ]);

    let func_exp = ori_ir::FunctionExp {
        kind: ori_ir::FunctionExpKind::Timeout,
        props,
        type_args: ori_ir::ParsedTypeRange::EMPTY,
        span: Span::DUMMY,
    };
    let exp_id = arena.alloc_function_exp(func_exp);
    let expr_id = alloc(&mut arena, ExprKind::FunctionExp(exp_id));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    // timeout returns Option<T>
    assert_eq!(
        engine.pool().tag(ty),
        Tag::Option,
        "timeout should return Option"
    );
    assert!(!engine.has_errors());
}

// ========================================================================
// ParsedType Resolution Tests
// ========================================================================

#[test]
fn test_resolve_primitive_int() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let arena = ExprArena::new();

    let parsed = ParsedType::Primitive(ori_ir::TypeId::INT);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::INT);
}

#[test]
fn test_resolve_primitive_void_to_unit() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let arena = ExprArena::new();

    // TypeId::VOID (6) should map to Idx::UNIT (6)
    let parsed = ParsedType::Primitive(ori_ir::TypeId::VOID);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::UNIT);
}

#[test]
fn test_resolve_primitive_duration() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let arena = ExprArena::new();

    let parsed = ParsedType::Primitive(ori_ir::TypeId::DURATION);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::DURATION);
}

#[test]
fn test_resolve_primitive_size() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let arena = ExprArena::new();

    let parsed = ParsedType::Primitive(ori_ir::TypeId::SIZE);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::SIZE);
}

#[test]
fn test_resolve_list_type() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // [int]
    let elem_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let parsed = ParsedType::List(elem_id);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(engine.pool().tag(ty), Tag::List);
    assert_eq!(engine.pool().list_elem(ty), Idx::INT);
}

#[test]
fn test_resolve_tuple_type() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // (int, str)
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let str_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::STR));
    let elems = arena.alloc_parsed_type_list([int_id, str_id]);
    let parsed = ParsedType::Tuple(elems);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(engine.pool().tag(ty), Tag::Tuple);
    let tuple_elems = engine.pool().tuple_elems(ty);
    assert_eq!(tuple_elems.len(), 2);
    assert_eq!(tuple_elems[0], Idx::INT);
    assert_eq!(tuple_elems[1], Idx::STR);
}

#[test]
fn test_resolve_function_type() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // (int, int) -> bool
    let int_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let int_id2 = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let bool_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::BOOL));
    let params = arena.alloc_parsed_type_list([int_id, int_id2]);
    let parsed = ParsedType::Function {
        params,
        ret: bool_id,
    };
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(engine.pool().tag(ty), Tag::Function);
    let fn_params = engine.pool().function_params(ty);
    assert_eq!(fn_params.len(), 2);
    assert_eq!(fn_params[0], Idx::INT);
    assert_eq!(fn_params[1], Idx::INT);
    assert_eq!(engine.pool().function_return(ty), Idx::BOOL);
}

#[test]
fn test_resolve_map_type() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let mut arena = ExprArena::new();

    // {str: int}
    let key_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::STR));
    let value_id = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let parsed = ParsedType::Map {
        key: key_id,
        value: value_id,
    };
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(engine.pool().tag(ty), Tag::Map);
    assert_eq!(engine.pool().map_key(ty), Idx::STR);
    assert_eq!(engine.pool().map_value(ty), Idx::INT);
}

#[test]
fn test_resolve_infer_creates_fresh_var() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let arena = ExprArena::new();

    let parsed = ParsedType::Infer;
    let ty1 = resolve_parsed_type(&mut engine, &arena, &parsed);
    let ty2 = resolve_parsed_type(&mut engine, &arena, &parsed);

    // Should create different fresh variables
    assert_eq!(engine.pool().tag(ty1), Tag::Var);
    assert_eq!(engine.pool().tag(ty2), Tag::Var);
    assert_ne!(ty1, ty2);
}

#[test]
fn test_resolve_self_type_creates_fresh_var() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let arena = ExprArena::new();

    let parsed = ParsedType::SelfType;
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    // For now, SelfType creates a fresh variable
    assert_eq!(engine.pool().tag(ty), Tag::Var);
}

#[test]
fn test_resolve_empty_tuple_is_unit() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    let arena = ExprArena::new();

    let parsed = ParsedType::unit();
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::UNIT);
}

// ========================================================================
// TYPECK_BUILTIN_METHODS ↔ resolve_builtin_method Consistency
// ========================================================================

/// Verify every `(type_name, method_name)` in `TYPECK_BUILTIN_METHODS` is actually
/// resolvable by `resolve_builtin_method()`. Catches drift where an entry is added
/// to the const but the corresponding resolver match arm is missing.
#[test]
fn typeck_builtin_methods_all_resolve() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    // Build concrete receiver types for container/generic types.
    // Element types are arbitrary — we only care that the resolver returns Some.
    let list_ty = engine.pool_mut().list(Idx::INT);
    let map_ty = engine.pool_mut().map(Idx::STR, Idx::INT);
    let set_ty = engine.pool_mut().set(Idx::INT);
    let option_ty = engine.pool_mut().option(Idx::INT);
    let result_ty = engine.pool_mut().result(Idx::INT, Idx::STR);
    let range_ty = engine.pool_mut().range(Idx::INT);
    let iterator_ty = engine.pool_mut().iterator(Idx::INT);
    let dei_ty = engine.pool_mut().double_ended_iterator(Idx::INT);
    let channel_ty = engine.pool_mut().channel(Idx::INT);
    let tuple_ty = engine.pool_mut().tuple(&[Idx::INT, Idx::STR]);

    let mut failures = Vec::new();

    for &(type_name, method_name) in TYPECK_BUILTIN_METHODS {
        let (tag, receiver_ty) = match type_name {
            "int" => (Tag::Int, Idx::INT),
            "float" => (Tag::Float, Idx::FLOAT),
            "bool" => (Tag::Bool, Idx::BOOL),
            "str" => (Tag::Str, Idx::STR),
            "char" => (Tag::Char, Idx::CHAR),
            "byte" => (Tag::Byte, Idx::BYTE),
            "Duration" => (Tag::Duration, Idx::DURATION),
            "Size" => (Tag::Size, Idx::SIZE),
            "Ordering" => (Tag::Ordering, Idx::ORDERING),
            "list" => (Tag::List, list_ty),
            "map" => (Tag::Map, map_ty),
            "Set" => (Tag::Set, set_ty),
            "Option" => (Tag::Option, option_ty),
            "Result" => (Tag::Result, result_ty),
            "range" => (Tag::Range, range_ty),
            "Iterator" => (Tag::Iterator, iterator_ty),
            "DoubleEndedIterator" => (Tag::DoubleEndedIterator, dei_ty),
            "Channel" => (Tag::Channel, channel_ty),
            "tuple" => (Tag::Tuple, tuple_ty),
            other => panic!("unknown type name in TYPECK_BUILTIN_METHODS: {other:?}"),
        };

        let result = resolve_builtin_method(&mut engine, receiver_ty, tag, method_name);
        if result.is_none() {
            failures.push(format!("  ({type_name}, {method_name})"));
        }
    }

    assert!(
        failures.is_empty(),
        "resolve_builtin_method() returned None for {} entries in TYPECK_BUILTIN_METHODS:\n{}",
        failures.len(),
        failures.join("\n"),
    );
}

// ========================================================================
// Trait Satisfaction Tests — Clone on compound types
// ========================================================================

#[test]
fn test_clone_satisfied_by_list() {
    let mut pool = Pool::new();
    let list_ty = pool.list(Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(list_ty, "Clone", &pool),
        "[int] should satisfy Clone"
    );
}

#[test]
fn test_clone_satisfied_by_map() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(map_ty, "Clone", &pool),
        "{{str: int}} should satisfy Clone"
    );
}

#[test]
fn test_clone_satisfied_by_set() {
    let mut pool = Pool::new();
    let set_ty = pool.set(Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(set_ty, "Clone", &pool),
        "Set<int> should satisfy Clone"
    );
}

#[test]
fn test_clone_satisfied_by_option() {
    let mut pool = Pool::new();
    let opt_ty = pool.option(Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(opt_ty, "Clone", &pool),
        "Option<int> should satisfy Clone"
    );
}

#[test]
fn test_clone_satisfied_by_result() {
    let mut pool = Pool::new();
    let res_ty = pool.result(Idx::STR, Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(res_ty, "Clone", &pool),
        "Result<str, int> should satisfy Clone"
    );
}

#[test]
fn test_clone_satisfied_by_tuple() {
    let mut pool = Pool::new();
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::STR]);

    assert!(
        super::calls::type_satisfies_trait(tuple_ty, "Clone", &pool),
        "(int, str) should satisfy Clone"
    );
}

#[test]
fn test_clone_satisfied_by_tuple_triple() {
    let mut pool = Pool::new();
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::BOOL, Idx::STR]);

    assert!(
        super::calls::type_satisfies_trait(tuple_ty, "Clone", &pool),
        "(int, bool, str) should satisfy Clone"
    );
}

#[test]
fn test_clone_not_satisfied_by_range() {
    let mut pool = Pool::new();
    let range_ty = pool.range(Idx::INT);

    assert!(
        !super::calls::type_satisfies_trait(range_ty, "Clone", &pool),
        "Range<int> should not satisfy Clone"
    );
}

// ========================================================================
// Trait Satisfaction Tests — Eq NOT satisfied by compound types
// ========================================================================
//
// Compound types (List, Map, Set, Option, Result, Tuple) do NOT satisfy Eq
// because no `.equals()` method implementation exists in downstream phases.
// Re-add when roadmap 3.14 delivers `equals()` across all phases.

#[test]
fn test_eq_not_satisfied_by_list() {
    let mut pool = Pool::new();
    let list_ty = pool.list(Idx::INT);

    assert!(
        !super::calls::type_satisfies_trait(list_ty, "Eq", &pool),
        "[int] should NOT satisfy Eq (no equals() in evaluator/codegen)"
    );
}

#[test]
fn test_eq_not_satisfied_by_map() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);

    assert!(
        !super::calls::type_satisfies_trait(map_ty, "Eq", &pool),
        "{{str: int}} should NOT satisfy Eq (no equals() in evaluator/codegen)"
    );
}

#[test]
fn test_eq_not_satisfied_by_set() {
    let mut pool = Pool::new();
    let set_ty = pool.set(Idx::INT);

    assert!(
        !super::calls::type_satisfies_trait(set_ty, "Eq", &pool),
        "Set<int> should NOT satisfy Eq (no equals() in evaluator/codegen)"
    );
}

#[test]
fn test_eq_not_satisfied_by_option() {
    let mut pool = Pool::new();
    let opt_ty = pool.option(Idx::INT);

    assert!(
        !super::calls::type_satisfies_trait(opt_ty, "Eq", &pool),
        "Option<int> should NOT satisfy Eq (no equals() in evaluator/codegen)"
    );
}

#[test]
fn test_eq_not_satisfied_by_result() {
    let mut pool = Pool::new();
    let res_ty = pool.result(Idx::STR, Idx::INT);

    assert!(
        !super::calls::type_satisfies_trait(res_ty, "Eq", &pool),
        "Result<str, int> should NOT satisfy Eq (no equals() in evaluator/codegen)"
    );
}

#[test]
fn test_eq_not_satisfied_by_tuple() {
    let mut pool = Pool::new();
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::STR]);

    assert!(
        !super::calls::type_satisfies_trait(tuple_ty, "Eq", &pool),
        "(int, str) should NOT satisfy Eq (no equals() in evaluator/codegen)"
    );
}
