//! Match pattern binding extraction.
//!
//! Extracts variable bindings from match patterns and unifies pattern structure
//! with scrutinee types.

use crate::ir::{Name, Span, MatchPattern};
use crate::types::Type;
use crate::typeck::type_registry::{TypeKind, VariantDef};
use super::super::checker::{TypeChecker, TypeCheckError};
use super::infer_expr;
use std::collections::HashSet;

/// Extract variable bindings from a match pattern given the scrutinee type.
///
/// Returns a list of (name, type) pairs for all bindings introduced by the pattern.
/// This handles nested patterns recursively.
pub fn extract_match_pattern_bindings(
    checker: &mut TypeChecker<'_>,
    pattern: &MatchPattern,
    scrutinee_ty: &Type,
) -> Vec<(Name, Type)> {
    let resolved_ty = checker.inference.ctx.resolve(scrutinee_ty);

    match pattern {
        // Patterns that introduce no bindings
        MatchPattern::Wildcard | MatchPattern::Literal(_) | MatchPattern::Range { .. } => vec![],

        MatchPattern::Binding(name) => {
            vec![(*name, resolved_ty.clone())]
        }

        MatchPattern::Variant { name, inner } => {
            // Look up the variant type to get the inner type
            let inner_ty = get_variant_inner_type(checker, &resolved_ty, *name);

            if let Some(inner_pattern) = inner {
                extract_match_pattern_bindings(checker, inner_pattern, &inner_ty)
            } else {
                vec![]
            }
        }

        MatchPattern::Struct { fields } => {
            // Look up struct field types
            let field_types = get_struct_field_types(checker, &resolved_ty);

            let mut bindings = Vec::new();
            for (field_name, opt_pattern) in fields {
                let field_ty = field_types
                    .iter()
                    .find(|(n, _)| n == field_name).map_or_else(|| checker.inference.ctx.fresh_var(), |(_, ty)| ty.clone());

                match opt_pattern {
                    Some(nested) => {
                        bindings.extend(extract_match_pattern_bindings(checker, nested, &field_ty));
                    }
                    None => {
                        // Shorthand field: { x } binds x to the field value
                        bindings.push((*field_name, field_ty));
                    }
                }
            }
            bindings
        }

        MatchPattern::Tuple(patterns) => {
            // Get element types from tuple type
            let elem_types = match &resolved_ty {
                Type::Tuple(elems) => elems.clone(),
                _ => vec![checker.inference.ctx.fresh_var(); patterns.len()],
            };

            let mut bindings = Vec::new();
            for (pattern, ty) in patterns.iter().zip(elem_types.iter()) {
                bindings.extend(extract_match_pattern_bindings(checker, pattern, ty));
            }
            bindings
        }

        MatchPattern::List { elements, rest } => {
            // Get element type from list type
            let elem_ty = match &resolved_ty {
                Type::List(elem) => (**elem).clone(),
                _ => checker.inference.ctx.fresh_var(),
            };

            let mut bindings = Vec::new();
            for pattern in elements {
                bindings.extend(extract_match_pattern_bindings(checker, pattern, &elem_ty));
            }

            // Rest pattern binds to the remaining list
            if let Some(rest_name) = rest {
                bindings.push((*rest_name, Type::List(Box::new(elem_ty))));
            }

            bindings
        }

        MatchPattern::Or(patterns) => {
            // For or-patterns, all branches must bind the same names with compatible types.
            // We take bindings from the first branch.
            if let Some(first) = patterns.first() {
                extract_match_pattern_bindings(checker, first, &resolved_ty)
            } else {
                vec![]
            }
        }

        MatchPattern::At { name, pattern } => {
            // Bind the name to the scrutinee type
            let mut bindings = vec![(*name, resolved_ty.clone())];
            // Also extract bindings from the inner pattern
            bindings.extend(extract_match_pattern_bindings(checker, pattern, &resolved_ty));
            bindings
        }
    }
}

/// Get the inner type for a variant pattern (Some, Ok, Err, etc.).
fn get_variant_inner_type(
    checker: &mut TypeChecker<'_>,
    scrutinee_ty: &Type,
    variant_name: Name,
) -> Type {
    let variant_str = checker.context.interner.lookup(variant_name);

    match variant_str {
        "Some" => {
            match scrutinee_ty {
                Type::Option(inner) => (**inner).clone(),
                _ => checker.inference.ctx.fresh_var(),
            }
        }
        "None" => Type::Unit,
        "Ok" => {
            match scrutinee_ty {
                Type::Result { ok, .. } => (**ok).clone(),
                _ => checker.inference.ctx.fresh_var(),
            }
        }
        "Err" => {
            match scrutinee_ty {
                Type::Result { err, .. } => (**err).clone(),
                _ => checker.inference.ctx.fresh_var(),
            }
        }
        _ => {
            // User-defined enum variant
            if let Type::Named(type_name) = scrutinee_ty {
                if let Some(entry) = checker.registries.types.get_by_name(*type_name) {
                    if let TypeKind::Enum { variants } = &entry.kind {
                        for variant in variants {
                            if variant.name == variant_name {
                                return get_variant_field_type(variant);
                            }
                        }
                    }
                }
            }
            checker.inference.ctx.fresh_var()
        }
    }
}

