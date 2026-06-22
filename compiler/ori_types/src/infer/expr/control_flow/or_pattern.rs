//! Or-pattern binding reconciliation for match patterns.

use ori_ir::{ExprArena, MatchPatternRange, Name, Span};
use rustc_hash::FxHashMap;

use super::super::super::InferEngine;
use super::matches::check_match_pattern;
use crate::type_error::OrBindingMismatchReason;
use crate::{Idx, PatternKey, TypeCheckError};

/// Check an or-pattern: all alternatives match the same type AND bind identical
/// name-sets + types (Spec: Clause 15 patterns). Each alternative's bindings are
/// staged in an isolated child scope, reconciled against the first (canonical)
/// alternative, and only names bound — at a compatible type — by EVERY
/// alternative are committed to the arm scope. Divergent names emit an error
/// (one per divergence, never bail-first) and are NOT bound, so the arm body
/// never sees a name that is unbound on some matched alternative.
pub(super) fn check_or_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    alternatives: MatchPatternRange,
    expected_ty: Idx,
    span: Span,
) {
    let alt_ids = arena.get_match_pattern_list(alternatives);
    let mut staged_maps: Vec<FxHashMap<Name, Idx>> = Vec::with_capacity(alt_ids.len());
    for alt_id in alt_ids {
        let alt_pattern = arena.get_match_pattern(*alt_id);
        let nested_key = PatternKey::Nested(alt_id.raw());
        engine.enter_scope();
        check_match_pattern(engine, arena, alt_pattern, expected_ty, nested_key, span);
        let staged: FxHashMap<Name, Idx> = engine.env().local_bindings().collect();
        engine.exit_scope();
        staged_maps.push(staged);
    }

    let Some((canon, rest)) = staged_maps.split_first() else {
        return;
    };

    // Diff every subsequent alternative against the canonical first one,
    // emitting a name- or type-divergence error per divergent binding.
    for alt in rest {
        reconcile_or_alternative(engine, canon, alt, span);
    }

    // Commit only names bound by EVERY alternative at a compatible type — the
    // validated common set. Divergent names already emitted an error and stay
    // unbound (the error blocks compilation; the arm scope stays consistent).
    for (name, canon_ty) in canon {
        let mut common_and_compatible = true;
        for alt in rest {
            match alt.get(name) {
                Some(&alt_ty) if types_match(engine, *canon_ty, alt_ty) => {}
                _ => {
                    common_and_compatible = false;
                    break;
                }
            }
        }
        if common_and_compatible {
            engine.env_mut().bind(*name, *canon_ty);
        }
    }
}

/// Reconcile one or-pattern alternative's staged bindings against the canonical
/// (first) alternative, accumulating every name- and type-divergence error.
fn reconcile_or_alternative(
    engine: &mut InferEngine<'_>,
    canon: &FxHashMap<Name, Idx>,
    staged: &FxHashMap<Name, Idx>,
    span: Span,
) {
    // Names bound on this alternative but absent from canonical.
    for name in staged.keys() {
        if !canon.contains_key(name) {
            engine.push_error(TypeCheckError::or_pattern_binding_mismatch(
                span,
                *name,
                OrBindingMismatchReason::NameDivergence,
            ));
        }
    }
    // Names in canonical: absent here (name divergence) or present at a
    // divergent type (type divergence).
    for (name, canon_ty) in canon {
        match staged.get(name) {
            None => engine.push_error(TypeCheckError::or_pattern_binding_mismatch(
                span,
                *name,
                OrBindingMismatchReason::NameDivergence,
            )),
            Some(&alt_ty) if !types_match(engine, *canon_ty, alt_ty) => {
                engine.push_error(TypeCheckError::or_pattern_binding_mismatch(
                    span,
                    *name,
                    OrBindingMismatchReason::TypeDivergence {
                        found: *canon_ty,
                        other: alt_ty,
                    },
                ));
            }
            Some(_) => {}
        }
    }
}

/// Non-mutating type-equality probe via the pool's canonical `structural_eq`
/// (read-only) — compares type STRUCTURE, so structurally-equal types that were
/// separately interned (e.g. re-interned during cross-module merge) still match,
/// and the union-find is never bound as a side effect of the comparison.
fn types_match(engine: &InferEngine<'_>, a: Idx, b: Idx) -> bool {
    engine.pool().structural_eq(a, b)
}
