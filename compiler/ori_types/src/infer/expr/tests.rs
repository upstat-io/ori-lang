use super::*;
use crate::{Idx, Pool};
use ori_ir::{
    ast::{Expr, ExprKind, MapEntry, MatchArm, MatchPattern, Param, Stmt, StmtKind},
    BindingPattern, ExprArena, ExprId, Mutability, Name, Span, StringInterner,
};

// ========================================================================
// Test Helpers
// ========================================================================

/// Create test components with interner always set.
///
/// Two-arg form: `test_engine!(pool, engine)` — interner is created but not exposed.
/// Three-arg form: `test_engine!(interner, pool, engine)` — exposes interner for `intern()`.
macro_rules! test_engine {
    ($pool:ident, $engine:ident) => {
        let __test_interner = StringInterner::new();
        let mut $pool = Pool::new();
        let mut $engine = InferEngine::new(&mut $pool);
        $engine.set_interner(&__test_interner);
    };
    ($interner:ident, $pool:ident, $engine:ident) => {
        let $interner = StringInterner::new();
        let mut $pool = Pool::new();
        let mut $engine = InferEngine::new(&mut $pool);
        $engine.set_interner(&$interner);
    };
}

/// Create a Name from a raw u32 for testing.
fn name(n: u32) -> Name {
    Name::from_raw(n)
}

/// Create a dummy span for test expressions.
fn span() -> Span {
    Span::DUMMY
}

/// Allocate a simple name binding pattern for for-loop tests.
fn binding_pat(arena: &mut ExprArena, n: u32) -> ori_ir::BindingPatternId {
    arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(n),
        mutable: Mutability::Mutable,
    })
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Int(42));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_float() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Float(3_14_f64.to_bits()));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::FLOAT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_bool() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let true_id = alloc(&mut arena, ExprKind::Bool(true));
    let false_id = alloc(&mut arena, ExprKind::Bool(false));

    assert_eq!(infer_expr(&mut engine, &arena, true_id), Idx::BOOL);
    assert_eq!(infer_expr(&mut engine, &arena, false_id), Idx::BOOL);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_str() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::String(name(1)));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::STR);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_char() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Char('a'));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::CHAR);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_unit() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let expr_id = alloc(&mut arena, ExprKind::Unit);
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::UNIT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_literal_duration() {
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let entries = arena.alloc_map_entries(std::iter::empty());
    let expr_id = alloc(&mut arena, ExprKind::Map(entries));

    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(engine.pool().tag(ty), Tag::Map);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_map_with_entries() {
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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

/// Regression: BUG-02-001 — if without else with non-void then-branch
/// must produce a type error (Spec: Clause 16, §16.1).
#[test]
fn test_infer_if_without_else_non_void_then() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let cond = alloc(&mut arena, ExprKind::Bool(true));
    let then_branch = alloc(&mut arena, ExprKind::Int(42));

    let if_expr = alloc(
        &mut arena,
        ExprKind::If {
            cond,
            then_branch,
            else_branch: ExprId::INVALID,
        },
    );

    let ty = infer_expr(&mut engine, &arena, if_expr);

    assert_eq!(ty, Idx::UNIT, "if without else should have type void");
    assert!(
        engine.has_errors(),
        "Non-void then-branch without else must be a type error"
    );
}

#[test]
fn test_infer_if_branch_mismatch() {
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // Bind 'list' to [int]
    let list_ty = engine.infer_list(Idx::INT);
    engine.env_mut().bind(name(1), list_ty);

    let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
    let body = alloc(&mut arena, ExprKind::Unit);
    let pat = binding_pat(&mut arena, 2);

    let for_expr = alloc(
        &mut arena,
        ExprKind::For {
            label: Name::EMPTY,
            pattern: pat, // 'x'
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // Bind 'list' to [int]
    let list_ty = engine.infer_list(Idx::INT);
    engine.env_mut().bind(name(1), list_ty);

    let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
    // Use x (the bound element) in body
    let x_ref = alloc(&mut arena, ExprKind::Ident(name(2)));
    let pat = binding_pat(&mut arena, 2);

    let for_expr = alloc(
        &mut arena,
        ExprKind::For {
            label: Name::EMPTY,
            pattern: pat, // 'x'
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // Bind 'list' to [int]
    let list_ty = engine.infer_list(Idx::INT);
    engine.env_mut().bind(name(1), list_ty);

    let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
    let guard = alloc(&mut arena, ExprKind::Bool(true));
    let body = alloc(&mut arena, ExprKind::Unit);
    let pat = binding_pat(&mut arena, 2);

    let for_expr = alloc(
        &mut arena,
        ExprKind::For {
            label: Name::EMPTY,
            pattern: pat,
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let list_ty = engine.infer_list(Idx::INT);
    engine.env_mut().bind(name(1), list_ty);

    let iter = alloc(&mut arena, ExprKind::Ident(name(1)));
    let guard = alloc(&mut arena, ExprKind::Int(1)); // Not bool!
    let body = alloc(&mut arena, ExprKind::Unit);
    let pat = binding_pat(&mut arena, 2);

    let for_expr = alloc(
        &mut arena,
        ExprKind::For {
            label: Name::EMPTY,
            pattern: pat,
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    engine.env_mut().bind(name(1), Idx::INT);

    let expr_id = alloc(&mut arena, ExprKind::Ident(name(1)));
    let ty = infer_expr(&mut engine, &arena, expr_id);

    assert_eq!(ty, Idx::INT);
    assert!(!engine.has_errors());
}

#[test]
fn test_infer_ident_unbound() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    let unbound_name = interner.intern("nonexistent");
    let expr_id = alloc(&mut arena, ExprKind::Ident(unbound_name));
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // { let x = 42; x }
    let init = alloc(&mut arena, ExprKind::Int(42));
    let pattern = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: Mutability::Mutable,
    });
    let _stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern,
            ty: ori_ir::ParsedTypeId::INVALID,
            init,
            mutable: Mutability::Immutable,
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // { let x: int = 42; x }
    let init = alloc(&mut arena, ExprKind::Int(42));
    let pattern = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: Mutability::Mutable,
    });
    let int_ty = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT));
    let _stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern,
            ty: int_ty,
            init,
            mutable: Mutability::Immutable,
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
    test_engine!(pool, engine);
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
        mutable: Mutability::Mutable,
    });
    let list_ty = arena.alloc_parsed_type(list_annotation);
    let _stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern,
            ty: list_ty,
            init: list,
            mutable: Mutability::Immutable,
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
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // { let x: str = 42; x }
    // Type mismatch: annotated str but init is int
    let init = alloc(&mut arena, ExprKind::Int(42));
    let pattern = arena.alloc_binding_pattern(BindingPattern::Name {
        name: name(1),
        mutable: Mutability::Mutable,
    });
    let str_ty = arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::STR));
    let _stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern,
            ty: str_ty,
            init,
            mutable: Mutability::Immutable,
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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

