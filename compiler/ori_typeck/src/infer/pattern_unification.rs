//! Pattern unification with scrutinee types.

use super::infer_expr;
use super::pattern_types::{get_struct_field_types, get_variant_field_types};
use crate::checker::TypeChecker;
use crate::registry::TypeKind;
use ori_ir::{MatchPattern, Span};
use ori_types::Type;

/// Unify a pattern's structure with the scrutinee type.
pub fn unify_pattern_with_scrutinee(
    checker: &mut TypeChecker<'_>,
    pattern: &MatchPattern,
    scrutinee_ty: &Type,
    span: Span,
) {
    let resolved_ty = checker.inference.ctx.resolve(scrutinee_ty);

    match pattern {
        MatchPattern::Wildcard | MatchPattern::Binding(_) => {}

        MatchPattern::Literal(expr_id) => {
            let literal_ty = infer_expr(checker, *expr_id);
            if let Err(e) = checker.inference.ctx.unify(&literal_ty, &resolved_ty) {
                checker.report_type_error(&e, span);
            }
        }

        MatchPattern::Variant { name, inner } => {
            let variant_str = checker.context.interner.lookup(*name);

            let valid_variant = match variant_str {
                "Some" | "None" => matches!(resolved_ty, Type::Option(_) | Type::Var(_)),
                "Ok" | "Err" => matches!(resolved_ty, Type::Result { .. } | Type::Var(_)),
                _ => {
                    if let Type::Named(type_name) = &resolved_ty {
                        if let Some(entry) = checker.registries.types.get_by_name(*type_name) {
                            if let TypeKind::Enum { variants } = &entry.kind {
                                variants.iter().any(|v| v.name == *name)
                            } else {
                                false
                            }
                        } else {
                            true
                        }
                    } else {
                        matches!(resolved_ty, Type::Var(_))
                    }
                }
            };

            if !valid_variant && !matches!(resolved_ty, Type::Error) {
                checker.push_error(
                    format!(
                        "pattern `{}` is not a valid variant for type `{}`",
                        variant_str,
                        resolved_ty.display(checker.context.interner)
                    ),
                    span,
                    ori_diagnostic::ErrorCode::E2001,
                );
            }

            // Unify inner patterns with variant field types
            let field_types = get_variant_field_types(checker, &resolved_ty, *name);
            let inner_ids = checker.context.arena.get_match_pattern_list(*inner);
            for (pat_id, field_ty) in inner_ids.iter().zip(field_types.iter()) {
                let inner_pattern = checker.context.arena.get_match_pattern(*pat_id);
                unify_pattern_with_scrutinee(checker, inner_pattern, field_ty, span);
            }
            // Extra patterns get fresh type variables
            for pat_id in inner_ids.iter().skip(field_types.len()) {
                let fresh = checker.inference.ctx.fresh_var();
                let inner_pattern = checker.context.arena.get_match_pattern(*pat_id);
                unify_pattern_with_scrutinee(checker, inner_pattern, &fresh, span);
            }
        }

        MatchPattern::Struct { fields } => {
            let field_types = get_struct_field_types(checker, &resolved_ty);

            for (field_name, opt_pattern_id) in fields {
                let field_ty = field_types.get(field_name).cloned();

                if field_ty.is_none() && !matches!(resolved_ty, Type::Var(_) | Type::Error) {
                    let field_str = checker.context.interner.lookup(*field_name);
                    checker.push_error(
                        format!(
                            "type `{}` has no field `{}`",
                            resolved_ty.display(checker.context.interner),
                            field_str
                        ),
                        span,
                        ori_diagnostic::ErrorCode::E2001,
                    );
                }

                if let (Some(nested_id), Some(ty)) = (opt_pattern_id, field_ty) {
                    let nested = checker.context.arena.get_match_pattern(*nested_id);
                    unify_pattern_with_scrutinee(checker, nested, &ty, span);
                }
            }
        }

        MatchPattern::Tuple(patterns) => {
            let pattern_ids = checker.context.arena.get_match_pattern_list(*patterns);
            match &resolved_ty {
                Type::Tuple(elems) => {
                    if pattern_ids.len() != elems.len() {
                        checker.push_error(
                            format!(
                                "tuple pattern has {} elements but scrutinee has {}",
                                pattern_ids.len(),
                                elems.len()
                            ),
                            span,
                            ori_diagnostic::ErrorCode::E2001,
                        );
                    }

                    for (pat_id, ty) in pattern_ids.iter().zip(elems.iter()) {
                        let pattern = checker.context.arena.get_match_pattern(*pat_id);
                        unify_pattern_with_scrutinee(checker, pattern, ty, span);
                    }
                }
                Type::Var(_) => {
                    let elem_types: Vec<Type> = pattern_ids
                        .iter()
                        .map(|_| checker.inference.ctx.fresh_var())
                        .collect();
                    let tuple_ty = Type::Tuple(elem_types.clone());
                    if let Err(e) = checker.inference.ctx.unify(&resolved_ty, &tuple_ty) {
                        checker.report_type_error(&e, span);
                    }

                    for (pat_id, ty) in pattern_ids.iter().zip(elem_types.iter()) {
                        let pattern = checker.context.arena.get_match_pattern(*pat_id);
                        unify_pattern_with_scrutinee(checker, pattern, ty, span);
                    }
                }
                Type::Error => {}
                _ => {
                    checker.push_error(
                        format!(
                            "tuple pattern cannot match type `{}`",
                            resolved_ty.display(checker.context.interner)
                        ),
                        span,
                        ori_diagnostic::ErrorCode::E2001,
                    );
                }
            }
        }

        MatchPattern::List { elements, rest: _ } => {
            let element_ids = checker.context.arena.get_match_pattern_list(*elements);
            match &resolved_ty {
                Type::List(elem) => {
                    for pat_id in element_ids {
                        let pattern = checker.context.arena.get_match_pattern(*pat_id);
                        unify_pattern_with_scrutinee(checker, pattern, elem, span);
                    }
                }
                Type::Var(_) => {
                    let elem_ty = checker.inference.ctx.fresh_var();
                    let list_ty = Type::List(Box::new(elem_ty.clone()));
                    if let Err(e) = checker.inference.ctx.unify(&resolved_ty, &list_ty) {
                        checker.report_type_error(&e, span);
                    }

                    for pat_id in element_ids {
                        let pattern = checker.context.arena.get_match_pattern(*pat_id);
                        unify_pattern_with_scrutinee(checker, pattern, &elem_ty, span);
                    }
                }
                Type::Error => {}
                _ => {
                    checker.push_error(
                        format!(
                            "list pattern cannot match type `{}`",
                            resolved_ty.display(checker.context.interner)
                        ),
                        span,
                        ori_diagnostic::ErrorCode::E2001,
                    );
                }
            }
        }

        MatchPattern::Range { start, end, .. } => {
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
            for pat_id in checker.context.arena.get_match_pattern_list(*patterns) {
                let p = checker.context.arena.get_match_pattern(*pat_id);
                unify_pattern_with_scrutinee(checker, p, scrutinee_ty, span);
            }
        }

        MatchPattern::At { pattern, .. } => {
            let inner = checker.context.arena.get_match_pattern(*pattern);
            unify_pattern_with_scrutinee(checker, inner, scrutinee_ty, span);
        }
    }
}
