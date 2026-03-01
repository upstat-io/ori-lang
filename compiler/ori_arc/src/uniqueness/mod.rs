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

pub mod inter;
pub mod intra;

use std::fmt;
use std::hash::{Hash, Hasher};

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcInstr, ArcVarId};
use crate::liveness::BlockLiveness;
use crate::ArcClassification;

/// Abstract uniqueness state for a value.
///
/// Classifies whether a value is provably uniquely owned (RC == 1),
/// provably shared (RC > 1), or unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Uniqueness {
    /// Provably uniquely owned. RC is guaranteed to be 1.
    /// The runtime COW check can be eliminated — emit only the fast path.
    Unique,

    /// May or may not be shared. The runtime check is needed.
    /// This is the conservative default for function parameters and
    /// values with unknown provenance.
    MaybeShared,

    /// Provably shared (RC > 1). The slow path is always taken.
    /// Rare in practice — mostly for values bound to multiple variables
    /// without intervening COW operations.
    Shared,
}

impl Uniqueness {
    /// Lattice join (least upper bound).
    ///
    /// Used at control flow merge points (if/else, match arms) to combine
    /// the uniqueness states from all incoming edges.
    ///
    /// ```text
    /// Unique ⊔ Unique = Unique
    /// Shared ⊔ Shared = Shared
    /// Unique ⊔ Shared = MaybeShared
    /// MaybeShared ⊔ _ = MaybeShared
    /// ```
    #[inline]
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unique, Self::Unique) => Self::Unique,
            (Self::Shared, Self::Shared) => Self::Shared,
            _ => Self::MaybeShared,
        }
    }

    /// Whether the runtime COW check can be eliminated.
    #[inline]
    pub fn is_unique(self) -> bool {
        self == Self::Unique
    }

    /// Whether the runtime COW check is definitely needed.
    #[inline]
    pub fn is_maybe_shared(self) -> bool {
        self == Self::MaybeShared
    }

    /// Whether the value is provably shared (slow path always).
    #[inline]
    pub fn is_shared(self) -> bool {
        self == Self::Shared
    }
}

impl fmt::Display for Uniqueness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unique => write!(f, "unique"),
            Self::MaybeShared => write!(f, "maybe_shared"),
            Self::Shared => write!(f, "shared"),
        }
    }
}

/// Annotation on a COW operation indicating whether the runtime uniqueness
/// check can be eliminated.
///
/// Produced by static uniqueness analysis and consumed by LLVM codegen.
/// When a collection operation (push, insert, etc.) is annotated with
/// `StaticUnique`, the codegen emits only the fast (in-place) path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CowMode {
    /// Runtime check needed: `if ori_rc_is_unique(ptr) { fast } else { slow }`.
    /// Default when uniqueness cannot be proven statically.
    Dynamic,

    /// Statically proven unique — emit only the fast path.
    /// No `ori_rc_is_unique` call, no branch, no slow path code.
    StaticUnique,

    /// Statically proven shared — emit only the slow path.
    /// Rare; useful when a value is provably aliased (e.g., bound twice).
    StaticShared,
}

impl CowMode {
    /// Derive the `CowMode` from a `Uniqueness` state.
    #[inline]
    pub fn from_uniqueness(u: Uniqueness) -> Self {
        match u {
            Uniqueness::Unique => Self::StaticUnique,
            Uniqueness::MaybeShared => Self::Dynamic,
            Uniqueness::Shared => Self::StaticShared,
        }
    }
}

impl fmt::Display for CowMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dynamic => write!(f, "dynamic"),
            Self::StaticUnique => write!(f, "static_unique"),
            Self::StaticShared => write!(f, "static_shared"),
        }
    }
}

/// Summary of a function's uniqueness behavior for interprocedural analysis.
///
/// Captures the uniqueness of each parameter and the return value, enabling
/// callers to determine the uniqueness of call results without re-analyzing
/// the callee.
///
/// Produced by [`inter::analyze_program`] and consumed by
/// [`intra::analyze_with_summaries`] to refine `Apply` results from the
/// conservative `MaybeShared` default to a precise uniqueness state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniquenessSummary {
    /// Uniqueness of each parameter at function entry.
    ///
    /// Currently all `MaybeShared` (callers may share), but included for
    /// future interprocedural parameter refinement.
    ///
    /// **Note:** For builtin/COW summaries (from [`inter::build_cow_summaries`]),
    /// this is empty — only `return_val` is meaningful. Do not index into
    /// `params` without checking length first; use `.get(i)` or guard with
    /// `if i < summary.params.len()`.
    pub params: Vec<Uniqueness>,

    /// Uniqueness of the function's return value.
    ///
    /// - `Unique`: the function always returns a fresh allocation (e.g.,
    ///   constructor, COW operation result).
    /// - `MaybeShared`: the function may return a parameter or captured value.
    /// - `Shared`: the function always returns a known-shared value (rare).
    pub return_val: Uniqueness,

    /// Whether the function is "freshness-preserving": if all RC'd inputs
    /// are `Unique`, the output is guaranteed `Unique`.
    ///
    /// True for functions that only construct fresh values or apply COW
    /// operations. False for functions that may return a parameter directly.
    pub preserves_freshness: bool,
}