/// COALESCE-CHAIN: `Option<T> ?? Option<T> -> Option<T>`
/// Spec: operator-rules.md §Coalesce
#[test]
fn test_infer_coalesce_option_chain() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // Bind 'a' and 'b' to Option<int>
    let opt_ty = engine.infer_option(Idx::INT);
    engine.env_mut().bind(name(1), opt_ty);
    engine.env_mut().bind(name(2), opt_ty);

    let left = alloc(&mut arena, ExprKind::Ident(name(1)));
    let right = alloc(&mut arena, ExprKind::Ident(name(2)));
    let coalesce = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Coalesce,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, coalesce);
    assert!(!engine.has_errors());

    // Drop engine to release mutable borrow on pool
    drop(engine);

    let tag = pool.tag(pool.resolve_fully(ty));
    assert_eq!(tag, Tag::Option, "Option<int> ?? Option<int> = Option<int>");
    let inner = pool.option_inner(pool.resolve_fully(ty));
    assert_eq!(
        pool.resolve_fully(inner),
        Idx::INT,
        "inner type should be int"
    );
}

/// COALESCE-RESULT-CHAIN: `Result<T,E> ?? Result<T,E> -> Result<T,E>`
/// Spec: operator-rules.md §Coalesce
#[test]
fn test_infer_coalesce_result_chain() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // Bind 'a' and 'b' to Result<int, str>
    let res_ty = engine.infer_result(Idx::INT, Idx::STR);
    engine.env_mut().bind(name(1), res_ty);
    engine.env_mut().bind(name(2), res_ty);

    let left = alloc(&mut arena, ExprKind::Ident(name(1)));
    let right = alloc(&mut arena, ExprKind::Ident(name(2)));
    let coalesce = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Coalesce,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, coalesce);
    assert!(!engine.has_errors());

    drop(engine);

    let tag = pool.tag(pool.resolve_fully(ty));
    assert_eq!(
        tag,
        Tag::Result,
        "Result<int,str> ?? Result<int,str> = Result<int,str>"
    );
    let ok_ty = pool.result_ok(pool.resolve_fully(ty));
    let err_ty = pool.result_err(pool.resolve_fully(ty));
    assert_eq!(pool.resolve_fully(ok_ty), Idx::INT, "ok type should be int");
    assert_eq!(
        pool.resolve_fully(err_ty),
        Idx::STR,
        "err type should be str"
    );
}

