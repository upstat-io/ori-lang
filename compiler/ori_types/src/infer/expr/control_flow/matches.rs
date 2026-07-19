//! Match-expression and match-pattern inference.

use ori_ir::{ExprArena, ExprId, Name, Span};

use super::super::super::InferEngine;
use super::super::{infer_expr, lookup_struct_field_types};
use super::substitution::substitute_type_params;
use crate::{
    ContextKind, Expected, ExpectedOrigin, Idx, PatternKey, SequenceKind, Tag, TypeCheckError,
    VariantFields,
};

/// Infer the type of a match expression: unify all arm body types to one result
/// (`Never` when there are no arms).
pub(crate) fn infer_match(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    scrutinee: ExprId,
    arms: ori_ir::ArmRange,
    span: Span,
) -> Idx {
    engine.push_context(ContextKind::MatchScrutinee);
    let scrutinee_ty = infer_expr(engine, arena, scrutinee);
    engine.pop_context();

    let arms_slice = arena.get_arms(arms);

    // Empty match returns never (vacuously true that all branches agree)
    if arms_slice.is_empty() {
        return Idx::NEVER;
    }

    let mut result_ty: Option<Idx> = None;
    let scrutinee_span = arena.get_expr(scrutinee).span;

    for (i, arm) in arms_slice.iter().enumerate() {
        // Each arm gets its own type-environment scope so pattern bindings are
        // isolated to this arm's guard + body and never leak into sibling arms
        // or the enclosing scope (mirrors loops.rs / lambdas.rs / blocks.rs).
        engine.enter_scope();

        // Check pattern against scrutinee type (and bind variables)
        engine.push_context(ContextKind::MatchArmPattern { arm_index: i });
        #[expect(clippy::cast_possible_truncation, reason = "arm index fits in u32")]
        let arm_key = PatternKey::Arm(arms.start + i as u32);
        check_match_pattern(engine, arena, &arm.pattern, scrutinee_ty, arm_key, arm.span);
        engine.pop_context();

        // Check guard is bool (if present)
        if let Some(guard_id) = arm.guard {
            engine.push_context(ContextKind::MatchArmGuard { arm_index: i });
            let guard_ty = infer_expr(engine, arena, guard_id);
            let expected = Expected {
                ty: Idx::BOOL,
                origin: ExpectedOrigin::Context {
                    span: arena.get_expr(guard_id).span,
                    kind: ContextKind::MatchArmGuard { arm_index: i },
                },
            };
            let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard_id).span);
            engine.pop_context();
        }

        engine.push_context(ContextKind::MatchArm { arm_index: i });
        let body_ty = infer_expr(engine, arena, arm.body);
        engine.pop_context();

        // Exit this arm's scope: pattern bindings go out of scope here. The
        // arm-result unify below reads only the pooled `body_ty` Idx, which is
        // independent of env bindings, so exiting before the unify is sound.
        engine.exit_scope();

        // Unify with previous arms
        match result_ty {
            None => {
                // First arm establishes the result type
                result_ty = Some(body_ty);
            }
            Some(prev_ty) => {
                // Subsequent arms must match the first
                let expected = Expected {
                    ty: prev_ty,
                    origin: ExpectedOrigin::PreviousInSequence {
                        previous_span: scrutinee_span,
                        current_index: i,
                        sequence_kind: SequenceKind::MatchArms,
                    },
                };
                let _ = engine.check_type(body_ty, &expected, arena.get_expr(arm.body).span);
            }
        }
    }

    if let Some(ty) = result_ty {
        engine.resolve(ty)
    } else {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            1,
            0,
            crate::ArityMismatchKind::Pattern,
        ));
        Idx::ERROR
    }
}

/// Check a match pattern against an expected type, binding variables in the environment.
///
/// This function validates that a pattern can match values of the given type,
/// and binds any variable names introduced by the pattern.
///
/// The `pattern_key` identifies this pattern for resolution lookup. For top-level
/// arm patterns it's `PatternKey::Arm(arms.start + i)`, for nested patterns it's
/// `PatternKey::Nested(match_pattern_id.raw())`.
pub(crate) fn check_match_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::MatchPattern,
    expected_ty: Idx,
    pattern_key: PatternKey,
    span: Span,
) {
    use ori_ir::MatchPattern;

    match pattern {
        // Wildcard matches anything
        MatchPattern::Wildcard => {}

        MatchPattern::Binding(name) => {
            check_binding_pattern(engine, *name, expected_ty, pattern_key);
        }

        // Literal must have compatible type
        MatchPattern::Literal(expr_id) => {
            let lit_ty = infer_expr(engine, arena, *expr_id);
            let _ = engine.unify_types(lit_ty, expected_ty);
        }

        MatchPattern::Variant { name, inner } => {
            check_variant_pattern(engine, arena, *name, *inner, expected_ty, span);
        }

        MatchPattern::Tuple(inner) => {
            check_tuple_pattern(engine, arena, *inner, expected_ty, span);
        }

        MatchPattern::List { elements, rest } => {
            check_list_pattern(engine, arena, *elements, *rest, expected_ty, span);
        }

        MatchPattern::Struct { fields, .. } => {
            check_struct_pattern(engine, arena, fields, expected_ty, span);
        }

        // Range pattern: check bounds
        MatchPattern::Range { start, end, .. } => {
            if let Some(start_id) = start {
                let start_ty = infer_expr(engine, arena, *start_id);
                let _ = engine.unify_types(start_ty, expected_ty);
            }
            if let Some(end_id) = end {
                let end_ty = infer_expr(engine, arena, *end_id);
                let _ = engine.unify_types(end_ty, expected_ty);
            }
        }

        MatchPattern::Or(alternatives) => {
            super::or_pattern::check_or_pattern(engine, arena, *alternatives, expected_ty, span);
        }

        // At pattern: bind name and check inner pattern
        MatchPattern::At {
            name,
            pattern: inner_id,
        } => {
            engine.env_mut().bind(*name, expected_ty);
            let inner_pattern = arena.get_match_pattern(*inner_id);
            let nested_key = PatternKey::Nested(inner_id.raw());
            check_match_pattern(engine, arena, inner_pattern, expected_ty, nested_key, span);
        }
    }
}

