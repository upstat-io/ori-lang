//! Uniqueness types for COW check elimination.
//!
//! Provides the [`Uniqueness`] lattice, [`CowMode`] annotation, and container
//! types ([`CowAnnotations`], [`DropHints`], [`UniquenessMap`], [`UniquenessSummary`])
//! used by both the AIMS pipeline and LLVM codegen.
//!
//! # Lattice
//!
//! The analysis operates over a three-point uniqueness lattice:
//!
//! ```text
//!          Unique
//!         /      \
//!    MaybeShared
//!         \      /
//!          Shared
//! ```
//!
//! - **Unique**: provably RC == 1. COW check can be eliminated.
//! - **`MaybeShared`**: unknown. Runtime check needed (conservative default).
//! - **Shared**: provably RC > 1. Slow path always taken.

mod annotations;
pub mod drop_hints;
mod lattice;
mod summary;

pub use annotations::CowAnnotations;
pub use drop_hints::DropHints;
pub use lattice::{CowMode, Uniqueness};
pub use summary::UniquenessSummary;

use rustc_hash::FxHashMap;

use crate::ir::ArcVarId;

/// Maps each variable in a function to its uniqueness state.
///
/// Backed by a `HashMap<ArcVarId, Uniqueness>`. Variables not present
/// in the map are implicitly `MaybeShared` (conservative default).
#[derive(Clone, Debug, PartialEq)]
pub struct UniquenessMap {
    states: FxHashMap<ArcVarId, Uniqueness>,
}

impl UniquenessMap {
    /// Create an empty uniqueness map.
    pub fn new() -> Self {
        Self {
            states: FxHashMap::default(),
        }
    }

    /// Create a map with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            states: FxHashMap::with_capacity_and_hasher(capacity, rustc_hash::FxBuildHasher),
        }
    }

    /// Get the uniqueness state of a variable.
    ///
    /// Returns `MaybeShared` for variables not explicitly tracked.
    #[inline]
    pub fn get(&self, var: ArcVarId) -> Uniqueness {
        self.states
            .get(&var)
            .copied()
            .unwrap_or(Uniqueness::MaybeShared)
    }

    /// Set the uniqueness state of a variable.
    #[inline]
    pub fn set(&mut self, var: ArcVarId, state: Uniqueness) {
        self.states.insert(var, state);
    }

    /// Mark a variable as `Unique`.
    #[inline]
    pub fn mark_unique(&mut self, var: ArcVarId) {
        self.states.insert(var, Uniqueness::Unique);
    }

    /// Mark a variable as `Shared`.
    #[inline]
    pub fn mark_shared(&mut self, var: ArcVarId) {
        self.states.insert(var, Uniqueness::Shared);
    }

    /// Join the state of a variable with a new state (lattice join).
    ///
    /// Used at control flow merge points. If the variable is not tracked,
    /// the implicit `MaybeShared` is used as the base.
    #[inline]
    pub fn join(&mut self, var: ArcVarId, state: Uniqueness) {
        let current = self.get(var);
        self.states.insert(var, current.join(state));
    }

    /// Merge another map into this one using lattice join.
    ///
    /// For each variable present in either map, the result is the join
    /// of both states. Variables only in `other` are joined with the
    /// implicit `MaybeShared`. Variables only in `self` are unaffected.
    pub fn join_from(&mut self, other: &Self) {
        for (&var, &state) in &other.states {
            self.join(var, state);
        }
    }

    /// Number of explicitly tracked variables.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Whether the map has no explicitly tracked variables.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Iterate over all explicitly tracked (variable, state) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (ArcVarId, Uniqueness)> + '_ {
        self.states.iter().map(|(&var, &state)| (var, state))
    }

    /// Determine the [`CowMode`] for a variable at a COW operation site.
    #[inline]
    pub fn cow_mode(&self, var: ArcVarId) -> CowMode {
        CowMode::from_uniqueness(self.get(var))
    }
}

impl Default for UniquenessMap {
    fn default() -> Self {
        Self::new()
    }
}
