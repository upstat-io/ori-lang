//! Static uniqueness analysis for COW check elimination.
//!
//! Determines at compile time whether collection values are uniquely owned
//! (RC == 1) at mutation points, allowing the codegen to emit only the fast
//! path — no runtime `ori_rc_is_unique()` check, no branch, no slow path code.
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
//!
//! The lattice join (`Unique ⊔ Unique = Unique`, `Shared ⊔ Shared = Shared`,
//! otherwise `MaybeShared`) models control flow merges. Monotonic descent
//! guarantees termination of fixpoint iteration.
//!
//! # Key Insight
//!
//! **COW operations always produce `Unique` results.** Whether the fast path
//! (in-place mutation, RC was 1) or slow path (copy, new allocation with
//! RC = 1) executes, the output has exactly one reference.
//!
//! # References
//!
//! - Lean 4 `Borrow.lean`: iterative fixpoint borrow/ownership inference
//! - Koka `Parc.hs`: reverse-liveness ownership tracking
//! - Roc `morphic_lib/analyze.rs`: full alias analysis with `UpdateMode`
//! - Swift `ARCSequenceOpts.cpp`: dataflow with lattice-based RC state

mod annotations;
pub mod drop_hints;
pub mod inter;
pub mod intra;
mod lattice;
mod summary;

pub use annotations::{compute_cow_annotations, CowAnnotations};
pub use drop_hints::{compute_drop_hints, DropHints};
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
    /// This is correct in SSA form: any variable reachable at a join
    /// point dominates all predecessors, so it is present in all maps.
    /// Variables unique to one predecessor are unreachable after the
    /// join and thus harmless.
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

#[cfg(test)]
mod tests;