impl UniquenessSummary {
    /// Create a conservative summary where everything is `MaybeShared`.
    pub fn conservative(num_params: usize) -> Self {
        Self {
            params: vec![Uniqueness::MaybeShared; num_params],
            return_val: Uniqueness::MaybeShared,
            preserves_freshness: false,
        }
    }
}

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

// -- COW annotations --

/// Per-instruction COW mode annotations for LLVM codegen.
///
/// Maps `(block_index, instruction_index)` to [`CowMode`] for each
/// COW operation in a function. Produced by [`compute_cow_annotations`]
/// during the ARC pipeline and consumed by the LLVM arc emitter.
///
/// Instructions not present in the map default to [`CowMode::Dynamic`]
/// (runtime uniqueness check needed).
///
/// # Identity
///
/// `CowAnnotations` is **derived data** (like [`crate::ir::ValueRepr`]),
/// not part of a function's structural identity. `PartialEq`/`Eq`/`Hash`
/// are implemented as no-ops to avoid breaking `ArcFunction`'s derives.
#[derive(Clone, Debug, Default)]
pub struct CowAnnotations {
    annotations: FxHashMap<(usize, usize), CowMode>,
}

impl CowAnnotations {
    /// Create an empty annotation set.
    pub fn new() -> Self {
        Self {
            annotations: FxHashMap::default(),
        }
    }

    /// Set the COW mode for an instruction.
    pub fn set(&mut self, block_idx: usize, instr_idx: usize, mode: CowMode) {
        self.annotations.insert((block_idx, instr_idx), mode);
    }

    /// Get the COW mode for an instruction.
    ///
    /// Returns [`CowMode::Dynamic`] for instructions without explicit annotations.
    #[inline]
    pub fn get(&self, block_idx: usize, instr_idx: usize) -> CowMode {
        self.annotations
            .get(&(block_idx, instr_idx))
            .copied()
            .unwrap_or(CowMode::Dynamic)
    }

    /// Number of annotated instructions.
    pub fn len(&self) -> usize {
        self.annotations.len()
    }

    /// Whether there are no annotations.
    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }

    /// Count of instructions annotated as [`CowMode::StaticUnique`].
    pub fn static_unique_count(&self) -> usize {
        self.annotations
            .values()
            .filter(|&&m| m == CowMode::StaticUnique)
            .count()
    }

    /// Count of instructions annotated as [`CowMode::StaticShared`].
    pub fn static_shared_count(&self) -> usize {
        self.annotations
            .values()
            .filter(|&&m| m == CowMode::StaticShared)
            .count()
    }

    /// Iterate over all annotations.
    pub fn iter(&self) -> impl Iterator<Item = ((usize, usize), CowMode)> + '_ {
        self.annotations.iter().map(|(&k, &v)| (k, v))
    }
}

// Derived data — not part of structural identity.
impl PartialEq for CowAnnotations {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for CowAnnotations {}

impl Hash for CowAnnotations {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

/// Compute COW mode annotations for all COW operations in a function.
///
/// Walks each basic block using the converged uniqueness state from
/// [`intra::analyze_with_summaries`]. For each `Apply` instruction that
/// calls a COW method (identified by presence in `cow_method_names`),
/// records the receiver variable's uniqueness as a [`CowMode`].
///
/// # Arguments
///
/// * `func` — ARC IR function (pre-RC-insertion)
/// * `classifier` — type classification for RC tracking
/// * `liveness` — precomputed liveness (from [`compute_liveness`](crate::liveness::compute_liveness))
/// * `summaries` — interprocedural uniqueness summaries
/// * `cow_method_names` — set of method `Name`s that are COW operations
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn compute_cow_annotations(
    func: &crate::ir::ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    summaries: &FxHashMap<Name, UniquenessSummary>,
    cow_method_names: &rustc_hash::FxHashSet<Name>,
) -> CowAnnotations {
    let result = intra::analyze_with_summaries(func, classifier, liveness, summaries);
    let last_use_maps: Vec<intra::LastUseMap> = func
        .blocks
        .iter()
        .enumerate()
        .map(|(i, block)| intra::precompute_last_use(block, &liveness.live_out[i]))
        .collect();

    let mut annotations = CowAnnotations::new();
    let mut total_cow_ops = 0u32;
    let mut eliminated = 0u32;

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut state = result.block_in[block_idx].clone();

        for (instr_idx, instr) in block.body.iter().enumerate() {
            // Check if this is a COW method call BEFORE applying the transfer.
            // The receiver's uniqueness at this point determines whether the
            // runtime check can be eliminated.
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if cow_method_names.contains(callee) && !args.is_empty() {
                    total_cow_ops += 1;
                    let receiver = args[0];
                    let mode = state.cow_mode(receiver);
                    if mode != CowMode::Dynamic {
                        eliminated += 1;
                    }
                    annotations.set(block_idx, instr_idx, mode);
                }
            }

            // Advance state through this instruction.
            intra::transfer_instr(
                &mut state,
                instr,
                instr_idx,
                func,
                classifier,
                &last_use_maps[block_idx],
                summaries,
            );
        }
    }

    if total_cow_ops > 0 {
        tracing::debug!(
            function = func.name.raw(),
            total_cow_ops,
            eliminated,
            dynamic = total_cow_ops - eliminated,
            "COW annotation complete"
        );
    }

    annotations
}

#[cfg(test)]
mod tests;
