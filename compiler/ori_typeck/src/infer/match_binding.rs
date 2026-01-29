//! Match pattern binding extraction.
//!
//! Extracts variable bindings from match patterns and unifies pattern structure
//! with scrutinee types.

use super::pattern_types::{get_struct_field_types, get_variant_field_types};
use crate::checker::TypeChecker;
use ori_ir::{ExprArena, MatchPattern, Name};
use ori_types::Type;
use std::collections::HashSet;

// Re-export unification function
pub use super::pattern_unification::unify_pattern_with_scrutinee;

/// Extract variable bindings from a match pattern given the scrutinee type.
pub fn extract_match_pattern_bindings(
    checker: &mut TypeChecker<'_>,
    pattern: &MatchPattern,
    scrutinee_ty: &Type,
) -> Vec<(Name, Type)> {
    let resolved_ty = checker.inference.ctx.resolve(scrutinee_ty);

    match pattern {
        MatchPattern::Wildcard | MatchPattern::Literal(_) | MatchPattern::Range { .. } => vec![],

        MatchPattern::Binding(name) => {
            vec![(*name, resolved_ty.clone())]
        }

        MatchPattern::Variant { name, inner } => {
            let field_types = get_variant_field_types(checker, &resolved_ty, *name);
            let inner_ids = checker.context.arena.get_match_pattern_list(*inner);

            // Handle multiple inner patterns for multi-field variants
            let mut bindings = Vec::new();
            for (pat_id, field_ty) in inner_ids.iter().zip(field_types.iter()) {
                let pattern = checker.context.arena.get_match_pattern(*pat_id);
                bindings.extend(extract_match_pattern_bindings(checker, pattern, field_ty));
            }
            // If fewer field types than patterns, use fresh vars
            for pat_id in inner_ids.iter().skip(field_types.len()) {
                let fresh = checker.inference.ctx.fresh_var();
                let pattern = checker.context.arena.get_match_pattern(*pat_id);
                bindings.extend(extract_match_pattern_bindings(checker, pattern, &fresh));
            }
            bindings
        }

        MatchPattern::Struct { fields } => {
            let field_types = get_struct_field_types(checker, &resolved_ty);

            let mut bindings = Vec::new();
            for (field_name, opt_pattern_id) in fields {
                let field_ty = field_types
                    .iter()
                    .find(|(n, _)| n == field_name)
                    .map_or_else(|| checker.inference.ctx.fresh_var(), |(_, ty)| ty.clone());

                match opt_pattern_id {
                    Some(nested_id) => {
                        let nested = checker.context.arena.get_match_pattern(*nested_id);
                        bindings.extend(extract_match_pattern_bindings(checker, nested, &field_ty));
                    }
                    None => {
                        bindings.push((*field_name, field_ty));
                    }
                }
            }
            bindings
        }

        MatchPattern::Tuple(patterns) => {
            let pattern_ids = checker.context.arena.get_match_pattern_list(*patterns);
            let elem_types = match &resolved_ty {
                Type::Tuple(elems) => elems.clone(),
                _ => vec![checker.inference.ctx.fresh_var(); pattern_ids.len()],
            };

            let mut bindings = Vec::new();
            for (pat_id, ty) in pattern_ids.iter().zip(elem_types.iter()) {
                let pattern = checker.context.arena.get_match_pattern(*pat_id);
                bindings.extend(extract_match_pattern_bindings(checker, pattern, ty));
            }
            bindings
        }

        MatchPattern::List { elements, rest } => {
            let elem_ty = match &resolved_ty {
                Type::List(elem) => (**elem).clone(),
                _ => checker.inference.ctx.fresh_var(),
            };

            let element_ids = checker.context.arena.get_match_pattern_list(*elements);
            let mut bindings = Vec::new();
            for pat_id in element_ids {
                let pattern = checker.context.arena.get_match_pattern(*pat_id);
                bindings.extend(extract_match_pattern_bindings(checker, pattern, &elem_ty));
            }

            if let Some(rest_name) = rest {
                bindings.push((*rest_name, Type::List(Box::new(elem_ty))));
            }

            bindings
        }

        MatchPattern::Or(patterns) => {
            let pattern_ids = checker.context.arena.get_match_pattern_list(*patterns);
            if let Some(first_id) = pattern_ids.first() {
                let first = checker.context.arena.get_match_pattern(*first_id);
                extract_match_pattern_bindings(checker, first, &resolved_ty)
            } else {
                vec![]
            }
        }

        MatchPattern::At { name, pattern } => {
            let mut bindings = vec![(*name, resolved_ty.clone())];
            let inner = checker.context.arena.get_match_pattern(*pattern);
            bindings.extend(extract_match_pattern_bindings(
                checker,
                inner,
                &resolved_ty,
            ));
            bindings
        }
    }
}

/// Collect names bound by a match pattern.
pub fn collect_match_pattern_names(pattern: &MatchPattern, arena: &ExprArena) -> HashSet<Name> {
    let mut names = HashSet::new();
    collect_match_pattern_names_inner(pattern, arena, &mut names);
    names
}

fn collect_match_pattern_names_inner(
    pattern: &MatchPattern,
    arena: &ExprArena,
    names: &mut HashSet<Name>,
) {
    match pattern {
        MatchPattern::Wildcard | MatchPattern::Literal(_) | MatchPattern::Range { .. } => {}

        MatchPattern::Binding(name) => {
            names.insert(*name);
        }

        MatchPattern::Variant { inner, .. } => {
            for pat_id in arena.get_match_pattern_list(*inner) {
                collect_match_pattern_names_inner(arena.get_match_pattern(*pat_id), arena, names);
            }
        }

        MatchPattern::Struct { fields } => {
            for (field_name, opt_pattern_id) in fields {
                match opt_pattern_id {
                    Some(nested_id) => {
                        collect_match_pattern_names_inner(
                            arena.get_match_pattern(*nested_id),
                            arena,
                            names,
                        );
                    }
                    None => {
                        names.insert(*field_name);
                    }
                }
            }
        }

        MatchPattern::Tuple(patterns) => {
            for pat_id in arena.get_match_pattern_list(*patterns) {
                collect_match_pattern_names_inner(arena.get_match_pattern(*pat_id), arena, names);
            }
        }

        MatchPattern::List { elements, rest } => {
            for pat_id in arena.get_match_pattern_list(*elements) {
                collect_match_pattern_names_inner(arena.get_match_pattern(*pat_id), arena, names);
            }
            if let Some(rest_name) = rest {
                names.insert(*rest_name);
            }
        }

        MatchPattern::Or(patterns) => {
            let pattern_ids = arena.get_match_pattern_list(*patterns);
            if let Some(first_id) = pattern_ids.first() {
                collect_match_pattern_names_inner(arena.get_match_pattern(*first_id), arena, names);
            }
        }

        MatchPattern::At { name, pattern } => {
            names.insert(*name);
            collect_match_pattern_names_inner(arena.get_match_pattern(*pattern), arena, names);
        }
    }
}
