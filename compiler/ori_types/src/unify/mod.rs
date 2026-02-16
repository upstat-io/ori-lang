//! Type unification engine.
//!
//! This module provides link-based unification with path compression,
//! achieving O(α(n)) amortized complexity (nearly constant time).
//!
//! # Design
//!
//! Based on Gleam's unification approach:
//! - Variables are linked directly to their unified type (no substitution maps)
//! - Path compression shortens chains during resolution
//! - Flag-gated occurs check skips traversal when `HAS_VAR` is false
//! - Rich error context for helpful diagnostics
//!
//! # Usage
//!
//! ```ignore
//! let mut pool = Pool::new();
//! let mut engine = UnifyEngine::new(&mut pool);
//!
//! let var = engine.fresh_var();
//! engine.unify(var, Idx::INT)?;
//!
//! // Now var resolves to INT
//! assert_eq!(engine.resolve(var), Idx::INT);
//! ```

mod error;
mod rank;

pub use error::{ArityKind, UnifyContext, UnifyError};
pub use rank::Rank;

use rustc_hash::FxHashMap;

use crate::{Idx, Pool, Tag, TypeFlags, VarState};

/// The unification engine.
///
/// Handles type variable resolution and unification with:
/// - Link-based union-find for O(α(n)) unification
/// - Path compression for efficient resolution
/// - Rank tracking for let-polymorphism
pub struct UnifyEngine<'pool> {
    /// The type pool (mutable access for setting links).
    pool: &'pool mut Pool,
    /// Current rank (scope depth) for new variables.
    current_rank: Rank,
    /// Accumulated errors (allows continuing after errors).
    errors: Vec<UnifyError>,
}

impl<'pool> UnifyEngine<'pool> {
    /// Create a new unification engine.
    pub fn new(pool: &'pool mut Pool) -> Self {
        Self {
            pool,
            current_rank: Rank::FIRST,
            errors: Vec::new(),
        }
    }

    /// Get the current rank.
    #[inline]
    pub fn current_rank(&self) -> Rank {
        self.current_rank
    }

    /// Enter a new scope (increase rank).
    ///
    /// Variables created at higher ranks can be generalized
    /// when the scope exits.
    pub fn enter_scope(&mut self) {
        self.current_rank = self.current_rank.next();
    }

    /// Exit current scope (decrease rank).
    ///
    /// Call `generalize()` on types before exiting to capture
    /// variables that should be generalized.
    pub fn exit_scope(&mut self) {
        self.current_rank = self.current_rank.prev().max(Rank::FIRST);
    }

    /// Create a fresh unbound type variable at current rank.
    pub fn fresh_var(&mut self) -> Idx {
        self.pool.fresh_var_with_rank(self.current_rank)
    }

    /// Create a fresh named type variable at current rank.
    pub fn fresh_named_var(&mut self, name: ori_ir::Name) -> Idx {
        self.pool.fresh_named_var_with_rank(name, self.current_rank)
    }

    /// Get read-only access to the pool.
    #[inline]
    pub fn pool(&self) -> &Pool {
        self.pool
    }

    /// Get mutable access to the pool (for type construction).
    #[inline]
    pub fn pool_mut(&mut self) -> &mut Pool {
        self.pool
    }

    /// Take accumulated errors, leaving an empty vector.
    pub fn take_errors(&mut self) -> Vec<UnifyError> {
        std::mem::take(&mut self.errors)
    }

    /// Check if any errors occurred.
    #[inline]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get accumulated errors.
    #[inline]
    pub fn errors(&self) -> &[UnifyError] {
        &self.errors
    }

    // ========================================
    // Resolution
    // ========================================

    /// Resolve a type by following links.
    ///
    /// Implements path compression: intermediate links are updated
    /// to point directly to the final target, giving O(α(n)) amortized.
    pub fn resolve(&mut self, idx: Idx) -> Idx {
        // Fast path: not a variable
        if self.pool.tag(idx) != Tag::Var {
            return idx;
        }

        let var_id = self.pool.data(idx);
        let state = self.pool.var_state(var_id);

        match state {
            VarState::Link { target } => {
                let target = *target;
                // Recursively resolve
                let resolved = self.resolve(target);

                // Path compression: update to point directly to final
                if resolved != target {
                    *self.pool.var_state_mut(var_id) = VarState::Link { target: resolved };
                }

                resolved
            }
            // Unbound, Rigid, Generalized all return the variable itself
            _ => idx,
        }
    }

