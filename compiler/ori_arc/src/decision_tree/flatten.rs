//! Flatten arena-allocated `MatchPattern`s into algorithm-internal `FlatPattern`s.
//!
//! The Maranget decision tree algorithm operates on `FlatPattern` — a self-contained
//! enum with owned `Vec`s for sub-patterns. This module bridges the gap from the
//! arena-allocated `MatchPattern` (which uses `MatchPatternId` indices into `ExprArena`)
//! to the flat representation.

use ori_ir::ast::patterns::MatchPattern;
use ori_ir::ast::ExprKind;
use ori_ir::{ExprArena, ExprId, Name};

use super::FlatPattern;

/// Flatten a `MatchPattern` from the arena into a `FlatPattern`.
///
/// Recursively resolves `MatchPatternId` references via the arena and
/// converts literal `ExprId` references to concrete `FlatPattern` variants.
///
/// The `resolve_variant_index` callback provides enum variant → index mapping
/// for `Variant` patterns, since this requires type information not available
/// in the pattern itself.
// Variant indices never exceed u32.
#[allow(clippy::cast_possible_truncation)]
pub fn flatten_pattern(
    pattern: &MatchPattern,
    arena: &ExprArena,
    scrutinee_ty: ori_types::Idx,
    pool: &ori_types::Pool,
) -> FlatPattern {
    match pattern {
        MatchPattern::Wildcard => FlatPattern::Wildcard,

        MatchPattern::Binding(name) => FlatPattern::Binding(*name),

        MatchPattern::Literal(expr_id) => flatten_literal(arena, *expr_id),

        MatchPattern::Variant { name, inner } => {
            let variant_index = resolve_variant_index(pool, scrutinee_ty, *name);
            let pat_ids: Vec<_> = arena.get_match_pattern_list(*inner).to_vec();

            // Get the field types for this variant directly from the enum definition.
            let field_types = resolve_variant_field_types(pool, scrutinee_ty, variant_index);

            let fields: Vec<FlatPattern> = pat_ids
                .iter()
                .enumerate()
                .map(|(i, &pat_id)| {
                    let inner_pat = arena.get_match_pattern(pat_id);
                    let field_ty = field_types.get(i).copied().unwrap_or(ori_types::Idx::UNIT);
                    flatten_pattern(inner_pat, arena, field_ty, pool)
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
                    flatten_pattern(inner_pat, arena, elem_ty, pool)
                })
                .collect();

            FlatPattern::Tuple(elements)
        }

        MatchPattern::Struct { fields } => {
            let flat_fields: Vec<(Name, FlatPattern)> = fields
                .iter()
                .enumerate()
                .map(|(i, (field_name, sub_pat))| {
                    let field_ty = resolve_struct_field_ty(pool, scrutinee_ty, *field_name, i);
                    let flat = if let Some(pat_id) = sub_pat {
                        let inner_pat = arena.get_match_pattern(*pat_id);
                        flatten_pattern(inner_pat, arena, field_ty, pool)
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
                    flatten_pattern(inner_pat, arena, elem_ty, pool)
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
                    flatten_pattern(inner_pat, arena, scrutinee_ty, pool)
                })
                .collect();

            FlatPattern::Or(alternatives)
        }

        MatchPattern::At { name, pattern } => {
            let inner_pat = arena.get_match_pattern(*pattern);
            let inner = flatten_pattern(inner_pat, arena, scrutinee_ty, pool);
            FlatPattern::At {
                name: *name,
                inner: Box::new(inner),
            }
        }
    }
}

// ── Literal Extraction ──────────────────────────────────────────────

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

// ── Type Resolution Helpers ─────────────────────────────────────────

/// Resolve a variant name to its discriminant index.
// Variant indices never exceed u32.
#[allow(clippy::cast_possible_truncation)]
fn resolve_variant_index(
    pool: &ori_types::Pool,
    enum_ty: ori_types::Idx,
    variant_name: Name,
) -> u32 {
    use ori_types::Tag;
    let resolved = pool.resolve(enum_ty).unwrap_or(enum_ty);
    if pool.tag(resolved) == Tag::Enum {
        let count = pool.enum_variant_count(resolved);
        for i in 0..count {
            let (vname, _) = pool.enum_variant(resolved, i);
            if vname == variant_name {
                return i as u32;
            }
        }
    }
    tracing::debug!(
        ?enum_ty,
        ?variant_name,
        "could not resolve variant index in flatten"
    );
    0
}

/// Get the field types for a specific variant of an enum.
fn resolve_variant_field_types(
    pool: &ori_types::Pool,
    enum_ty: ori_types::Idx,
    variant_index: u32,
) -> Vec<ori_types::Idx> {
    use ori_types::Tag;
    let resolved = pool.resolve(enum_ty).unwrap_or(enum_ty);
    if pool.tag(resolved) == Tag::Enum {
        let count = pool.enum_variant_count(resolved);
        if (variant_index as usize) < count {
            let (_, field_types) = pool.enum_variant(resolved, variant_index as usize);
            return field_types;
        }
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
    if pool.tag(tuple_ty) == Tag::Tuple {
        let count = pool.tuple_elem_count(tuple_ty);
        if index < count {
            return pool.tuple_elem(tuple_ty, index);
        }
    }
    ori_types::Idx::UNIT
}

/// Get the type of a struct field.
fn resolve_struct_field_ty(
    pool: &ori_types::Pool,
    struct_ty: ori_types::Idx,
    field_name: Name,
    _fallback_index: usize,
) -> ori_types::Idx {
    use ori_types::Tag;
    let resolved = pool.resolve(struct_ty).unwrap_or(struct_ty);
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
    if pool.tag(list_ty) == Tag::List {
        return pool.list_elem(list_ty);
    }
    ori_types::Idx::UNIT
}