/// Check a binding pattern: either a variable binding or an ambiguous unit variant.
///
/// The parser can't distinguish `Pending` (unit variant) from `x` (binding)
/// without type context. Resolved here by checking if the name is a unit variant
/// of the scrutinee's enum type.
fn check_binding_pattern(
    engine: &mut InferEngine<'_>,
    name: Name,
    expected_ty: Idx,
    pattern_key: PatternKey,
) {
    let resolved = engine.resolve(expected_ty);
    let tag = engine.pool().tag(resolved);

    // Check if this name is a unit variant of the scrutinee's enum type
    let is_unit_variant = if matches!(tag, Tag::Named | Tag::Applied) {
        let scrutinee_name = if tag == Tag::Named {
            engine.pool().named_name(resolved)
        } else {
            engine.pool().applied_name(resolved)
        };
        engine.type_registry().and_then(|reg| {
            let (type_entry, variant_def) = reg.lookup_variant_def(name)?;
            // INVARIANT: variant must belong to the scrutinee's type, not any enum
            if type_entry.name != scrutinee_name {
                return None;
            }
            if !variant_def.fields.is_unit() {
                return None;
            }
            let (_, variant_idx) = reg.lookup_variant(name)?;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "enums are limited to 256 variants"
            )]
            Some((type_entry.name, variant_idx as u8))
        })
    } else {
        None
    };

    if let Some((type_name, variant_index)) = is_unit_variant {
        engine.record_pattern_resolution(
            pattern_key,
            crate::PatternResolution::UnitVariant {
                type_name,
                variant_index,
            },
        );
        // Do NOT bind name — it's a constructor, not a variable
    } else {
        engine.env_mut().bind(name, expected_ty);
    }
}

/// Check a variant pattern: extract inner type if it's an enum/Option/Result.
fn check_variant_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    name: Name,
    inner: ori_ir::MatchPatternRange,
    expected_ty: Idx,
    span: Span,
) {
    let resolved = engine.resolve(expected_ty);
    let tag = engine.pool().tag(resolved);
    let inner_ids = arena.get_match_pattern_list(inner);

    // Resolve the inner field types for each variant payload
    let inner_types = match tag {
        Tag::Option => {
            vec![engine.pool().option_inner(resolved)]
        }
        Tag::Result => {
            let variant_str = engine.lookup_name(name);
            match variant_str {
                Some("Err") => vec![engine.pool().result_err(resolved)],
                _ => vec![engine.pool().result_ok(resolved)],
            }
        }
        Tag::Named | Tag::Applied => {
            resolve_variant_fields(engine, name, resolved, inner_ids.len())
        }
        _ => inner_ids.iter().map(|_| engine.fresh_var()).collect(),
    };

    // Check inner patterns
    for (inner_id, inner_ty) in inner_ids.iter().zip(inner_types.iter()) {
        let inner_pattern = arena.get_match_pattern(*inner_id);
        let nested_key = PatternKey::Nested(inner_id.raw());
        check_match_pattern(engine, arena, inner_pattern, *inner_ty, nested_key, span);
    }
}

/// Check a tuple pattern: verify arity and check each element.
fn check_tuple_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ori_ir::MatchPatternRange,
    expected_ty: Idx,
    span: Span,
) {
    let resolved = engine.resolve(expected_ty);

    if engine.pool().tag(resolved) == Tag::Tuple {
        let elem_types = engine.pool().tuple_elems(resolved);
        let inner_ids = arena.get_match_pattern_list(inner);

        // Check arity
        if inner_ids.len() != elem_types.len() {
            engine.push_error(TypeCheckError::arity_mismatch(
                span,
                elem_types.len(),
                inner_ids.len(),
                crate::ArityMismatchKind::Pattern,
            ));
            return;
        }

        // Check each element
        for (inner_id, elem_ty) in inner_ids.iter().zip(elem_types.iter()) {
            let inner_pattern = arena.get_match_pattern(*inner_id);
            let nested_key = PatternKey::Nested(inner_id.raw());
            check_match_pattern(engine, arena, inner_pattern, *elem_ty, nested_key, span);
        }
    } else if resolved != Idx::ERROR {
        // Not a tuple type
        engine.push_error(TypeCheckError::mismatch(
            span,
            expected_ty,
            resolved,
            vec![],
            crate::ErrorContext::new(ContextKind::PatternMatch {
                pattern_kind: "tuple",
            }),
        ));
    }
}

