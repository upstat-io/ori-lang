//! Link-based type unification with path compression.
//!
//! Variables link directly to resolved types, ranks govern generalization, and
//! flag-gated occurs checks skip types that cannot contain inference variables.

mod error;
mod generalization;
mod rank;
mod structural;
mod substitute;
mod variables;

pub use error::{ArityKind, UnifyContext, UnifyError};
pub use rank::Rank;

use crate::{Idx, Pool};

/// Link-based unification state with path compression and rank generalization.
#[derive(Debug)]
pub struct UnifyEngine<'pool> {
    /// The type pool (mutable access for setting links).
    pub(super) pool: &'pool mut Pool,
    /// Current rank (scope depth) for new variables.
    pub(super) current_rank: Rank,
    /// Accumulated errors (allows continuing after errors).
    pub(super) errors: Vec<UnifyError>,
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
}

#[cfg(test)]
mod tests;
