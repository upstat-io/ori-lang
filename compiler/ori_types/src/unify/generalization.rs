//! Rank-based type generalization.
//!
//! Implements let-polymorphism by generalizing unbound type variables
//! at or above the current rank into type schemes (forall quantifiers).

use crate::{Idx, Rank, Tag, TypeFlags, VarState};

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
    /// ```ignore
    /// let mut engine = UnifyEngine::new(&mut pool);
    /// engine.enter_scope();
    /// let var = engine.fresh_var();
    /// let fn_ty = pool.function(&[var], var);  // a -> a
    /// let scheme = engine.generalize(fn_ty);   // ∀a. a -> a
    /// engine.exit_scope();
    /// ```
    pub fn generalize(&mut self, ty: Idx) -> Idx {
        // Resolve to get the current structure
        let ty = self.resolve(ty);

        // Fast path: no variables
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return ty;
        }

        // Collect free variables at current rank or higher
        let vars = self.collect_free_vars_at_rank(ty, self.current_rank);

        if vars.is_empty() {
            return ty; // Monomorphic
        }

        // Mark collected variables as generalized.
        // Extract id/name from the immutable borrow, then write the new state.
        for &var_id in &vars {
            let gen = match self.pool.var_state(var_id) {
                VarState::Unbound { id, name, .. } => Some((*id, *name)),
                _ => None,
            };
            if let Some((id, name)) = gen {
                *self.pool.var_state_mut(var_id) = VarState::Generalized { id, name };
            }
        }

        // Create type scheme
        self.pool.scheme(&vars, ty)
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
    fn collect_free_vars_inner(&self, ty: Idx, min_rank: Rank, vars: &mut Vec<u32>) {
        // Fast path: no variables
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return;
        }

        match self.pool.tag(ty) {
            Tag::Var => {
                let var_id = self.pool.data(ty);
                match self.pool.var_state(var_id) {
                    VarState::Unbound { rank, .. } if rank.can_generalize_at(min_rank) => {
                        vars.push(var_id);
                    }
                    VarState::Link { target } => {
                        self.collect_free_vars_inner(*target, min_rank, vars);
                    }
                    _ => {}
                }
            }

            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => {
                let child = Idx::from_raw(self.pool.data(ty));
                self.collect_free_vars_inner(child, min_rank, vars);
            }

            Tag::Map => {
                let key = self.pool.map_key(ty);
                let value = self.pool.map_value(ty);
                self.collect_free_vars_inner(key, min_rank, vars);
                self.collect_free_vars_inner(value, min_rank, vars);
            }

            Tag::Result => {
                let ok = self.pool.result_ok(ty);
                let err = self.pool.result_err(ty);
                self.collect_free_vars_inner(ok, min_rank, vars);
                self.collect_free_vars_inner(err, min_rank, vars);
            }

            Tag::Borrowed => {
                let inner = self.pool.borrowed_inner(ty);
                self.collect_free_vars_inner(inner, min_rank, vars);
            }

            Tag::Function => {
                let params = self.pool.function_params(ty);
                let ret = self.pool.function_return(ty);
                for p in params {
                    self.collect_free_vars_inner(p, min_rank, vars);
                }
                self.collect_free_vars_inner(ret, min_rank, vars);
            }

            Tag::Tuple => {
                let elems = self.pool.tuple_elems(ty);
                for e in elems {
                    self.collect_free_vars_inner(e, min_rank, vars);
                }
            }

            Tag::Applied => {
                let args = self.pool.applied_args(ty);
                for a in args {
                    self.collect_free_vars_inner(a, min_rank, vars);
                }
            }

            // Schemes have their own quantification, other types don't contain variables
            _ => {}
        }
    }
}