/// COALESCE-CHAIN with polymorphic RHS: `Option<int> ?? None -> Option<int>`
/// Regression: TPR-02-003 — chain detection must work when RHS is bare `None`
/// (type variable not yet unified before comparison).
#[test]
fn test_infer_coalesce_option_chain_bare_none() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // Bind 'a' to Option<int>
    let opt_ty = engine.infer_option(Idx::INT);
    engine.env_mut().bind(name(1), opt_ty);

    let left = alloc(&mut arena, ExprKind::Ident(name(1)));
    let right = alloc(&mut arena, ExprKind::None); // bare None → Option<_>
    let coalesce = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Coalesce,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, coalesce);
    assert!(!engine.has_errors(), "Option<int> ?? None should not error");

    drop(engine);

    let tag = pool.tag(pool.resolve_fully(ty));
    assert_eq!(
        tag,
        Tag::Option,
        "Option<int> ?? None = Option<int> (chain, not unwrap)"
    );
    let inner = pool.option_inner(pool.resolve_fully(ty));
    assert_eq!(
        pool.resolve_fully(inner),
        Idx::INT,
        "inner type should be int"
    );
}

/// Original unwrap behavior preserved: `Option<int> ?? int -> int`
/// Semantic pin: would fail if we always returned wrapper type.
#[test]
fn test_infer_coalesce_unwrap_preserved() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    let opt_ty = engine.infer_option(Idx::INT);
    engine.env_mut().bind(name(1), opt_ty);

    let left = alloc(&mut arena, ExprKind::Ident(name(1)));
    let right = alloc(&mut arena, ExprKind::Int(42));
    let coalesce = alloc(
        &mut arena,
        ExprKind::Binary {
            op: BinaryOp::Coalesce,
            left,
            right,
        },
    );

    let ty = infer_expr(&mut engine, &arena, coalesce);

    assert_eq!(ty, Idx::INT, "Option<int> ?? int still unwraps to int");
    assert!(!engine.has_errors());
}

// ========================================================================
// Pattern Expression Tests (FunctionSeq)
// ========================================================================

#[test]
fn test_infer_function_exp_print() {
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(interner, pool, engine);
    let expr_name = interner.intern("expr");
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
fn test_infer_function_exp_timeout_rejected() {
    test_engine!(pool, engine);
    let mut arena = ExprArena::new();

    // timeout is a post-0.1 concurrency feature — rejected at type checking (E2040)
    let duration = alloc(&mut arena, ExprKind::Int(1000));
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

    // Post-0.1 concurrency features return ERROR and emit E2040
    assert_eq!(ty, Idx::ERROR, "timeout should return error type");
    assert!(engine.has_errors(), "should emit unsupported feature error");
}

// ========================================================================
// ParsedType Resolution Tests
// ========================================================================

#[test]
fn test_resolve_primitive_int() {
    test_engine!(pool, engine);
    let arena = ExprArena::new();

    let parsed = ParsedType::Primitive(ori_ir::TypeId::INT);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::INT);
}

#[test]
fn test_resolve_primitive_void_to_unit() {
    test_engine!(pool, engine);
    let arena = ExprArena::new();

    // TypeId::VOID (6) should map to Idx::UNIT (6)
    let parsed = ParsedType::Primitive(ori_ir::TypeId::VOID);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::UNIT);
}

#[test]
fn test_resolve_primitive_duration() {
    test_engine!(pool, engine);
    let arena = ExprArena::new();

    let parsed = ParsedType::Primitive(ori_ir::TypeId::DURATION);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::DURATION);
}

#[test]
fn test_resolve_primitive_size() {
    test_engine!(pool, engine);
    let arena = ExprArena::new();

    let parsed = ParsedType::Primitive(ori_ir::TypeId::SIZE);
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::SIZE);
}

#[test]
fn test_resolve_list_type() {
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
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
    test_engine!(pool, engine);
    let arena = ExprArena::new();

    let parsed = ParsedType::SelfType;
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    // For now, SelfType creates a fresh variable
    assert_eq!(engine.pool().tag(ty), Tag::Var);
}

#[test]
fn test_resolve_empty_tuple_is_unit() {
    test_engine!(pool, engine);
    let arena = ExprArena::new();

    let parsed = ParsedType::unit();
    let ty = resolve_parsed_type(&mut engine, &arena, &parsed);

    assert_eq!(ty, Idx::UNIT);
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
// Trait Satisfaction Tests — Eq satisfied by compound types (§3.14)
// ========================================================================
//
// Compound types satisfy Eq because `.equals()` is implemented in the
// evaluator and type checker (delivered by roadmap §3.14).

#[test]
fn test_eq_satisfied_by_list() {
    let mut pool = Pool::new();
    let list_ty = pool.list(Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(list_ty, "Eq", &pool),
        "[int] should satisfy Eq (equals() implemented in §3.14)"
    );
}

#[test]
fn test_eq_satisfied_by_map() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(map_ty, "Eq", &pool),
        "{{str: int}} should satisfy Eq (equals() implemented in §3.14)"
    );
}

#[test]
fn test_eq_satisfied_by_set() {
    let mut pool = Pool::new();
    let set_ty = pool.set(Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(set_ty, "Eq", &pool),
        "Set<int> should satisfy Eq (equals() implemented in §3.14)"
    );
}

#[test]
fn test_eq_satisfied_by_option() {
    let mut pool = Pool::new();
    let opt_ty = pool.option(Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(opt_ty, "Eq", &pool),
        "Option<int> should satisfy Eq (equals() implemented in §3.14)"
    );
}