    /// Resolve without mutation (for read-only queries).
    ///
    /// Follows links but doesn't apply path compression.
    pub fn resolve_readonly(&self, idx: Idx) -> Idx {
        // Fast path: not a variable
        if self.pool.tag(idx) != Tag::Var {
            return idx;
        }

        let var_id = self.pool.data(idx);
        let state = self.pool.var_state(var_id);

        match state {
            VarState::Link { target } => self.resolve_readonly(*target),
            _ => idx,
        }
    }

    // ========================================
    // Unification
    // ========================================

    /// Unify two types, making them equivalent.
    ///
    /// Returns `Ok(())` if unification succeeds.
    /// Returns `Err(UnifyError)` on failure.
    ///
    /// After successful unification, both types will resolve to the same type.
    pub fn unify(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
        self.unify_with_context(a, b, UnifyContext::TopLevel)
    }

    /// Unify with explicit context for better error messages.
    pub fn unify_with_context(
        &mut self,
        a: Idx,
        b: Idx,
        context: UnifyContext,
    ) -> Result<(), UnifyError> {
        // Fast path: identical indices
        if a == b {
            return Ok(());
        }

        // Resolve both sides
        let a = self.resolve(a);
        let b = self.resolve(b);

        // After resolution, check again
        if a == b {
            return Ok(());
        }

        // Get flags for early exits
        let a_flags = self.pool.flags(a);
        let b_flags = self.pool.flags(b);

        // Error type propagates (don't report cascading errors)
        if a_flags.contains(TypeFlags::HAS_ERROR) || b_flags.contains(TypeFlags::HAS_ERROR) {
            return Ok(());
        }

        // Never type unifies with anything (bottom type)
        let a_tag = self.pool.tag(a);
        let b_tag = self.pool.tag(b);

        if a_tag == Tag::Never || b_tag == Tag::Never {
            return Ok(());
        }

        // Dispatch based on types
        match (a_tag, b_tag) {
            // Variable on left
            (Tag::Var, _) => self.unify_var_with(a, b, context),

            // Variable on right (swap to normalize)
            (_, Tag::Var) => self.unify_var_with(b, a, context),

            // Rigid variables
            (Tag::RigidVar, Tag::RigidVar) => self.unify_rigid_rigid(a, b),
            (Tag::RigidVar, _) => {
                let name = self.get_rigid_name(a);
                Err(UnifyError::RigidMismatch {
                    rigid_name: name,
                    concrete: b,
                })
            }
            (_, Tag::RigidVar) => {
                let name = self.get_rigid_name(b);
                Err(UnifyError::RigidMismatch {
                    rigid_name: name,
                    concrete: a,
                })
            }

            // Structural unification for concrete types
            _ => self.unify_structural(a, b, context),
        }
    }

    /// Unify a variable with another type.
    fn unify_var_with(
        &mut self,
        var_idx: Idx,
        other: Idx,
        context: UnifyContext,
    ) -> Result<(), UnifyError> {
        let var_id = self.pool.data(var_idx);

        // Occurs check: prevent infinite types
        if self.occurs(var_id, other) {
            return Err(UnifyError::InfiniteType {
                var_id,
                containing_type: other,
            });
        }

        // Get variable state
        let state = self.pool.var_state(var_id).clone();

        match state {
            VarState::Unbound { rank, .. } => {
                // Update ranks of variables in `other` to be at most `rank`
                self.update_ranks(other, rank);

                // Set link
                *self.pool.var_state_mut(var_id) = VarState::Link { target: other };
                Ok(())
            }

            VarState::Link { target } => {
                // Should not happen after resolve(), but handle it
                self.unify_with_context(target, other, context)
            }

            VarState::Rigid { name } => Err(UnifyError::RigidMismatch {
                rigid_name: name,
                concrete: other,
            }),

            VarState::Generalized { id, .. } => {
                // Generalized variables should be instantiated before unification.
                // This is a compiler invariant violation, not a user error.
                tracing::error!(
                    var_id = id,
                    "attempted to unify generalized variable without instantiation"
                );
                Err(UnifyError::UninstantiatedGeneralized { var_id: id })
            }
        }
    }

