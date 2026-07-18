//! Rank-based type generalization.
//!
//! Implements let-polymorphism by generalizing unbound type variables
//! at or above the current rank into type schemes (forall quantifiers).

use rustc_hash::FxHashMap;

use crate::pool::substitute::substitute_in_pool;
use crate::{GeneralizedVarState, Idx, Pool, Rank, Tag, TypeFlags, UnboundVarState, VarState};

use super::UnifyEngine;

impl UnifyEngine<'_> {
    /// Generalize a type at the current rank.
    ///
    /// Finds all unbound type variables at or above the current rank
    /// and creates a type scheme quantifying over them.
    ///
    /// Returns the original type if no variables need generalization (monomorphic).
    /// Returns a type scheme `∀vars. body` if variables were generalized.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ori_types::{Pool, Tag, UnifyEngine};
    ///
    /// let mut pool = Pool::new();
    /// let mut engine = UnifyEngine::new(&mut pool);
    /// engine.enter_scope();
    /// let var = engine.fresh_var();
    /// let fn_ty = engine.pool_mut().function(&[var], var); // a -> a
    /// let scheme = engine.generalize(fn_ty);   // ∀a. a -> a
    /// engine.exit_scope();
    /// assert_eq!(engine.pool().tag(scheme), Tag::Scheme);
    /// ```
    pub fn generalize(&mut self, ty: Idx) -> Idx {
        let ty = self.resolve(ty);

        // Fast path: no variables
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return ty;
        }

        let vars = self.collect_free_vars_at_rank(ty, self.current_rank);

        if vars.is_empty() {
            return ty;
        }

        // Why: end the immutable state borrow before mutating the same variable.
        for &var_id in &vars {
            let gen = match self.pool.var_state(var_id) {
                VarState::Unbound(UnboundVarState { id, name, .. }) => Some((*id, *name)),
                _ => None,
            };
            if let Some((id, name)) = gen {
                *self.pool.var_state_mut(var_id) =
                    VarState::Generalized(GeneralizedVarState { id, name });
            }
        }

        // INVARIANT: schemes expose generalized variables only as `Tag::BoundVar` leaves.
        let body = rewrite_generalized_to_bound_var(self.pool, ty, &vars);

        self.pool.scheme(&vars, body)
    }

    /// Collect unbound type variables at or above the given rank.
    fn collect_free_vars_at_rank(&self, ty: Idx, min_rank: Rank) -> Vec<u32> {
        let mut vars = Vec::new();
        self.collect_free_vars_inner(ty, min_rank, &mut vars);
        vars.sort_unstable();
        vars.dedup();
        vars
    }

    /// Inner traversal for collecting free variables.
    ///
    /// Delegates compound-tag child traversal to the canonical
    /// `Pool::visit_children` walker (TF-3) rather than open-coding a parallel
    /// tag-dispatch ladder. Mirrors the shape of
    /// `check::validators::collect_first_unbound_var` — the in-repo reference
    /// for this delegation pattern.
    fn collect_free_vars_inner(&self, ty: Idx, min_rank: Rank, vars: &mut Vec<u32>) {
        // Fast path: no variables. `Tag::BoundVar` sets `HAS_BOUND_VAR`, not
        // `HAS_VAR` (TF-1), so scheme bodies whose only inner
        // variables are bound, short-circuit.
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return;
        }

        match self.pool.tag(ty) {
            Tag::Var => {
                let var_id = self.pool.data(ty);
                match self.pool.var_state(var_id) {
                    VarState::Unbound(UnboundVarState { rank, .. })
                        if rank.can_generalize_at(min_rank) =>
                    {
                        vars.push(var_id);
                    }
                    VarState::Link { target } => {
                        self.collect_free_vars_inner(*target, min_rank, vars);
                    }
                    _ => {}
                }
            }

            // Defensive against a stale-flag edge case: `BoundVar` sets
            // `HAS_BOUND_VAR`, not `HAS_VAR`, so this arm should be
            // unreachable under the top-level gate. Skip silently if reached.
            Tag::BoundVar => {}

            // INVARIANT: the canonical walker covers every compound tag.
            _ => {
                self.pool.visit_children(ty, |child| {
                    self.collect_free_vars_inner(child, min_rank, vars);
                });
            }
        }
    }
}

/// Rewrite `Tag::Var(Generalized)` leaves in a scheme body to `Tag::BoundVar`
/// leaves bearing the same `var_id`.
///
/// Builds a substitution map `{var_id → BoundVar(var_id)}` for each
/// scheme-declared `var_id`, then delegates to the canonical
/// [`substitute_in_pool`] machinery to walk the body and rewrite matching
/// `Tag::Var` leaves. The new `Tag::BoundVar` leaf carries `data == var_id`,
/// satisfying's "data = `var_id` matching one of the scheme's
/// declared var ids" rule.
///
/// Reusing `substitute_in_pool` keeps the structural recursion in one place
///  — no parallel walker needed.
///
/// Called once per `generalize()` invocation, between `var_states` mutation
/// and scheme construction. The substitution applies because
/// `substitute_var` matches by `var_id` directly and has no generalized-state
/// fallback (orphan
/// `Tag::Var(Generalized)` entries simply fall through to return `ty`
/// unchanged).
fn rewrite_generalized_to_bound_var(pool: &mut Pool, body: Idx, scheme_var_ids: &[u32]) -> Idx {
    let subst: FxHashMap<u32, Idx> = scheme_var_ids
        .iter()
        .map(|&id| (id, pool.bound_var(id)))
        .collect();
    substitute_in_pool(pool, body, &subst)
}
