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
use ori_ir::canon::tree::{DecisionTree, FlatPattern, PathInstruction, PatternRow, ScrutineePath};
use ori_ir::PatternKey;

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
    arms: &[(MatchPattern, Option<ori_ir::canon::CanId>)],
    arm_range_start: u32,
    scrutinee_ty: ori_types::Idx,
) -> DecisionTree {
    if arms.is_empty() {
        return DecisionTree::Fail;
    }

    // Build the pattern matrix: one row per arm, one column (the scrutinee).
    // Guards are already lowered to CanId by the caller.
    let matrix: Vec<PatternRow> = arms
        .iter()
        .enumerate()
        .map(|(arm_index, (pattern, guard))| {
            #[expect(clippy::cast_possible_truncation, reason = "arm count always fits u32")]
            let key = PatternKey::Arm(arm_range_start + arm_index as u32);
            let flat = flatten_arm_pattern(lowerer, pattern, key, scrutinee_ty);
            PatternRow {
                patterns: vec![flat],
                arm_index,
                guard: *guard,
                bindings: vec![],
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
///
/// When the type checker lacks resolution (e.g., untyped lambda parameters in
/// higher-order methods like `.map()`/`.fold()`), we fall back to pool-based
/// resolution: if the binding name starts with uppercase and is a variant of
/// the scrutinee's enum type, treat it as a variant pattern. This mirrors the
/// legacy evaluator's value-based fallback in `try_match`.
fn flatten_arm_pattern(
    lowerer: &Lowerer<'_>,
    pattern: &MatchPattern,
    key: PatternKey,
    scrutinee_ty: ori_types::Idx,
) -> FlatPattern {
    // Check if this Binding pattern was resolved to a unit variant by the type checker.
    if let MatchPattern::Binding(name) = pattern {
        if let Some(ori_ir::PatternResolution::UnitVariant { variant_index, .. }) =
            lowerer.typed.resolve_pattern(key)
        {
            return FlatPattern::Variant {
                variant_name: *name,
                variant_index: u32::from(*variant_index),
                fields: vec![],
            };
        }

        // Fallback: resolve uppercase binding names as unit variants via the pool.
        // Handles cases where the type checker lacks resolution (e.g., lambda
        // parameters in higher-order methods where element types aren't propagated).
        if let Some(idx) = try_resolve_unit_variant(lowerer, *name, scrutinee_ty) {
            return FlatPattern::Variant {
                variant_name: *name,
                variant_index: idx,
                fields: vec![],
            };
        }
    }

    ori_arc::decision_tree::flatten::flatten_pattern(
        pattern,
        lowerer.src,
        scrutinee_ty,
        lowerer.pool,
        lowerer.interner,
    )
}

/// Compile multi-clause function parameter patterns into a decision tree.
///
/// Each clause contributes one row. Each parameter contributes one column.
/// The scrutinee is either a single value (1 param) or a tuple (N params).
///
/// # Arguments
///
/// - `lowerer`: The active lowerer.
/// - `clauses`: Parameter patterns for each clause (each inner Vec is one clause's params).
/// - `guards`: Optional guard `CanId` for each clause.
///
/// # Returns
///
/// A compiled `DecisionTree` ready for storage in the `DecisionTreePool`.
pub(crate) fn compile_multi_clause_patterns(
    clauses: &[Vec<FlatPattern>],
    guards: &[Option<ori_ir::canon::CanId>],
) -> DecisionTree {
    if clauses.is_empty() {
        return DecisionTree::Fail;
    }

    let col_count = clauses[0].len();

    let matrix: Vec<PatternRow> = clauses
        .iter()
        .zip(guards.iter())
        .enumerate()
        .map(|(arm_index, (patterns, guard))| PatternRow {
            patterns: patterns.clone(),
            arm_index,
            guard: *guard,
            bindings: vec![],
        })
        .collect();

    // Initial paths: for single-column, root at empty path.
    // For multi-column, each column projects via TupleIndex.
    let paths: Vec<ScrutineePath> = if col_count == 1 {
        vec![Vec::new()]
    } else {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "param count always fits u32"
        )]
        (0..col_count)
            .map(|i| vec![PathInstruction::TupleIndex(i as u32)])
            .collect()
    };

    ori_arc::decision_tree::compile::compile(matrix, paths)
}

/// Flatten a function parameter pattern into a `FlatPattern`.
///
/// Handles `Option<MatchPattern>` from `Param.pattern`:
/// - `None` → binding (the parameter name)
/// - `Some(pattern)` → flatten via the standard pipeline
pub(crate) fn flatten_param_pattern(
    lowerer: &Lowerer<'_>,
    param: &ori_ir::ast::items::Param,
) -> FlatPattern {
    match &param.pattern {
        None => FlatPattern::Binding(param.name),
        Some(pattern) => {
            // Use UNIT type as scrutinee_ty since multi-clause functions lack type info.
            // This works for all literal patterns (Int, String, Bool) which don't need
            // type resolution, and for variant patterns via the registry-based fallback.
            flatten_arm_pattern(lowerer, pattern, PatternKey::Arm(0), ori_types::Idx::UNIT)
        }
    }
}

/// Try to resolve a binding name as a unit variant of the scrutinee type.
///
/// Two resolution strategies:
///
/// 1. **Pool-based**: If `scrutinee_ty` resolves to an enum in the pool, check
///    if the name matches a variant of that enum.
///
/// 2. **Registry-based fallback**: If `scrutinee_ty` is unresolved (e.g., a type
///    variable from an untyped lambda parameter), search the module's type
///    definitions for any enum with a matching unit variant. This handles cases
///    where the type checker couldn't resolve the scrutinee type because the lambda
///    parameter wasn't unified with the concrete element type during inference.
///
/// Returns the variant index if found, `None` otherwise.
fn try_resolve_unit_variant(
    lowerer: &Lowerer<'_>,
    name: ori_ir::Name,
    scrutinee_ty: ori_types::Idx,
) -> Option<u32> {
    use ori_types::TypeKind;

    let name_str = lowerer.interner.lookup(name);
    if !name_str.starts_with(char::is_uppercase) {
        return None;
    }

    // Strategy 1: Pool-based resolution when scrutinee type is known.
    let resolved = lowerer.pool.resolve_fully(scrutinee_ty);
    if lowerer.pool.tag(resolved) == ori_types::Tag::Enum {
        let count = lowerer.pool.enum_variant_count(resolved);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "enum variant count bounded by u8 (max 256)"
        )]
        for i in 0..count {
            let (vname, _) = lowerer.pool.enum_variant(resolved, i);
            if vname == name {
                return Some(i as u32);
            }
        }
        return None;
    }

    // Strategy 2: Registry-based fallback when scrutinee type is unresolved.
    // Search the module's exported type definitions for any enum with a
    // matching unit variant. This mirrors TypeRegistry::lookup_variant_def().
    for type_entry in &lowerer.typed.types {
        if let TypeKind::Enum { variants } = &type_entry.kind {
            for (i, variant) in variants.iter().enumerate() {
                if variant.name == name && variant.fields.is_unit() {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "enum variant count bounded by u8 (max 256)"
                    )]
                    return Some(i as u32);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
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
}
