//! Type instantiation and substitution.
//!
//! Handles instantiation of type schemes (replacing generalized variables
//! with fresh unbound variables) and general type variable substitution.

use rustc_hash::FxHashMap;

use crate::{Idx, Tag};

use super::UnifyEngine;

impl UnifyEngine<'_> {
    /// Instantiate a type scheme with fresh variables.
    ///
    /// For each quantified variable in the scheme, creates a fresh unbound
    /// variable at the current rank, then substitutes throughout the body.
    ///
    /// Returns the type unchanged if it's not a scheme.
    ///
    /// # Example
    ///
    /// ```text
    /// // Given scheme: ∀a. a -> a
    /// let concrete = engine.instantiate(scheme);  // $1 -> $1 (fresh var)
    /// engine.unify(concrete_param, Idx::INT);     // $1 unified with int
    /// // `concrete` resolves to `int -> int`.
    /// ```
    pub fn instantiate(&mut self, scheme_idx: Idx) -> Idx {
        self.instantiate_with_subst(scheme_idx).0
    }

    /// Instantiate a scheme and return both the substituted body AND the
    /// `scheme_var_id → fresh_var_idx` substitution map.
    ///
    /// Callers needing the map (e.g. `check_method_inline_bounds` to find the
    /// post-instantiation Var Idx of each method-level binder by its
    /// declared `var_id`) use this instead of `instantiate`. Non-scheme
    /// inputs return `(scheme_idx, empty_map)`; monomorphic schemes return
    /// `(body, empty_map)`.
    pub fn instantiate_with_subst(&mut self, scheme_idx: Idx) -> (Idx, FxHashMap<u32, Idx>) {
        if self.pool.tag(scheme_idx) != Tag::Scheme {
            return (scheme_idx, FxHashMap::default());
        }

        let vars = self.pool.scheme_vars(scheme_idx).to_vec();
        let body = self.pool.scheme_body(scheme_idx);

        if vars.is_empty() {
            return (body, FxHashMap::default());
        }

        // Create fresh variables for each quantified variable
        let mut subst: FxHashMap<u32, Idx> = FxHashMap::default();
        for var_id in vars {
            let fresh = self.fresh_var();
            subst.insert(var_id, fresh);
        }

        // Substitute in the body
        let new_body = self.substitute(body, &subst);
        (new_body, subst)
    }

    /// Substitute variables according to the given mapping.
    ///
    /// Returns the original type if no substitutions apply.
    fn substitute(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        crate::pool::substitute_in_pool(self.pool, ty, subst)
    }
}