#[test]
fn test_eq_satisfied_by_result() {
    let mut pool = Pool::new();
    let res_ty = pool.result(Idx::STR, Idx::INT);

    assert!(
        super::calls::type_satisfies_trait(res_ty, "Eq", &pool),
        "Result<str, int> should satisfy Eq (equals() implemented in §3.14)"
    );
}

#[test]
fn test_eq_satisfied_by_tuple() {
    let mut pool = Pool::new();
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::STR]);

    assert!(
        super::calls::type_satisfies_trait(tuple_ty, "Eq", &pool),
        "(int, str) should satisfy Eq (equals() implemented in §3.14)"
    );
}

// ========================================================================
// Trait Satisfaction Tests — Len satisfied by tuple (§3.0.1)
// ========================================================================

#[test]
fn test_len_satisfied_by_tuple() {
    let mut pool = Pool::new();
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::STR]);

    assert!(
        super::calls::type_satisfies_trait(tuple_ty, "Len", &pool),
        "(int, str) should satisfy Len"
    );
}

#[test]
fn test_len_satisfied_by_triple_tuple() {
    let mut pool = Pool::new();
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::BOOL, Idx::STR]);

    assert!(
        super::calls::type_satisfies_trait(tuple_ty, "Len", &pool),
        "(int, bool, str) should satisfy Len"
    );
}

#[test]
fn test_len_satisfied_by_single_tuple() {
    let mut pool = Pool::new();
    let tuple_ty = pool.tuple(&[Idx::INT]);

    assert!(
        super::calls::type_satisfies_trait(tuple_ty, "Len", &pool),
        "(int,) should satisfy Len"
    );
}

#[test]
fn test_len_not_satisfied_by_result() {
    let mut pool = Pool::new();
    let res_ty = pool.result(Idx::INT, Idx::STR);

    assert!(
        !super::calls::type_satisfies_trait(res_ty, "Len", &pool),
        "Result<int, str> should NOT satisfy Len"
    );
}

// ========================================================================
// Infinite Iterator Detection Tests (W2001)
// ========================================================================

/// Helper: build a method call expression on a receiver.
fn method_call(
    arena: &mut ExprArena,
    interner: &ori_ir::StringInterner,
    receiver: ExprId,
    method_name: &str,
) -> ExprId {
    let method = interner.intern(method_name);
    let args = arena.alloc_expr_list_inline(&[]);
    alloc(
        arena,
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        },
    )
}

/// Helper: build a `repeat(value)` call expression.
fn repeat_call(arena: &mut ExprArena, interner: &ori_ir::StringInterner) -> ExprId {
    let repeat_name = interner.intern("repeat");
    let func = alloc(arena, ExprKind::Ident(repeat_name));
    let arg = alloc(arena, ExprKind::Int(42));
    let args = arena.alloc_expr_list_inline(&[arg]);
    alloc(arena, ExprKind::Call { func, args })
}

/// Helper: build an unbounded range `(0..)`.
fn unbounded_range(arena: &mut ExprArena) -> ExprId {
    let start = alloc(arena, ExprKind::Int(0));
    alloc(
        arena,
        ExprKind::Range {
            start,
            end: ExprId::INVALID,
            step: ExprId::INVALID,
            inclusive: false,
        },
    )
}

/// Helper: build a bounded range `(0..10)`.
fn bounded_range(arena: &mut ExprArena) -> ExprId {
    let start = alloc(arena, ExprKind::Int(0));
    let end = alloc(arena, ExprKind::Int(10));
    alloc(
        arena,
        ExprKind::Range {
            start,
            end,
            step: ExprId::INVALID,
            inclusive: false,
        },
    )
}

#[test]
fn find_infinite_source_repeat() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // repeat(42)
    let repeat = repeat_call(&mut arena, &interner);
    let result = super::calls::find_infinite_source(&engine, &arena, repeat);
    assert_eq!(result.as_deref(), Some("repeat()"));
}

#[test]
fn find_infinite_source_unbounded_range() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // (0..)
    let range = unbounded_range(&mut arena);
    let result = super::calls::find_infinite_source(&engine, &arena, range);
    assert_eq!(result.as_deref(), Some("unbounded range (start..)"));
}

#[test]
fn find_infinite_source_bounded_range_returns_none() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // (0..10) — finite, no warning
    let range = bounded_range(&mut arena);
    let result = super::calls::find_infinite_source(&engine, &arena, range);
    assert_eq!(result, None);
}

#[test]
fn find_infinite_source_cycle() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // [1,2,3].iter().cycle()
    let list_elem = alloc(&mut arena, ExprKind::Int(1));
    let list = arena.alloc_expr_list_inline(&[list_elem]);
    let list_expr = alloc(&mut arena, ExprKind::List(list));
    let iter_call = method_call(&mut arena, &interner, list_expr, "iter");
    let cycle_call = method_call(&mut arena, &interner, iter_call, "cycle");

    let result = super::calls::find_infinite_source(&engine, &arena, cycle_call);
    assert_eq!(result.as_deref(), Some("cycle()"));
}