/// Check a list pattern: check element patterns and bind the rest pattern.
fn check_list_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::MatchPatternRange,
    rest: Option<Name>,
    expected_ty: Idx,
    span: Span,
) {
    let resolved = engine.resolve(expected_ty);

    if engine.pool().tag(resolved) == Tag::List {
        let elem_ty = engine.pool().list_elem(resolved);
        let elem_ids = arena.get_match_pattern_list(elements);

        // Check each element pattern
        for inner_id in elem_ids {
            let inner_pattern = arena.get_match_pattern(*inner_id);
            let nested_key = PatternKey::Nested(inner_id.raw());
            check_match_pattern(engine, arena, inner_pattern, elem_ty, nested_key, span);
        }

        // Bind rest pattern to list type
        if let Some(rest_name) = rest {
            engine.env_mut().bind(rest_name, resolved);
        }
    } else if resolved != Idx::ERROR {
        // Not a list type
        engine.push_error(TypeCheckError::mismatch(
            span,
            expected_ty,
            resolved,
            vec![],
            crate::ErrorContext::new(ContextKind::PatternMatch {
                pattern_kind: "list",
            }),
        ));
    }
}

/// Check a struct pattern: resolve field types from the registry and check each field.
fn check_struct_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    fields: &[(Name, Option<ori_ir::MatchPatternId>)],
    expected_ty: Idx,
    span: Span,
) {
    let resolved = engine.resolve(expected_ty);
    let field_type_map = match engine.pool().tag(resolved) {
        Tag::Named => {
            let type_name = engine.pool().named_name(resolved);
            lookup_struct_field_types(engine, type_name, None)
        }
        Tag::Applied => {
            let type_name = engine.pool().applied_name(resolved);
            let type_args = engine.pool().applied_args(resolved);
            lookup_struct_field_types(engine, type_name, Some(&type_args))
        }
        _ => None,
    };

    for (name, inner_pattern) in fields {
        let field_ty = field_type_map
            .as_ref()
            .and_then(|m| m.get(name).copied())
            .unwrap_or_else(|| engine.fresh_var());
        if let Some(inner_id) = inner_pattern {
            let inner = arena.get_match_pattern(*inner_id);
            let nested_key = PatternKey::Nested(inner_id.raw());
            check_match_pattern(engine, arena, inner, field_ty, nested_key, span);
        } else {
            // Shorthand: `{ x }` binds x to the field value
            engine.env_mut().bind(*name, field_ty);
        }
    }
}

/// Resolve the field types for a user-defined enum variant in a match pattern.
///
/// Looks up the variant in the `TypeRegistry`, extracts field types, and substitutes
/// any generic type parameters with concrete type arguments from the scrutinee.
/// Falls back to fresh type variables when the variant is not found or type argument
/// counts don't match.
fn resolve_variant_fields(
    engine: &mut InferEngine<'_>,
    variant_name: Name,
    resolved_scrutinee: Idx,
    fallback_count: usize,
) -> Vec<Idx> {
    let result = engine.type_registry().and_then(|reg| {
        let (type_entry, variant_def) = reg.lookup_variant_def(variant_name)?;
        let field_types: Vec<Idx> = match &variant_def.fields {
            VariantFields::Unit => vec![],
            VariantFields::Tuple(types) => types.clone(),
            VariantFields::Record(fields) => fields.iter().map(|f| f.ty).collect(),
        };
        Some((type_entry.type_params.clone(), field_types))
    });

    match result {
        Some((type_params, field_types)) if type_params.is_empty() => {
            // Non-generic enum: field types are concrete
            field_types
        }
        Some((type_params, field_types)) => {
            // Generic enum: substitute type parameters with concrete type arguments
            let type_args = if engine.pool().tag(resolved_scrutinee) == Tag::Applied {
                engine.pool().applied_args(resolved_scrutinee)
            } else {
                vec![]
            };

            if type_args.len() == type_params.len() {
                field_types
                    .iter()
                    .map(|&ft| substitute_type_params(engine, ft, &type_params, &type_args))
                    .collect()
            } else {
                // Type arg count mismatch — fall back to fresh variables
                (0..fallback_count).map(|_| engine.fresh_var()).collect()
            }
        }
        None => {
            // Variant not found — fall back to fresh variables
            (0..fallback_count).map(|_| engine.fresh_var()).collect()
        }
    }
}
