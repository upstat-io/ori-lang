//! Flatten arena-allocated `MatchPattern`s into algorithm-internal `FlatPattern`s.
//!
//! The Maranget decision tree algorithm operates on `FlatPattern` — a self-contained
//! enum with owned `Vec`s for sub-patterns. This module bridges the gap from the
//! arena-allocated `MatchPattern` (which uses `MatchPatternId` indices into `ExprArena`)
//! to the flat representation.

use ori_ir::ast::patterns::MatchPattern;
use ori_ir::ast::ExprKind;
use ori_ir::{ExprArena, ExprId, Name, StringInterner};

use super::FlatPattern;

/// Flatten a `MatchPattern` from the arena into a `FlatPattern`.
///
/// Recursively resolves `MatchPatternId` references via the arena and
/// converts literal `ExprId` references to concrete `FlatPattern` variants.
///
/// The `interner` is needed to resolve variant names for well-known types
/// (`Option`, `Result`) which have dedicated Pool tags rather than `Tag::Enum`.
pub fn flatten_pattern(
    pattern: &MatchPattern,
    arena: &ExprArena,
    scrutinee_ty: ori_types::Idx,
    pool: &ori_types::Pool,
    interner: &StringInterner,
) -> FlatPattern {
    match pattern {
        MatchPattern::Wildcard => FlatPattern::Wildcard,

        MatchPattern::Binding(name) => FlatPattern::Binding(*name),

        MatchPattern::Literal(expr_id) => flatten_literal(arena, *expr_id),

        MatchPattern::Variant { name, inner } => {
            let variant_index = resolve_variant_index(pool, scrutinee_ty, *name, interner);
            let pat_ids: Vec<_> = arena.get_match_pattern_list(*inner).to_vec();

            // Get the field types for this variant directly from the enum definition.
            let field_types =
                resolve_variant_field_types(pool, scrutinee_ty, variant_index, *name, interner);

            let fields: Vec<FlatPattern> = pat_ids
                .iter()
                .enumerate()
                .map(|(i, &pat_id)| {
                    let inner_pat = arena.get_match_pattern(pat_id);
                    let field_ty = field_types.get(i).copied().unwrap_or(ori_types::Idx::UNIT);
                    flatten_pattern(inner_pat, arena, field_ty, pool, interner)
                })
                .collect();

            FlatPattern::Variant {
                variant_name: *name,
                variant_index,
                fields,
            }
        }

        MatchPattern::Tuple(sub_patterns) => {
            let pat_ids: Vec<_> = arena.get_match_pattern_list(*sub_patterns).to_vec();
            let elements: Vec<FlatPattern> = pat_ids
                .iter()
                .enumerate()
                .map(|(i, &pat_id)| {
                    let inner_pat = arena.get_match_pattern(pat_id);
                    let elem_ty = resolve_tuple_elem_ty(pool, scrutinee_ty, i);
                    flatten_pattern(inner_pat, arena, elem_ty, pool, interner)
                })
                .collect();

            FlatPattern::Tuple(elements)
        }

        MatchPattern::Struct { fields } => {
            let flat_fields: Vec<(Name, FlatPattern)> = fields
                .iter()
                .map(|(field_name, sub_pat)| {
                    let field_ty = resolve_struct_field_ty(pool, scrutinee_ty, *field_name);
                    let flat = if let Some(pat_id) = sub_pat {
                        let inner_pat = arena.get_match_pattern(*pat_id);
                        flatten_pattern(inner_pat, arena, field_ty, pool, interner)
                    } else {
                        // Shorthand: `{ x }` is equivalent to `{ x: x }` → binding
                        FlatPattern::Binding(*field_name)
                    };
                    (*field_name, flat)
                })
                .collect();

            FlatPattern::Struct {
                fields: flat_fields,
            }
        }

        MatchPattern::List { elements, rest } => {
            let pat_ids: Vec<_> = arena.get_match_pattern_list(*elements).to_vec();
            let elem_ty = resolve_list_elem_ty(pool, scrutinee_ty);
            let flat_elements: Vec<FlatPattern> = pat_ids
                .iter()
                .map(|&pat_id| {
                    let inner_pat = arena.get_match_pattern(pat_id);
                    flatten_pattern(inner_pat, arena, elem_ty, pool, interner)
                })
                .collect();

            FlatPattern::List {
                elements: flat_elements,
                rest: *rest,
            }
        }

        MatchPattern::Range {
            start,
            end,
            inclusive,
        } => {
            let start_val = start.map(|id| extract_int_literal(arena, id));
            let end_val = end.map(|id| extract_int_literal(arena, id));
            FlatPattern::Range {
                start: start_val,
                end: end_val,
                inclusive: *inclusive,
            }
        }

        MatchPattern::Or(sub_patterns) => {
            let pat_ids: Vec<_> = arena.get_match_pattern_list(*sub_patterns).to_vec();
            let alternatives: Vec<FlatPattern> = pat_ids
                .iter()
                .map(|&pat_id| {
                    let inner_pat = arena.get_match_pattern(pat_id);
                    flatten_pattern(inner_pat, arena, scrutinee_ty, pool, interner)
                })
                .collect();

            FlatPattern::Or(alternatives)
        }

        MatchPattern::At { name, pattern } => {
            let inner_pat = arena.get_match_pattern(*pattern);
            let inner = flatten_pattern(inner_pat, arena, scrutinee_ty, pool, interner);
            FlatPattern::At {
                name: *name,
                inner: Box::new(inner),
            }
        }
    }
}

// Literal extraction