#[test]
fn find_infinite_source_repeat_with_map_adapter() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // repeat(42).map(...)
    let repeat = repeat_call(&mut arena, &interner);
    let mapped = method_call(&mut arena, &interner, repeat, "map");

    let result = super::calls::find_infinite_source(&engine, &arena, mapped);
    assert_eq!(result.as_deref(), Some("repeat()"), "map is transparent");
}

#[test]
fn find_infinite_source_repeat_with_filter_adapter() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // repeat(42).filter(...)
    let repeat = repeat_call(&mut arena, &interner);
    let filtered = method_call(&mut arena, &interner, repeat, "filter");

    let result = super::calls::find_infinite_source(&engine, &arena, filtered);
    assert_eq!(result.as_deref(), Some("repeat()"), "filter is transparent");
}

#[test]
fn find_infinite_source_repeat_with_take_returns_none() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // repeat(42).take(10) — bounded, no warning
    let repeat = repeat_call(&mut arena, &interner);
    let taken = method_call(&mut arena, &interner, repeat, "take");

    let result = super::calls::find_infinite_source(&engine, &arena, taken);
    assert_eq!(result, None, "take bounds the iterator");
}

#[test]
fn find_infinite_source_cycle_with_take_returns_none() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // [1,2,3].iter().cycle().take(10)
    let list_elem = alloc(&mut arena, ExprKind::Int(1));
    let list = arena.alloc_expr_list_inline(&[list_elem]);
    let list_expr = alloc(&mut arena, ExprKind::List(list));
    let iter_call = method_call(&mut arena, &interner, list_expr, "iter");
    let cycle_call = method_call(&mut arena, &interner, iter_call, "cycle");
    let taken = method_call(&mut arena, &interner, cycle_call, "take");

    let result = super::calls::find_infinite_source(&engine, &arena, taken);
    assert_eq!(result, None, "take after cycle bounds the iterator");
}

#[test]
fn find_infinite_source_repeat_map_filter_enumerate() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // repeat(42).map(...).filter(...).enumerate()
    let repeat = repeat_call(&mut arena, &interner);
    let mapped = method_call(&mut arena, &interner, repeat, "map");
    let filtered = method_call(&mut arena, &interner, mapped, "filter");
    let enumerated = method_call(&mut arena, &interner, filtered, "enumerate");

    let result = super::calls::find_infinite_source(&engine, &arena, enumerated);
    assert_eq!(
        result.as_deref(),
        Some("repeat()"),
        "chain of transparent adapters still sees repeat()"
    );
}

#[test]
fn find_infinite_source_unbounded_range_iter() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // (0..).iter()
    let range = unbounded_range(&mut arena);
    let iter_call = method_call(&mut arena, &interner, range, "iter");

    let result = super::calls::find_infinite_source(&engine, &arena, iter_call);
    assert_eq!(
        result.as_deref(),
        Some("unbounded range (start..)"),
        "iter is transparent — walks through to range"
    );
}

#[test]
fn find_infinite_source_finite_list_returns_none() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // [1,2,3].iter()
    let list_elem = alloc(&mut arena, ExprKind::Int(1));
    let list = arena.alloc_expr_list_inline(&[list_elem]);
    let list_expr = alloc(&mut arena, ExprKind::List(list));
    let iter_call = method_call(&mut arena, &interner, list_expr, "iter");

    let result = super::calls::find_infinite_source(&engine, &arena, iter_call);
    assert_eq!(result, None, "finite list.iter() is not infinite");
}

#[test]
fn find_infinite_source_unknown_method_returns_none() {
    test_engine!(interner, pool, engine);
    let mut arena = ExprArena::new();

    // something.custom_method()
    let something = alloc(&mut arena, ExprKind::Int(42));
    let custom = method_call(&mut arena, &interner, something, "custom_method");

    let result = super::calls::find_infinite_source(&engine, &arena, custom);
    assert_eq!(
        result, None,
        "unknown methods are conservative — no false positives"
    );
}

// ========================================================================
// Trait Satisfaction Sync Tests — Name-based vs String-based
// ========================================================================