    /// Unify two rigid variables.
    fn unify_rigid_rigid(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
        // Rigid variables can only unify if they're the same variable
        let a_id = self.pool.data(a);
        let b_id = self.pool.data(b);

        if a_id == b_id {
            Ok(())
        } else {
            let name1 = self.get_rigid_name(a);
            let name2 = self.get_rigid_name(b);
            Err(UnifyError::RigidRigidMismatch {
                rigid1: name1,
                rigid2: name2,
            })
        }
    }

    /// Get the name of a rigid variable.
    fn get_rigid_name(&self, idx: Idx) -> ori_ir::Name {
        let var_id = self.pool.data(idx);
        match self.pool.var_state(var_id) {
            VarState::Rigid { name } => *name,
            _ => panic!("Expected rigid variable"),
        }
    }

    // ========================================
    // Occurs Check
    // ========================================

    /// Check if variable `var_id` occurs in type `ty`.
    ///
    /// This is flag-gated: if the type has no variables (`HAS_VAR` is false),
    /// we skip the expensive traversal entirely.
    fn occurs(&self, var_id: u32, ty: Idx) -> bool {
        // Fast path: no variables in type
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return false;
        }

        self.occurs_inner(var_id, ty)
    }

    /// Inner occurs check that traverses the type structure.
    fn occurs_inner(&self, var_id: u32, ty: Idx) -> bool {
        let tag = self.pool.tag(ty);

        match tag {
            Tag::Var => {
                let other_id = self.pool.data(ty);
                if other_id == var_id {
                    return true;
                }
                // Follow link if present
                if let VarState::Link { target } = self.pool.var_state(other_id) {
                    return self.occurs_inner(var_id, *target);
                }
                false
            }

            // Simple containers
            Tag::List | Tag::Option | Tag::Set | Tag::Channel | Tag::Range | Tag::Iterator => {
                let child = Idx::from_raw(self.pool.data(ty));
                self.occurs_inner(var_id, child)
            }

            // Two-child containers
            Tag::Map => {
                let key = self.pool.map_key(ty);
                let value = self.pool.map_value(ty);
                self.occurs_inner(var_id, key) || self.occurs_inner(var_id, value)
            }

            Tag::Result => {
                let ok = self.pool.result_ok(ty);
                let err = self.pool.result_err(ty);
                self.occurs_inner(var_id, ok) || self.occurs_inner(var_id, err)
            }

            Tag::Borrowed => {
                let inner = self.pool.borrowed_inner(ty);
                self.occurs_inner(var_id, inner)
            }

            // Functions
            Tag::Function => {
                let params = self.pool.function_params(ty);
                let ret = self.pool.function_return(ty);
                params.iter().any(|&p| self.occurs_inner(var_id, p))
                    || self.occurs_inner(var_id, ret)
            }

            // Tuples
            Tag::Tuple => {
                let elems = self.pool.tuple_elems(ty);
                elems.iter().any(|&e| self.occurs_inner(var_id, e))
            }

            // Applied types
            Tag::Applied => {
                let args = self.pool.applied_args(ty);
                args.iter().any(|&a| self.occurs_inner(var_id, a))
            }

            // Schemes (check body)
            Tag::Scheme => {
                let body = self.pool.scheme_body(ty);
                self.occurs_inner(var_id, body)
            }

            // Other types don't contain variables
            _ => false,
        }
    }

    // ========================================
    // Rank Updates
    // ========================================

    /// Update ranks of all unbound variables in `ty` to be at most `max_rank`.
    ///
    /// This ensures that when a variable at rank R is unified with a type,
    /// all variables in that type get promoted to rank R (or lower).
    fn update_ranks(&mut self, ty: Idx, max_rank: Rank) {
        // Fast path: no variables
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return;
        }

        self.update_ranks_inner(ty, max_rank);
    }

    fn update_ranks_inner(&mut self, ty: Idx, max_rank: Rank) {
        let tag = self.pool.tag(ty);

        match tag {
            Tag::Var => {
                let var_id = self.pool.data(ty);
                let state = self.pool.var_state_mut(var_id);

                match state {
                    VarState::Unbound { rank, .. } => {
                        if *rank > max_rank {
                            *rank = max_rank;
                        }
                    }
                    VarState::Link { target } => {
                        let target = *target;
                        self.update_ranks_inner(target, max_rank);
                    }
                    _ => {}
                }
            }

            Tag::List | Tag::Option | Tag::Set | Tag::Channel | Tag::Range | Tag::Iterator => {
                let child = Idx::from_raw(self.pool.data(ty));
                self.update_ranks_inner(child, max_rank);
            }

            Tag::Map => {
                let key = self.pool.map_key(ty);
                let value = self.pool.map_value(ty);
                self.update_ranks_inner(key, max_rank);
                self.update_ranks_inner(value, max_rank);
            }

            Tag::Result => {
                let ok = self.pool.result_ok(ty);
                let err = self.pool.result_err(ty);
                self.update_ranks_inner(ok, max_rank);
                self.update_ranks_inner(err, max_rank);
            }

            Tag::Borrowed => {
                let inner = self.pool.borrowed_inner(ty);
                self.update_ranks_inner(inner, max_rank);
            }

            Tag::Function => {
                let params = self.pool.function_params(ty);
                let ret = self.pool.function_return(ty);
                for p in params {
                    self.update_ranks_inner(p, max_rank);
                }
                self.update_ranks_inner(ret, max_rank);
            }

            Tag::Tuple => {
                let elems = self.pool.tuple_elems(ty);
                for e in elems {
                    self.update_ranks_inner(e, max_rank);
                }
            }

            Tag::Applied => {
                let args = self.pool.applied_args(ty);
                for a in args {
                    self.update_ranks_inner(a, max_rank);
                }
            }

            Tag::Scheme => {
                let body = self.pool.scheme_body(ty);
                self.update_ranks_inner(body, max_rank);
            }

            _ => {}
        }
    }

    // ========================================
    // Structural Unification
    // ========================================

    /// Unify two concrete (non-variable) types structurally.
    fn unify_structural(
        &mut self,
        a: Idx,
        b: Idx,
        context: UnifyContext,
    ) -> Result<(), UnifyError> {
        let tag_a = self.pool.tag(a);
        let tag_b = self.pool.tag(b);

        // Tags must match
        if tag_a != tag_b {
            return Err(UnifyError::Mismatch {
                expected: a,
                found: b,
                context,
            });
        }

        match tag_a {
            // Primitives: same tag means equal
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Str
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Error
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering => Ok(()),

            // Simple containers
            Tag::List => {
                let child_a = Idx::from_raw(self.pool.data(a));
                let child_b = Idx::from_raw(self.pool.data(b));
                self.unify_with_context(child_a, child_b, UnifyContext::ListElement)
            }

            Tag::Option => {
                let child_a = Idx::from_raw(self.pool.data(a));
                let child_b = Idx::from_raw(self.pool.data(b));
                self.unify_with_context(child_a, child_b, UnifyContext::OptionInner)
            }

            Tag::Set => {
                let child_a = Idx::from_raw(self.pool.data(a));
                let child_b = Idx::from_raw(self.pool.data(b));
                self.unify_with_context(child_a, child_b, UnifyContext::SetElement)
            }

            Tag::Channel => {
                let child_a = Idx::from_raw(self.pool.data(a));
                let child_b = Idx::from_raw(self.pool.data(b));
                self.unify_with_context(child_a, child_b, UnifyContext::ChannelElement)
            }

            Tag::Range => {
                let child_a = Idx::from_raw(self.pool.data(a));
                let child_b = Idx::from_raw(self.pool.data(b));
                self.unify_with_context(child_a, child_b, UnifyContext::RangeElement)
            }

            Tag::Iterator => {
                let child_a = Idx::from_raw(self.pool.data(a));
                let child_b = Idx::from_raw(self.pool.data(b));
                self.unify_with_context(child_a, child_b, UnifyContext::IteratorElement)
            }

            // Two-child containers
            Tag::Map => {
                let key_a = self.pool.map_key(a);
                let key_b = self.pool.map_key(b);
                let val_a = self.pool.map_value(a);
                let val_b = self.pool.map_value(b);

                self.unify_with_context(key_a, key_b, UnifyContext::MapKey)?;
                self.unify_with_context(val_a, val_b, UnifyContext::MapValue)
            }

            Tag::Result => {
                let ok_a = self.pool.result_ok(a);
                let ok_b = self.pool.result_ok(b);
                let err_a = self.pool.result_err(a);
                let err_b = self.pool.result_err(b);

                self.unify_with_context(ok_a, ok_b, UnifyContext::ResultOk)?;
                self.unify_with_context(err_a, err_b, UnifyContext::ResultErr)
            }

            Tag::Borrowed => {
                let inner_a = self.pool.borrowed_inner(a);
                let inner_b = self.pool.borrowed_inner(b);
                let lt_a = self.pool.borrowed_lifetime(a);
                let lt_b = self.pool.borrowed_lifetime(b);

                if lt_a != lt_b {
                    return Err(UnifyError::Mismatch {
                        expected: a,
                        found: b,
                        context,
                    });
                }
                self.unify_with_context(inner_a, inner_b, UnifyContext::BorrowedInner)
            }

            // Functions
            Tag::Function => {
                let params_a = self.pool.function_params(a);
                let params_b = self.pool.function_params(b);
                let ret_a = self.pool.function_return(a);
                let ret_b = self.pool.function_return(b);

                if params_a.len() != params_b.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: params_a.len(),
                        found: params_b.len(),
                        kind: ArityKind::Function,
                    });
                }

                for (i, (pa, pb)) in params_a.iter().zip(params_b.iter()).enumerate() {
                    self.unify_with_context(*pa, *pb, UnifyContext::param(i))?;
                }

                self.unify_with_context(ret_a, ret_b, UnifyContext::FunctionReturn)
            }

            // Tuples
            Tag::Tuple => {
                let elems_a = self.pool.tuple_elems(a);
                let elems_b = self.pool.tuple_elems(b);

                if elems_a.len() != elems_b.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: elems_a.len(),
                        found: elems_b.len(),
                        kind: ArityKind::Tuple,
                    });
                }

                for (i, (ea, eb)) in elems_a.iter().zip(elems_b.iter()).enumerate() {
                    self.unify_with_context(*ea, *eb, UnifyContext::tuple_elem(i))?;
                }

                Ok(())
            }

            // Named types: must have same name
            Tag::Named => {
                let name_a = self.pool.named_name(a);
                let name_b = self.pool.named_name(b);

                if name_a == name_b {
                    Ok(())
                } else {
                    Err(UnifyError::Mismatch {
                        expected: a,
                        found: b,
                        context,
                    })
                }
            }

            // Applied types: same name and unify args
            Tag::Applied => {
                let name_a = self.pool.applied_name(a);
                let name_b = self.pool.applied_name(b);

                if name_a != name_b {
                    return Err(UnifyError::Mismatch {
                        expected: a,
                        found: b,
                        context,
                    });
                }

                let args_a = self.pool.applied_args(a);
                let args_b = self.pool.applied_args(b);

                if args_a.len() != args_b.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: args_a.len(),
                        found: args_b.len(),
                        kind: ArityKind::TypeArgs,
                    });
                }

                for (i, (aa, ab)) in args_a.iter().zip(args_b.iter()).enumerate() {
                    self.unify_with_context(*aa, *ab, UnifyContext::type_arg(i))?;
                }

                Ok(())
            }

            // Other types: just check tag equality
            _ => Err(UnifyError::Mismatch {
                expected: a,
                found: b,
                context,
            }),
        }
    }

    // ========================================
    // Generalization
    // ========================================

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

        // Mark collected variables as generalized
        for &var_id in &vars {
            let state = self.pool.var_state_mut(var_id);
            if let VarState::Unbound { id, name, .. } = state.clone() {
                *state = VarState::Generalized { id, name };
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

            Tag::List | Tag::Option | Tag::Set | Tag::Channel | Tag::Range | Tag::Iterator => {
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

    // ========================================
    // Instantiation
    // ========================================

    /// Instantiate a type scheme with fresh variables.
    ///
    /// For each quantified variable in the scheme, creates a fresh unbound
    /// variable at the current rank, then substitutes throughout the body.
    ///
    /// Returns the type unchanged if it's not a scheme.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Given scheme: ∀a. a -> a
    /// let concrete = engine.instantiate(scheme);  // $1 -> $1 (fresh var)
    /// engine.unify(concrete_param, Idx::INT);     // $1 unified with int
    /// // Now concrete is: int -> int
    /// ```
    pub fn instantiate(&mut self, scheme_idx: Idx) -> Idx {
        if self.pool.tag(scheme_idx) != Tag::Scheme {
            return scheme_idx; // Not a scheme, return as-is
        }

        let vars = self.pool.scheme_vars(scheme_idx).to_vec();
        let body = self.pool.scheme_body(scheme_idx);

        if vars.is_empty() {
            return body; // Monomorphic scheme
        }

        // Create fresh variables for each quantified variable
        let mut subst: FxHashMap<u32, Idx> = FxHashMap::default();
        for var_id in vars {
            let fresh = self.fresh_var();
            subst.insert(var_id, fresh);
        }

        // Substitute in the body
        self.substitute(body, &subst)
    }

    /// Substitute variables according to the given mapping.
    ///
    /// Returns the original type if no substitutions apply.
    fn substitute(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        // Fast path: no variables to substitute
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return ty;
        }

        match self.pool.tag(ty) {
            Tag::Var => {
                let var_id = self.pool.data(ty);

                // Check if this variable should be substituted
                if let Some(&replacement) = subst.get(&var_id) {
                    return replacement;
                }

                // Follow link if present
                if let VarState::Link { target } = self.pool.var_state(var_id) {
                    return self.substitute(*target, subst);
                }

                // Check for generalized variable
                if let VarState::Generalized { id, .. } = self.pool.var_state(var_id) {
                    if let Some(&replacement) = subst.get(id) {
                        return replacement;
                    }
                }

                ty
            }

            Tag::List => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.list(new_child)
                }
            }

            Tag::Option => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.option(new_child)
                }
            }

            Tag::Set => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.set(new_child)
                }
            }

            Tag::Channel => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.channel(new_child)
                }
            }

            Tag::Range => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.range(new_child)
                }
            }

            Tag::Iterator => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.iterator(new_child)
                }
            }

            Tag::Map => {
                let key = self.pool.map_key(ty);
                let value = self.pool.map_value(ty);
                let new_key = self.substitute(key, subst);
                let new_value = self.substitute(value, subst);
                if new_key == key && new_value == value {
                    ty
                } else {
                    self.pool.map(new_key, new_value)
                }
            }

            Tag::Result => {
                let ok = self.pool.result_ok(ty);
                let err = self.pool.result_err(ty);
                let new_ok = self.substitute(ok, subst);
                let new_err = self.substitute(err, subst);
                if new_ok == ok && new_err == err {
                    ty
                } else {
                    self.pool.result(new_ok, new_err)
                }
            }

            Tag::Borrowed => {
                let inner = self.pool.borrowed_inner(ty);
                let lt = self.pool.borrowed_lifetime(ty);
                let new_inner = self.substitute(inner, subst);
                if new_inner == inner {
                    ty
                } else {
                    self.pool.borrowed(new_inner, lt)
                }
            }

            Tag::Function => {
                let params = self.pool.function_params(ty);
                let ret = self.pool.function_return(ty);

                let mut changed = false;
                let new_params: Vec<Idx> = params
                    .iter()
                    .map(|&p| {
                        let new_p = self.substitute(p, subst);
                        if new_p != p {
                            changed = true;
                        }
                        new_p
                    })
                    .collect();

                let new_ret = self.substitute(ret, subst);
                if new_ret != ret {
                    changed = true;
                }

                if changed {
                    self.pool.function(&new_params, new_ret)
                } else {
                    ty
                }
            }

            Tag::Tuple => {
                let elems = self.pool.tuple_elems(ty);

                let mut changed = false;
                let new_elems: Vec<Idx> = elems
                    .iter()
                    .map(|&e| {
                        let new_e = self.substitute(e, subst);
                        if new_e != e {
                            changed = true;
                        }
                        new_e
                    })
                    .collect();

                if changed {
                    self.pool.tuple(&new_elems)
                } else {
                    ty
                }
            }

            Tag::Applied => {
                let name = self.pool.applied_name(ty);
                let args = self.pool.applied_args(ty);

                let mut changed = false;
                let new_args: Vec<Idx> = args
                    .iter()
                    .map(|&a| {
                        let new_a = self.substitute(a, subst);
                        if new_a != a {
                            changed = true;
                        }
                        new_a
                    })
                    .collect();

                if changed {
                    self.pool.applied(name, &new_args)
                } else {
                    ty
                }
            }

            // Schemes have their own bound variables, other types don't contain variables
            _ => ty,
        }
    }
}

#[cfg(test)]
mod tests;