/// Convert a literal expression to a `FlatPattern`.
fn flatten_literal(arena: &ExprArena, expr_id: ExprId) -> FlatPattern {
    let expr = arena.get_expr(expr_id);
    match &expr.kind {
        ExprKind::Int(v) => FlatPattern::LitInt(*v),
        ExprKind::Float(bits) => FlatPattern::LitFloat(*bits),
        ExprKind::Bool(v) => FlatPattern::LitBool(*v),
        ExprKind::String(name) => FlatPattern::LitStr(*name),
        ExprKind::Char(v) => FlatPattern::LitChar(*v),
        _ => {
            tracing::debug!(
                ?expr_id,
                "non-literal in pattern position, treating as wildcard"
            );
            FlatPattern::Wildcard
        }
    }
}

/// Extract an i64 from a literal expression (for range patterns).
fn extract_int_literal(arena: &ExprArena, expr_id: ExprId) -> i64 {
    let expr = arena.get_expr(expr_id);
    if let ExprKind::Int(v) = &expr.kind {
        *v
    } else {
        tracing::debug!(?expr_id, "non-int literal in range pattern");
        0
    }
}

// Type resolution helpers

/// Resolve a variant name to its discriminant index.
///
/// Handles three cases:
/// - `Tag::Enum`: looks up variant by name in the enum definition
/// - `Tag::Option`: `None` = 0, `Some` = 1
/// - `Tag::Result`: `Ok` = 0, `Err` = 1
#[expect(
    clippy::cast_possible_truncation,
    reason = "variant indices never exceed u32"
)]
fn resolve_variant_index(
    pool: &ori_types::Pool,
    enum_ty: ori_types::Idx,
    variant_name: Name,
    interner: &StringInterner,
) -> u32 {
    use ori_types::Tag;
    let resolved = pool.resolve_fully(enum_ty);
    match pool.tag(resolved) {
        Tag::Enum => {
            let count = pool.enum_variant_count(resolved);
            for i in 0..count {
                let (vname, _) = pool.enum_variant(resolved, i);
                if vname == variant_name {
                    return i as u32;
                }
            }
        }
        Tag::Option => {
            // Convention: None = 0, Some = 1 (matches evaluator's Value::Some/None)
            return u32::from(interner.lookup(variant_name) == "Some");
        }
        Tag::Result => {
            // Convention: Ok = 0, Err = 1 (matches evaluator's Value::Ok/Err)
            return u32::from(interner.lookup(variant_name) == "Err");
        }
        _ => {}
    }
    tracing::debug!(
        ?enum_ty,
        ?resolved,
        resolved_tag = ?pool.tag(resolved),
        ?variant_name,
        "could not resolve variant index in flatten"
    );
    0
}

/// Get the field types for a specific variant of an enum, Option, or Result.
///
/// - `Tag::Enum`: returns field types from the enum definition
/// - `Tag::Option`: `Some` (index 1) has one field (the inner type), `None` (index 0) has none
/// - `Tag::Result`: `Ok` (index 0) has one field (ok type), `Err` (index 1) has one field (err type)
fn resolve_variant_field_types(
    pool: &ori_types::Pool,
    enum_ty: ori_types::Idx,
    variant_index: u32,
    variant_name: Name,
    interner: &StringInterner,
) -> Vec<ori_types::Idx> {
    use ori_types::Tag;
    let resolved = pool.resolve_fully(enum_ty);
    match pool.tag(resolved) {
        Tag::Enum => {
            let count = pool.enum_variant_count(resolved);
            if (variant_index as usize) < count {
                let (_, field_types) = pool.enum_variant(resolved, variant_index as usize);
                return field_types;
            }
        }
        Tag::Option => {
            // Some has 1 field (the inner type), None has 0 fields.
            if interner.lookup(variant_name) == "Some" {
                return vec![pool.option_inner(resolved)];
            }
            return Vec::new();
        }
        Tag::Result => {
            // Ok has 1 field (ok type), Err has 1 field (err type).
            if interner.lookup(variant_name) == "Err" {
                return vec![pool.result_err(resolved)];
            }
            return vec![pool.result_ok(resolved)];
        }
        _ => {}
    }
    Vec::new()
}

/// Get the type of a tuple element.
fn resolve_tuple_elem_ty(
    pool: &ori_types::Pool,
    tuple_ty: ori_types::Idx,
    index: usize,
) -> ori_types::Idx {
    use ori_types::Tag;
    let resolved = pool.resolve_fully(tuple_ty);
    if pool.tag(resolved) == Tag::Tuple {
        let count = pool.tuple_elem_count(resolved);
        if index < count {
            return pool.tuple_elem(resolved, index);
        }
    }
    ori_types::Idx::UNIT
}

/// Get the type of a struct field.
fn resolve_struct_field_ty(
    pool: &ori_types::Pool,
    struct_ty: ori_types::Idx,
    field_name: Name,
) -> ori_types::Idx {
    use ori_types::Tag;
    let resolved = pool.resolve_fully(struct_ty);
    if pool.tag(resolved) == Tag::Struct {
        let count = pool.struct_field_count(resolved);
        for i in 0..count {
            let (fname, fty) = pool.struct_field(resolved, i);
            if fname == field_name {
                return fty;
            }
        }
    }
    ori_types::Idx::UNIT
}

/// Get the element type of a list.
fn resolve_list_elem_ty(pool: &ori_types::Pool, list_ty: ori_types::Idx) -> ori_types::Idx {
    use ori_types::Tag;
    let resolved = pool.resolve_fully(list_ty);
    if pool.tag(resolved) == Tag::List {
        return pool.list_elem(resolved);
    }
    ori_types::Idx::UNIT
}