/// Verify that `WellKnownNames::type_satisfies_trait` (Name-based) produces
/// identical results to the string-based `calls::type_satisfies_trait` for all
/// primitive and compound types × all known trait names.
///
/// When adding a new trait to either function, this test catches drift between
/// the two implementations.
#[test]
fn well_known_trait_satisfaction_sync() {
    use crate::check::WellKnownNames;

    let interner = StringInterner::new();
    let wk = WellKnownNames::new(&interner);

    // All trait names used across both primitive and compound satisfaction checks
    let trait_names: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "FloorDiv",
        "Rem",
        "Neg",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
        "Not",
        "Len",
        "IsEmpty",
        "Iterable",
        "Iterator",
        "DoubleEndedIterator",
        // A trait name NOT in any list — should be false everywhere
        "Nonexistent",
    ];

    let primitives: &[(Idx, &str)] = &[
        (Idx::INT, "int"),
        (Idx::FLOAT, "float"),
        (Idx::BOOL, "bool"),
        (Idx::STR, "str"),
        (Idx::CHAR, "char"),
        (Idx::BYTE, "byte"),
        (Idx::UNIT, "unit"),
        (Idx::DURATION, "duration"),
        (Idx::SIZE, "size"),
        (Idx::ORDERING, "ordering"),
    ];

    let mut pool = Pool::new();

    for &(ty, type_name) in primitives {
        for &trait_str in trait_names {
            let trait_name = interner.intern(trait_str);
            let string_result = super::calls::type_satisfies_trait(ty, trait_str, &pool);
            let name_result = wk.type_satisfies_trait(ty, trait_name, &pool);
            assert_eq!(
                string_result, name_result,
                "SYNC MISMATCH: {type_name} × {trait_str}: \
                 string={string_result}, name={name_result}",
            );
        }
    }

    // Compound types (require pool construction)
    let list_ty = pool.list(Idx::INT);
    let map_ty = pool.map(Idx::STR, Idx::INT);
    let set_ty = pool.set(Idx::INT);
    let opt_ty = pool.option(Idx::INT);
    let res_ty = pool.result(Idx::STR, Idx::INT);
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::STR]);
    let range_ty = pool.range(Idx::INT);
    let iter_ty = pool.iterator(Idx::INT);
    let dei_ty = pool.double_ended_iterator(Idx::INT);

    let compounds: &[(Idx, &str)] = &[
        (list_ty, "[int]"),
        (map_ty, "{str: int}"),
        (set_ty, "Set<int>"),
        (opt_ty, "Option<int>"),
        (res_ty, "Result<str, int>"),
        (tuple_ty, "(int, str)"),
        (range_ty, "Range<int>"),
        (iter_ty, "Iterator<int>"),
        (dei_ty, "DoubleEndedIterator<int>"),
    ];

    for &(ty, type_name) in compounds {
        for &trait_str in trait_names {
            let trait_name = interner.intern(trait_str);
            let string_result = super::calls::type_satisfies_trait(ty, trait_str, &pool);
            let name_result = wk.type_satisfies_trait(ty, trait_name, &pool);
            assert_eq!(
                string_result, name_result,
                "SYNC MISMATCH: {type_name} × {trait_str}: \
                 string={string_result}, name={name_result}",
            );
        }
    }
}

// ========================================================================
// Printable Trait Satisfaction — E2038 coverage
// ========================================================================

/// Verify Printable trait satisfaction for all types relevant to string
/// interpolation. Primitives with Printable should pass; void should not.
/// Compound types (collections, wrappers, tuples) should all satisfy Printable.
#[test]
fn printable_satisfaction_primitives_and_compounds() {
    use crate::check::WellKnownNames;

    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let wk = WellKnownNames::new(&interner);
    let printable = wk.printable;

    // Primitives WITH Printable
    let printable_primitives = [
        (Idx::INT, "int"),
        (Idx::FLOAT, "float"),
        (Idx::BOOL, "bool"),
        (Idx::STR, "str"),
        (Idx::CHAR, "char"),
        (Idx::BYTE, "byte"),
        (Idx::DURATION, "Duration"),
        (Idx::SIZE, "Size"),
        (Idx::ORDERING, "Ordering"),
    ];
    for &(ty, name) in &printable_primitives {
        assert!(
            wk.type_satisfies_trait(ty, printable, &pool),
            "{name} should satisfy Printable"
        );
    }

    // Primitives WITHOUT Printable
    assert!(
        !wk.type_satisfies_trait(Idx::UNIT, printable, &pool),
        "void should NOT satisfy Printable"
    );
    assert!(
        !wk.type_satisfies_trait(Idx::NEVER, printable, &pool),
        "Never should NOT satisfy Printable (via trait check)"
    );

    // Compound types WITH Printable
    let list_ty = pool.list(Idx::INT);
    let map_ty = pool.map(Idx::STR, Idx::INT);
    let set_ty = pool.set(Idx::INT);
    let opt_ty = pool.option(Idx::INT);
    let res_ty = pool.result(Idx::INT, Idx::STR);
    let tuple_ty = pool.tuple(&[Idx::INT, Idx::STR]);
    let range_ty = pool.range(Idx::INT);

    let printable_compounds = [
        (list_ty, "[int]"),
        (map_ty, "{str: int}"),
        (set_ty, "Set<int>"),
        (opt_ty, "Option<int>"),
        (res_ty, "Result<int, str>"),
        (tuple_ty, "(int, str)"),
        (range_ty, "Range<int>"),
    ];
    for &(ty, name) in &printable_compounds {
        assert!(
            wk.type_satisfies_trait(ty, printable, &pool),
            "{name} should satisfy Printable (for interpolation)"
        );
    }
}

