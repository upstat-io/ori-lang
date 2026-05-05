//! `BindingPattern` irrefutability check enforcing Spec Clauses 15.4 and 16.
//!
//! `let <pat> = expr` SHALL require an irrefutable pattern per Spec Clause 15.4.
//! Refutable patterns are rejected with error code E2001 listing the uncovered shape.
//!
//! See `compiler_repo/docs/ori_lang/v2026/spec/15-patterns.md:332-340` for the
//! canonical List Pattern Exhaustiveness table (`[]`, `[$x]`, `[$x, $y]`,
//! `[$x, ..rest]`, `[..rest]`). The irrefutability context requirement
//! (`let` / `for` MUST be irrefutable) is at `15-patterns.md:276-281`.

use ori_ir::{BindingPattern, FieldBinding, Name};

use crate::infer::expr::structs::lookup_struct_field_types;
use crate::infer::InferEngine;
use crate::{Idx, Tag};

/// Why a `BindingPattern` is refutable at a let-class binding site.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum RefutableReason {
    /// List pattern requires a specific length on a dynamic-length `[T]`.
    ListLength {
        /// Minimum elements the pattern demands.
        required: usize,
        /// Whether a rest pattern `..rest` is present (covers the tail).
        has_rest: bool,
    },
    /// A sub-pattern inside a Tuple or Struct is refutable.
    NestedRefutable {
        /// Path from the top-level pattern to the refutable sub-pattern.
        path: Vec<NestedPathStep>,
        /// The refutable reason at the leaf position.
        inner: Box<RefutableReason>,
    },
}

/// One step in the path from the top-level pattern to a refutable sub-pattern.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum NestedPathStep {
    /// Index into a tuple pattern element.
    TupleIndex(usize),
    /// Name of a struct field.
    StructField(Name),
}

/// Check whether a `BindingPattern` is irrefutable at a let-class binding site.
///
/// Returns `Ok(())` if irrefutable, or `Err(RefutableReason)` enumerating why not.
/// Type-shape mismatches (e.g., `let (a, b) = 1`) are NOT diagnosed here;
/// those are `bind_pattern`'s responsibility. This predicate only catches refutable
/// pattern shapes on types that structurally match the pattern's arity.
pub(crate) fn pattern_is_irrefutable(
    engine: &mut InferEngine<'_>,
    pattern: &BindingPattern,
    ty: Idx,
) -> Result<(), RefutableReason> {
    match pattern {
        // Wildcard and simple Name bindings always match — irrefutable.
        BindingPattern::Wildcard | BindingPattern::Name { .. } => Ok(()),

        BindingPattern::List { elements, rest } => {
            if elements.is_empty() && rest.is_some() {
                Ok(())
            } else {
                Err(RefutableReason::ListLength {
                    required: elements.len(),
                    has_rest: rest.is_some(),
                })
            }
        }

        BindingPattern::Tuple(pats) => {
            let resolved = engine.resolve(ty);
            let elem_tys: Vec<Idx> = match engine.pool().tag(resolved) {
                Tag::Tuple => engine.pool().tuple_elems(resolved),
                _ => vec![Idx::ERROR; pats.len()],
            };
            let walk_n = elem_tys.len().min(pats.len());
            for (i, sub_pat) in pats.iter().take(walk_n).enumerate() {
                if let Err(inner) = pattern_is_irrefutable(engine, sub_pat, elem_tys[i]) {
                    return Err(wrap_path(NestedPathStep::TupleIndex(i), inner));
                }
            }
            Ok(())
        }

        BindingPattern::Struct { fields } => {
            let resolved = engine.resolve(ty);
            let field_types = match engine.pool().tag(resolved) {
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

            for FieldBinding {
                name,
                pattern: sub_opt,
                mutable,
                ..
            } in fields
            {
                let field_ty = match &field_types {
                    Some(map) => {
                        let m: &rustc_hash::FxHashMap<Name, Idx> = map;
                        m.get(name).copied().unwrap_or(Idx::ERROR)
                    }
                    None => Idx::ERROR,
                };
                let sub_pat: BindingPattern = match sub_opt {
                    Some(p) => p.clone(),
                    None => BindingPattern::Name {
                        name: *name,
                        mutable: *mutable,
                    },
                };
                if let Err(inner) = pattern_is_irrefutable(engine, &sub_pat, field_ty) {
                    return Err(wrap_path(NestedPathStep::StructField(*name), inner));
                }
            }
            Ok(())
        }
    }
}

fn wrap_path(step: NestedPathStep, inner: RefutableReason) -> RefutableReason {
    match inner {
        RefutableReason::NestedRefutable {
            mut path,
            inner: deeper,
        } => {
            path.insert(0, step);
            RefutableReason::NestedRefutable {
                path,
                inner: deeper,
            }
        }
        leaf @ RefutableReason::ListLength { .. } => RefutableReason::NestedRefutable {
            path: vec![step],
            inner: Box::new(leaf),
        },
    }
}

#[cfg(test)]
mod tests;
