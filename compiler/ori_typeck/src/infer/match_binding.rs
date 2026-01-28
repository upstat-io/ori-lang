//! Match pattern binding extraction.
//!
//! Extracts variable bindings from match patterns and unifies pattern structure
//! with scrutinee types.

use super::pattern_types::{get_struct_field_types, get_variant_inner_type};
use crate::checker::TypeChecker;
use ori_ir::{MatchPattern, Name};
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
            let inner_ty = get_variant_inner_type(checker, &resolved_ty, *name);

            if let Some(inner_pattern) = inner {
                extract_match_pattern_bindings(checker, inner_pattern, &inner_ty)
            } else {
                vec![]
            }
        }

        MatchPattern::Struct { fields } => {
            let field_types = get_struct_field_types(checker, &resolved_ty);

            let mut bindings = Vec::new();
            for (field_name, opt_pattern) in fields {
                let field_ty = field_types
                    .iter()
                    .find(|(n, _)| n == field_name)
                    .map_or_else(|| checker.inference.ctx.fresh_var(), |(_, ty)| ty.clone());

                match opt_pattern {
                    Some(nested) => {
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
            let elem_ty = match &resolved_ty {
                Type::List(elem) => (**elem).clone(),
                _ => checker.inference.ctx.fresh_var(),
            };

            let mut bindings = Vec::new();
            for pattern in elements {
                bindings.extend(extract_match_pattern_bindings(checker, pattern, &elem_ty));
            }

            if let Some(rest_name) = rest {
                bindings.push((*rest_name, Type::List(Box::new(elem_ty))));
            }

            bindings
        }

        MatchPattern::Or(patterns) => {
            if let Some(first) = patterns.first() {
                extract_match_pattern_bindings(checker, first, &resolved_ty)
            } else {
                vec![]
            }
        }

        MatchPattern::At { name, pattern } => {
            let mut bindings = vec![(*name, resolved_ty.clone())];
            bindings.extend(extract_match_pattern_bindings(
                checker,
                pattern,
                &resolved_ty,
            ));
            bindings
        }
    }
}

/// Collect names bound by a match pattern.
pub fn collect_match_pattern_names(pattern: &MatchPattern) -> HashSet<Name> {
    let mut names = HashSet::new();
    collect_match_pattern_names_inner(pattern, &mut names);
    names
}

fn collect_match_pattern_names_inner(pattern: &MatchPattern, names: &mut HashSet<Name>) {
    match pattern {
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