// ========================================================================
// Into Trait — Builtin Method Resolution (§3.17)
// ========================================================================

/// `int.into()` resolves to `float` via `resolve_builtin_method`.
#[test]
fn into_int_resolves_to_float() {
    test_engine!(pool, engine);

    let result = resolve_builtin_method(&mut engine, Idx::INT, Tag::Int, "into");
    assert_eq!(
        result,
        Some(Idx::FLOAT),
        "int.into() should return float (numeric widening)"
    );
}

/// `str.into()` resolves to `Error` — wraps string as error message.
#[test]
fn into_str_resolves_to_error() {
    test_engine!(pool, engine);

    let result = resolve_builtin_method(&mut engine, Idx::STR, Tag::Str, "into");
    assert_eq!(
        result,
        Some(Idx::ERROR),
        "str.into() should return Error (string wraps as error message)"
    );
}

/// `Set<int>.into()` resolves to `[int]` — set converts to list.
#[test]
fn into_set_resolves_to_list() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let set_ty = pool.set(Idx::INT);
    let expected_list = pool.list(Idx::INT);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let result = resolve_builtin_method(&mut engine, set_ty, Tag::Set, "into");
    assert_eq!(
        result,
        Some(expected_list),
        "Set<int>.into() should return [int]"
    );
}

/// Set's `.into()` preserves the element type parameter.
#[test]
fn into_set_preserves_element_type() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let set_ty = pool.set(Idx::STR);
    let expected_list = pool.list(Idx::STR);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let result = resolve_builtin_method(&mut engine, set_ty, Tag::Set, "into");
    assert_eq!(
        result,
        Some(expected_list),
        "Set<str>.into() should return [str], preserving element type"
    );
}

/// `Ordering.then_with()` resolves to `Ordering` via `resolve_builtin_method`.
#[test]
fn then_with_ordering_resolves_to_ordering() {
    test_engine!(pool, engine);

    let result = resolve_builtin_method(&mut engine, Idx::ORDERING, Tag::Ordering, "then_with");
    assert_eq!(
        result,
        Some(Idx::ORDERING),
        "Ordering.then_with() should return Ordering"
    );
}

/// Named (user-defined) types do NOT resolve `.into()` via builtins —
/// custom Into impls are dispatched through the `TraitRegistry`.
#[test]
fn into_not_on_named_types_via_builtins() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let user_type_name = interner.intern("Celsius");
    let user_ty = pool.named(user_type_name);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let result = resolve_builtin_method(&mut engine, user_ty, Tag::Named, "into");
    assert_eq!(
        result, None,
        "Named types should not resolve .into() via builtins (uses TraitRegistry)"
    );
}

// Registry migration validation