/// Get the field type for a variant.
/// For single-field variants, returns that field's type.
/// For multi-field variants, returns a tuple of field types.
/// For unit variants, returns Unit.
fn get_variant_field_type(variant: &VariantDef) -> Type {
    match variant.fields.len() {
        0 => Type::Unit,
        1 => variant.fields[0].1.clone(),
        _ => Type::Tuple(variant.fields.iter().map(|(_, ty)| ty.clone()).collect()),
    }
}

/// Get field types for a struct pattern.
fn get_struct_field_types(
    checker: &mut TypeChecker<'_>,
    scrutinee_ty: &Type,
) -> Vec<(Name, Type)> {
    if let Type::Named(type_name) = scrutinee_ty {
        if let Some(entry) = checker.registries.types.get_by_name(*type_name) {
            if let TypeKind::Struct { fields } = &entry.kind {
                return fields.clone();
            }
        }
    }
    vec![]
}

/// Unify a pattern's structure with the scrutinee type.
///
/// Reports type errors if the pattern structure doesn't match the scrutinee.
pub fn unify_pattern_with_scrutinee(
    checker: &mut TypeChecker<'_>,
    pattern: &MatchPattern,
    scrutinee_ty: &Type,
    span: Span,
) {
    let resolved_ty = checker.inference.ctx.resolve(scrutinee_ty);

    match pattern {
        MatchPattern::Wildcard | MatchPattern::Binding(_) => {
            // These match any type
        }

        MatchPattern::Literal(expr_id) => {
            // Unify the literal's type with scrutinee type
            let literal_ty = infer_expr(checker, *expr_id);
            if let Err(e) = checker.inference.ctx.unify(&literal_ty, &resolved_ty) {
                checker.report_type_error(&e, span);
            }
        }

        MatchPattern::Variant { name, inner } => {
            // Check that the scrutinee type can have this variant
            let variant_str = checker.context.interner.lookup(*name);

            let valid_variant = match variant_str {
                "Some" | "None" => matches!(resolved_ty, Type::Option(_) | Type::Var(_)),
                "Ok" | "Err" => matches!(resolved_ty, Type::Result { .. } | Type::Var(_)),
                _ => {
                    // User-defined enum
                    if let Type::Named(type_name) = &resolved_ty {
                        if let Some(entry) = checker.registries.types.get_by_name(*type_name) {
                            if let TypeKind::Enum { variants } = &entry.kind {
                                variants.iter().any(|v| v.name == *name)
                            } else {
                                false
                            }
                        } else {
                            true // Unknown type, assume ok
                        }
                    } else {
                        matches!(resolved_ty, Type::Var(_))
                    }
                }
            };

            if !valid_variant && !matches!(resolved_ty, Type::Error) {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!(
                        "pattern `{}` is not a valid variant for type `{}`",
                        variant_str,
                        resolved_ty.display(checker.context.interner)
                    ),
                    span,
                    code: crate::diagnostic::ErrorCode::E2001,
                });
            }

            // Recursively check inner pattern
            if let Some(inner_pattern) = inner {
                let inner_ty = get_variant_inner_type(checker, &resolved_ty, *name);
                unify_pattern_with_scrutinee(checker, inner_pattern, &inner_ty, span);
            }
        }

        MatchPattern::Struct { fields } => {
            // Check that scrutinee is a struct with matching fields
            let field_types = get_struct_field_types(checker, &resolved_ty);

            for (field_name, opt_pattern) in fields {
                let field_ty = field_types
                    .iter()
                    .find(|(n, _)| n == field_name)
                    .map(|(_, ty)| ty.clone());

                if field_ty.is_none() && !matches!(resolved_ty, Type::Var(_) | Type::Error) {
                    let field_str = checker.context.interner.lookup(*field_name);
                    checker.diagnostics.errors.push(TypeCheckError {
                        message: format!(
                            "type `{}` has no field `{}`",
                            resolved_ty.display(checker.context.interner),
                            field_str
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2001,
                    });
                }

                if let (Some(nested), Some(ty)) = (opt_pattern, field_ty) {
                    unify_pattern_with_scrutinee(checker, nested, &ty, span);
                }
            }
        }

        MatchPattern::Tuple(patterns) => {
            match &resolved_ty {
                Type::Tuple(elems) => {
                    if patterns.len() != elems.len() {
                        checker.diagnostics.errors.push(TypeCheckError {
                            message: format!(
                                "tuple pattern has {} elements but scrutinee has {}",
                                patterns.len(),
                                elems.len()
                            ),
                            span,
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                    }

                    for (pattern, ty) in patterns.iter().zip(elems.iter()) {
                        unify_pattern_with_scrutinee(checker, pattern, ty, span);
                    }
                }
                Type::Var(_) => {
                    // Create a tuple type with fresh variables
                    let elem_types: Vec<Type> = patterns.iter()
                        .map(|_| checker.inference.ctx.fresh_var())
                        .collect();
                    let tuple_ty = Type::Tuple(elem_types.clone());
                    if let Err(e) = checker.inference.ctx.unify(&resolved_ty, &tuple_ty) {
                        checker.report_type_error(&e, span);
                    }

                    for (pattern, ty) in patterns.iter().zip(elem_types.iter()) {
                        unify_pattern_with_scrutinee(checker, pattern, ty, span);
                    }
                }
                Type::Error => {}
                _ => {
                    checker.diagnostics.errors.push(TypeCheckError {
                        message: format!(
                            "tuple pattern cannot match type `{}`",
                            resolved_ty.display(checker.context.interner)
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2001,
                    });
                }
            }
        }

        MatchPattern::List { elements, rest: _ } => {
            match &resolved_ty {
                Type::List(elem) => {
                    for pattern in elements {
                        unify_pattern_with_scrutinee(checker, pattern, elem, span);
                    }
                }
                Type::Var(_) => {
                    let elem_ty = checker.inference.ctx.fresh_var();
                    let list_ty = Type::List(Box::new(elem_ty.clone()));
                    if let Err(e) = checker.inference.ctx.unify(&resolved_ty, &list_ty) {
                        checker.report_type_error(&e, span);
                    }

                    for pattern in elements {
                        unify_pattern_with_scrutinee(checker, pattern, &elem_ty, span);
                    }
                }
                Type::Error => {}
                _ => {
                    checker.diagnostics.errors.push(TypeCheckError {
                        message: format!(
                            "list pattern cannot match type `{}`",
                            resolved_ty.display(checker.context.interner)
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2001,
                    });
                }
            }
        }

        MatchPattern::Range { start, end, .. } => {
            // Range patterns match numeric types
            if let Some(start_id) = start {
                let start_ty = infer_expr(checker, *start_id);
                if let Err(e) = checker.inference.ctx.unify(&start_ty, &resolved_ty) {
                    checker.report_type_error(&e, span);
                }
            }
            if let Some(end_id) = end {
                let end_ty = infer_expr(checker, *end_id);
                if let Err(e) = checker.inference.ctx.unify(&end_ty, &resolved_ty) {
                    checker.report_type_error(&e, span);
                }
            }
        }

        MatchPattern::Or(patterns) => {
            for p in patterns {
                unify_pattern_with_scrutinee(checker, p, scrutinee_ty, span);
            }
        }

        MatchPattern::At { pattern, .. } => {
            unify_pattern_with_scrutinee(checker, pattern, scrutinee_ty, span);
        }
    }
}

/// Collect names bound by a match pattern.
///
/// This is used for free variable analysis in closure capture detection.
pub fn collect_match_pattern_names(pattern: &MatchPattern) -> HashSet<Name> {
    let mut names = HashSet::new();
    collect_match_pattern_names_inner(pattern, &mut names);
    names
}

fn collect_match_pattern_names_inner(pattern: &MatchPattern, names: &mut HashSet<Name>) {
    match pattern {
        // Patterns that introduce no bindings
        MatchPattern::Wildcard | MatchPattern::Literal(_) | MatchPattern::Range { .. } => {}

        MatchPattern::Binding(name) => {
            names.insert(*name);
        }

        MatchPattern::Variant { inner, .. } => {
            if let Some(inner_pattern) = inner {
                collect_match_pattern_names_inner(inner_pattern, names);
            }
        }

        MatchPattern::Struct { fields } => {
            for (field_name, opt_pattern) in fields {
                match opt_pattern {
                    Some(nested) => {
                        collect_match_pattern_names_inner(nested, names);
                    }
                    None => {
                        // Shorthand: { x } binds x
                        names.insert(*field_name);
                    }
                }
            }
        }

        MatchPattern::Tuple(patterns) => {
            for p in patterns {
                collect_match_pattern_names_inner(p, names);
            }
        }

        MatchPattern::List { elements, rest } => {
            for p in elements {
                collect_match_pattern_names_inner(p, names);
            }
            if let Some(rest_name) = rest {
                names.insert(*rest_name);
            }
        }

        MatchPattern::Or(patterns) => {
            // All branches should bind the same names, so just take first
            if let Some(first) = patterns.first() {
                collect_match_pattern_names_inner(first, names);
            }
        }

        MatchPattern::At { name, pattern } => {
            names.insert(*name);
            collect_match_pattern_names_inner(pattern, names);
        }
    }
}
