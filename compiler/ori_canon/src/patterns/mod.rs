//! Canonical pattern compilation using Maranget-style decision trees.
//!
//! Source patterns flatten into self-contained matrix cells before matrix
//! specialization produces a tree and its blank-pattern cleanup carriers.

mod decision_tree;

use ori_ir::ast::patterns::MatchPattern;
use ori_ir::canon::tree::{DecisionTree, FlatPattern, PathInstruction, PatternRow, ScrutineePath};
use ori_ir::PatternKey;

use crate::lower::Lowerer;

/// Compile match arms into a decision tree and blank-pattern cleanup carriers.
pub(crate) fn compile_patterns(
    lowerer: &Lowerer<'_>,
    arms: &[(MatchPattern, Option<ori_ir::canon::CanId>)],
    arm_range_start: u32,
    scrutinee_ty: ori_types::Idx,
) -> decision_tree::compile::CompiledDecisionTree {
    if arms.is_empty() {
        return decision_tree::compile::CompiledDecisionTree {
            tree: DecisionTree::Fail,
            leaf_discard_paths: Vec::new(),
        };
    }

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
                discard_paths: vec![],
            }
        })
        .collect();

    let paths: Vec<ScrutineePath> = vec![Vec::new()];

    decision_tree::compile::compile(matrix, paths)
}

/// Flatten an arm while resolving ambiguous bindings to unit variants.
fn flatten_arm_pattern(
    lowerer: &Lowerer<'_>,
    pattern: &MatchPattern,
    key: PatternKey,
    scrutinee_ty: ori_types::Idx,
) -> FlatPattern {
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

        // Why: Higher-order lambda inputs may lack a type-checker pattern resolution.
        if let Some(idx) = try_resolve_unit_variant(lowerer, *name, scrutinee_ty) {
            return FlatPattern::Variant {
                variant_name: *name,
                variant_index: idx,
                fields: vec![],
            };
        }
    }

    let ctx = decision_tree::flatten::FlattenCtx::new(lowerer.src, lowerer.pool, lowerer.interner);
    ctx.to_flat_pattern(pattern, scrutinee_ty)
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
/// A compiled tree plus blank-pattern cleanup carriers for `DecisionTreePool`.
pub(crate) fn compile_multi_clause_patterns(
    clauses: &[Vec<FlatPattern>],
    guards: &[Option<ori_ir::canon::CanId>],
) -> decision_tree::compile::CompiledDecisionTree {
    if clauses.is_empty() {
        return decision_tree::compile::CompiledDecisionTree {
            tree: DecisionTree::Fail,
            leaf_discard_paths: Vec::new(),
        };
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
            discard_paths: vec![],
        })
        .collect();

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

    decision_tree::compile::compile(matrix, paths)
}

/// Flatten a parameter's optional explicit pattern.
pub(crate) fn flatten_param_pattern(
    lowerer: &Lowerer<'_>,
    param: &ori_ir::ast::items::Param,
) -> FlatPattern {
    match &param.pattern {
        None => FlatPattern::Binding(param.name),
        Some(pattern) => {
            // Why: Multi-clause parameters have no scrutinee type at this stage.
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
mod tests;