/// Every instance method in the registry resolves via `resolve_builtin_method`.
///
/// This is the forward-coverage test: iterates all `BUILTIN_TYPES` from the
/// registry and verifies that every non-associated method resolves to `Some(_)`.
#[test]
fn registry_method_coverage_complete() {
    use ori_registry::{TypeTag, BUILTIN_TYPES};

    test_engine!(pool, engine);

    // Build receiver types for parameterized containers
    let list_int = engine.pool_mut().list(Idx::INT);
    let map_str_int = engine.pool_mut().map(Idx::STR, Idx::INT);
    let set_int = engine.pool_mut().set(Idx::INT);
    let option_int = engine.pool_mut().option(Idx::INT);
    let result_int_err = engine.pool_mut().result(Idx::INT, Idx::ERROR);
    let range_int = engine.pool_mut().range(Idx::INT);
    let channel_int = engine.pool_mut().channel(Idx::INT);
    let iter_int = engine.pool_mut().iterator(Idx::INT);
    let dei_int = engine.pool_mut().double_ended_iterator(Idx::INT);
    let tuple_int_str = engine.pool_mut().tuple(&[Idx::INT, Idx::STR]);

    let mut failures = Vec::new();

    for type_def in BUILTIN_TYPES {
        // Map TypeTag -> (Tag, receiver Idx)
        let (tag, receiver) = match type_def.tag {
            TypeTag::Int => (Tag::Int, Idx::INT),
            TypeTag::Float => (Tag::Float, Idx::FLOAT),
            TypeTag::Bool => (Tag::Bool, Idx::BOOL),
            TypeTag::Str => (Tag::Str, Idx::STR),
            TypeTag::Char => (Tag::Char, Idx::CHAR),
            TypeTag::Byte => (Tag::Byte, Idx::BYTE),
            TypeTag::Duration => (Tag::Duration, Idx::DURATION),
            TypeTag::Size => (Tag::Size, Idx::SIZE),
            TypeTag::Ordering => (Tag::Ordering, Idx::ORDERING),
            TypeTag::Error => (Tag::Error, Idx::ERROR),
            TypeTag::List => (Tag::List, list_int),
            TypeTag::Map => (Tag::Map, map_str_int),
            TypeTag::Set => (Tag::Set, set_int),
            TypeTag::Option => (Tag::Option, option_int),
            TypeTag::Result => (Tag::Result, result_int_err),
            TypeTag::Range => (Tag::Range, range_int),
            TypeTag::Channel => (Tag::Channel, channel_int),
            TypeTag::Tuple => (Tag::Tuple, tuple_int_str),
            TypeTag::Iterator => (Tag::Iterator, iter_int),
            TypeTag::DoubleEndedIterator => (Tag::DoubleEndedIterator, dei_int),
            _ => continue,
        };

        for method in type_def.methods {
            // Skip DEI-only methods on Iterator — they're filtered by find_method
            // and only available on DoubleEndedIterator receivers
            if method.dei_only && type_def.tag == TypeTag::Iterator {
                continue;
            }

            let result = resolve_builtin_method(&mut engine, receiver, tag, method.name);
            if result.is_none() {
                failures.push(format!("{}.{}", type_def.name, method.name));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Registry methods not resolvable via resolve_builtin_method:\n  {}",
        failures.join("\n  ")
    );
}

/// `str.as_bytes()` resolves to `[byte]` via the registry.
#[test]
fn str_as_bytes_resolves() {
    test_engine!(pool, engine);

    let result = resolve_builtin_method(&mut engine, Idx::STR, Tag::Str, "as_bytes");
    assert!(result.is_some(), "str.as_bytes() should resolve");
    let ret_ty = result.unwrap();
    assert_eq!(engine.pool().tag(ret_ty), Tag::List);
    assert_eq!(engine.pool().list_elem(ret_ty), Idx::BYTE);
}

/// `str.to_bytes()` resolves to `[byte]` via the registry.
#[test]
fn str_to_bytes_resolves() {
    test_engine!(pool, engine);

    let result = resolve_builtin_method(&mut engine, Idx::STR, Tag::Str, "to_bytes");
    assert!(result.is_some(), "str.to_bytes() should resolve");
    let ret_ty = result.unwrap();
    assert_eq!(engine.pool().tag(ret_ty), Tag::List);
    assert_eq!(engine.pool().list_elem(ret_ty), Idx::BYTE);
}

/// `str.from_utf8()` resolves to `Result<str, Error>` via the registry.
///
/// Although it's an associated function (`MethodKind::Associated`), the type
/// checker resolves it through `resolve_builtin_method` — the registry does not
/// distinguish instance vs associated for return type resolution.
#[test]
fn str_from_utf8_resolves() {
    use ori_registry::{find_method, MethodKind, TypeTag};

    // Verify the registry classifies it as associated
    let method =
        find_method(TypeTag::Str, "from_utf8").expect("str.from_utf8 should exist in registry");
    assert_eq!(method.kind, MethodKind::Associated);

    // It still resolves via resolve_builtin_method for return type inference
    test_engine!(pool, engine);
    let result = resolve_builtin_method(&mut engine, Idx::STR, Tag::Str, "from_utf8");
    assert!(result.is_some(), "str.from_utf8 should resolve");
    let ret_ty = result.unwrap();
    // Returns Result<str, fresh> — str ok type, fresh error type
    assert_eq!(engine.pool().tag(ret_ty), Tag::Result);
    assert_eq!(engine.pool().result_ok(ret_ty), Idx::STR);
}

/// `is_dei_only` returns true for exactly the 5 DEI-only methods.
#[test]
fn dei_only_methods_correct() {
    let dei_methods: Vec<&str> = ori_registry::dei_only_methods().collect();

    // These are the 5 methods that are only available on DoubleEndedIterator
    let expected = ["last", "next_back", "rev", "rfind", "rfold"];
    assert_eq!(
        dei_methods.len(),
        expected.len(),
        "Expected exactly {} DEI-only methods, got {}: {:?}",
        expected.len(),
        dei_methods.len(),
        dei_methods,
    );

    for name in &expected {
        assert!(
            ori_registry::is_dei_only(name),
            "'{name}' should be DEI-only"
        );
    }

    // Verify non-DEI methods return false
    assert!(!ori_registry::is_dei_only("map"));
    assert!(!ori_registry::is_dei_only("filter"));
    assert!(!ori_registry::is_dei_only("next"));
    assert!(!ori_registry::is_dei_only("collect"));
}

/// BUG-02-003: `has_comparable_trait` returns false for Named types without
/// a trait registry (covers the unit test path; full pipeline coverage is
/// in `tests/spec/expressions/operators_comparison.ori`).
#[test]
fn test_has_comparable_returns_false_without_registry() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let user_type_name = interner.intern("MyStruct");
    let user_ty = pool.named(user_type_name);

    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    // Without trait registry, has_comparable_trait should return false
    assert!(
        !super::operators::has_comparable_trait(&engine, user_ty),
        "Named type without Comparable should not pass the check"
    );
}
