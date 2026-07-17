//! Variable resolution, occurs checks, and rank maintenance.

use crate::{GeneralizedVarState, Idx, Tag, TypeFlags, UnboundVarState, VarState};

use super::{Rank, UnifyContext, UnifyEngine, UnifyError};

impl UnifyEngine<'_> {
    // Resolution

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

    /// Unify a variable with another type.
    pub(super) fn unify_var_with(
        &mut self,
        var_idx: Idx,
        other: Idx,
        context: UnifyContext,
    ) -> Result<(), UnifyError> {
        /// Extracted action from var state for borrow-splitting.
        enum Action {
            Link(crate::Rank),
            Follow(crate::Idx),
            Rigid(ori_ir::Name),
            GenError(u32),
        }

        let var_id = self.pool.data(var_idx);

        // Occurs check: prevent infinite types
        if self.occurs(var_id, other) {
            return Err(UnifyError::InfiniteType {
                var_id,
                containing_type: other,
            });
        }

        // Extract action from var state without cloning (all fields are Copy).
        // The match borrows pool immutably; we drop the borrow before mutating.
        let action = match self.pool.var_state(var_id) {
            VarState::Unbound(UnboundVarState { rank, .. }) => Action::Link(*rank),
            VarState::Link { target } => Action::Follow(*target),
            VarState::Rigid { name } => Action::Rigid(*name),
            VarState::Generalized(GeneralizedVarState { id, .. }) => Action::GenError(*id),
        };

        match action {
            Action::Link(rank) => {
                self.update_ranks(other, rank);
                *self.pool.var_state_mut(var_id) = VarState::Link { target: other };
                Ok(())
            }
            Action::Follow(target) => {
                // Should not happen after resolve(), but handle it
                self.unify_with_context(target, other, context)
            }
            Action::Rigid(name) => Err(UnifyError::RigidMismatch {
                rigid_name: name,
                concrete: other,
            }),
            Action::GenError(id) => {
                tracing::error!(
                    var_id = id,
                    "attempted to unify generalized variable without instantiation"
                );
                Err(UnifyError::UninstantiatedGeneralized { var_id: id })
            }
        }
    }

    /// Unify two rigid variables.
    pub(super) fn unify_rigid_rigid(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
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
    pub(super) fn get_rigid_name(&self, idx: Idx) -> ori_ir::Name {
        let var_id = self.pool.data(idx);
        match self.pool.var_state(var_id) {
            VarState::Rigid { name } => *name,
            _ => panic!("Expected rigid variable"),
        }
    }

    // Occurs Check

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
            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => {
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

    // Rank Updates

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
                    VarState::Unbound(UnboundVarState { rank, .. }) => {
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

            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => {
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
}
