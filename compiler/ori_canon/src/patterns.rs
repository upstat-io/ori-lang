//! Pattern compilation — match patterns to decision trees.
//!
//! Called by `lower.rs` when lowering `ExprKind::Match`. Converts Ori's
//! `MatchPattern` variants to the flattened `FlatPattern` representation,
//! then compiles the pattern matrix to a `DecisionTree` using the Maranget
//! (2008) algorithm.
//!
//! The compiled tree is stored in `DecisionTreePool` and referenced by
//! `DecisionTreeId` on the `CanExpr::Match` node.
//!
//! # Strategy
//!
//! This module delegates to existing infrastructure in `ori_arc::decision_tree`:
//! - `flatten::flatten_pattern()` — arena `MatchPattern` → self-contained `FlatPattern`
//! - `compile::compile()` — `PatternMatrix` → `DecisionTree`
//!
//! No algorithm duplication. Section 07 may move the algorithm here if needed.
//!
//! # Prior Art
//!
//! - Maranget (2008) "Compiling Pattern Matching to Good Decision Trees"
//! - Roc `crates/compiler/mono/src/ir/decision_tree.rs`
//! - Elm `compiler/src/Optimize/DecisionTree.hs`
//!
//! See `eval_v2` Section 03 for the full pattern compilation specification.

use ori_ir::ast::patterns::MatchPattern;
use ori_ir::canon::tree::{DecisionTree, FlatPattern, PatternRow, ScrutineePath};
use ori_types::PatternKey;

use crate::lower::Lowerer;

/// Compile match arm patterns into a decision tree.
///
/// Converts each arm's `MatchPattern` into a `FlatPattern` via
/// `ori_arc::decision_tree::flatten`, then builds a `PatternMatrix`
/// and compiles it with the Maranget algorithm.
///
/// # Arguments
///
/// - `lowerer`: The active lowerer (provides source arena, type info, and pool).
/// - `arms`: Pattern + optional guard for each match arm.
/// - `arm_range_start`: The `ArmRange.start` value, needed to construct `PatternKey::Arm`
///   for resolving ambiguous bindings via `TypedModule::resolve_pattern()`.
/// - `scrutinee_ty`: The type of the scrutinee expression (for variant resolution).
///
/// # Returns
///
/// A compiled `DecisionTree` ready for storage in the `DecisionTreePool`.
pub(crate) fn compile_patterns(
    lowerer: &Lowerer<'_>,
    arms: &[(MatchPattern, Option<ori_ir::ExprId>)],
    arm_range_start: u32,
    scrutinee_ty: ori_types::Idx,
) -> DecisionTree {
    if arms.is_empty() {
        return DecisionTree::Fail;
    }

    // Build the pattern matrix: one row per arm, one column (the scrutinee).
    let matrix: Vec<PatternRow> = arms
        .iter()
        .enumerate()
        .map(|(arm_index, (pattern, guard))| {
            #[allow(clippy::cast_possible_truncation)] // arm count always fits u32
            let key = PatternKey::Arm(arm_range_start + arm_index as u32);
            let flat = flatten_arm_pattern(lowerer, pattern, key, scrutinee_ty);
            PatternRow {
                patterns: vec![flat],
                arm_index,
                guard: *guard,
            }
        })
        .collect();

    // Initial paths: one column = root scrutinee at empty path.
    let paths: Vec<ScrutineePath> = vec![Vec::new()];

    ori_arc::decision_tree::compile::compile(matrix, paths)
}

/// Flatten a single arm pattern, handling `PatternResolution::UnitVariant`.
///
/// The type checker may resolve ambiguous `Binding` patterns to `UnitVariant`
/// (e.g., `None` looks like a variable name but is actually an enum variant
/// with no fields). We check `typed.resolve_pattern()` and convert these to
/// `FlatPattern::Variant` with the resolved index before passing to
/// `flatten_pattern()`.
fn flatten_arm_pattern(
    lowerer: &Lowerer<'_>,
    pattern: &MatchPattern,
    key: PatternKey,
    scrutinee_ty: ori_types::Idx,
) -> FlatPattern {
    // Check if this Binding pattern was resolved to a unit variant by the type checker.
    if let MatchPattern::Binding(name) = pattern {
        if let Some(ori_types::PatternResolution::UnitVariant { variant_index, .. }) =
            lowerer.typed.resolve_pattern(key)
        {
            return FlatPattern::Variant {
                variant_name: *name,
                variant_index: u32::from(*variant_index),
                fields: vec![],
            };
        }
    }

    ori_arc::decision_tree::flatten::flatten_pattern(
        pattern,
        lowerer.src,
        scrutinee_ty,
        lowerer.pool,
    )
}

#[cfg(test)]
mod tests {
    use ori_ir::ast::patterns::MatchPattern;
    use ori_ir::ast::Expr;
    use ori_ir::canon::tree::{DecisionTree, TestKind};
    use ori_ir::{ExprArena, ExprKind, SharedInterner, Span};
    use ori_types::{Idx, TypeCheckResult, TypedModule};

    use crate::lower;

    // ── Helpers ─────────────────────────────────────────────────

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

    // ── Tests ───────────────────────────────────────────────────

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
}
