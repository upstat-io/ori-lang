//! State map data structure for intraprocedural analysis.
//!
//! [`AimsStateMap`] stores the computed [`AimsState`] for every variable at block
//! boundaries (entry and exit). Per-instruction states are NOT stored — they are
//! re-derived during emission by replaying transfer functions within each block.
//!
//! The state map is the **analysis fact source** consumed by:
//! RC emission, reuse emission, COW annotation, drop hints, and FIP certification.

mod aliases;
mod block_states;
mod facts;
#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "state-map tests abort when a required program point has no state"
)]
mod tests;
mod types;

pub use types::{AimsEvent, ApplyAliasSource, InvokeEdgeState};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcBlockId, ArcFunction, ArcVarId};

use super::super::contract::EffectSummary;
use super::super::lattice::{AimsState, BorrowSource, Locality, ShapeClass, Uniqueness};
use super::birth_site_partition::BirthSitePartition;
use super::project_aliases::ProjectSources;

// State map

/// Complete analysis result for a function.
///
/// Maps (block-boundary, variable) → [`AimsState`].
/// Computed by backward dataflow, consumed by emission passes.
/// Per-instruction states are NOT stored — they are re-derived by
/// emission passes via backward replay within each block.
///
/// Also contains:
/// - Borrow provenance side table (per-variable, not per-point)
/// - Per-invoke edge states (normal vs unwind demand)
/// - Sparse event table for special-interest program points
///
/// # Memory layout
///
/// Uses `Vec<FxHashMap<ArcVarId, AimsState>>` (sparse) rather than
/// `Vec<Vec<AimsState>>` (dense). In backward demand analysis, most
/// variables are `BOTTOM` (no demand) at most blocks — sparse storage
/// avoids allocating `num_vars` entries per block when only a fraction
/// have non-BOTTOM state. For functions with very few variables where
/// dense storage would win on cache locality, the hash map overhead is
/// negligible (small maps fit in a single cache line).
pub struct AimsStateMap {
    /// State at the END of each block (after terminator).
    /// Indexed by `ArcBlockId::index()`.
    block_exit_states: Vec<FxHashMap<ArcVarId, AimsState>>,

    /// State at the ENTRY of each block (before first instruction).
    /// Computed by applying transfer functions backward through the block's
    /// instructions, starting from the block's exit state.
    /// Indexed by `ArcBlockId::index()`.
    block_entry_states: Vec<FxHashMap<ArcVarId, AimsState>>,

    /// Per-invoke edge states. Stored sparsely — only for blocks
    /// ending in Invoke.
    invoke_edge_states: FxHashMap<ArcBlockId, InvokeEdgeState>,

    /// Borrow provenance side table.
    /// Sparse: contains only variables in `AccessClass::Borrowed`.
    /// Per-variable (not per-point) — see module doc for precision trade-off.
    borrow_sources: FxHashMap<ArcVarId, BorrowSource>,

    /// Apply-result allocation-identity side table (§1.9 third
    /// side-table). Sparse: only entries for Apply/Invoke destinations whose
    /// callee `MemoryContract` carries `return_alias != None` for one or
    /// more Owned params. Empty when no in-scope callee transfers ownership
    /// through return — zero per-instruction overhead.
    ///
    /// Populated PRE-WALK by `apply_aliases::populate_apply_result_aliases`
    /// using converged callee `MemoryContract`s; read-only during the
    /// backward worklist (PL-5 no-stale-summary invariant). Composed into
    /// `project_alias_sources` at construction (Step 1b) so transitivity
    /// Rules 2/3/4/6 propagate the alias through Let/Jump/CFG-merge/nested-
    /// Project chains without re-coding the worklist.
    apply_result_aliases: FxHashMap<ArcVarId, ApplyAliasSource>,

    /// Project-derived alias graph (Spec: Annex E §AIMS Side-Table
    /// Domains). Sparse: only entries for Project destinations and
    /// their transitive Let / Jump-arg / CFG-merge / Apply-aliased sources per
    /// `compute_project_alias_sources`. Empty for functions with no Project
    /// instructions.
    ///
    /// Populated PRE-WALK by [`compute_project_alias_sources`](super::project_aliases::compute_project_alias_sources)
    /// — the side table also stores a clone of the local worklist input so
    /// post-convergence consumers can query it after the lattice converges.
    /// Read-only thereafter (PL-5 no-stale-summary invariant).
    ///
    /// Consumed by the realize-walk redundant-dec cleanup
    /// (`realize/cleanup_redundant.rs`) to widen the chain-class set with
    /// Project-derived aliases of class members. Project apply-aliases live
    /// in a DIFFERENT class than their source (PIN-2 in
    /// `ssa_alias_classes.rs`), so `class_members(class_a)` cannot see
    /// them; this side-table bridges the gap WITHOUT unifying classes
    /// (preserves "different RC slot" architectural rule).
    project_alias_sources: FxHashMap<ArcVarId, ProjectSources>,

    /// SSA-alias equivalence-class table. Sparse: only entries for variables
    /// participating in a multi-member class (singletons excluded).
    ///
    /// Class membership encodes "these SSA names refer to the same RC slot"
    /// — Let-Var aliases (transitively chained), Jump-arg → block-param
    /// pairs, and apply-result aliases of `Direct` / `Conditional` shape.
    /// `Project` apply-result aliases and `Select` operands are EXCLUDED
    /// per PIN-2 ("different RC slot" architectural rule).
    ///
    /// Populated PRE-WALK by
    /// [`ssa_alias_classes::compute_ssa_alias_classes`](super::ssa_alias_classes::compute_ssa_alias_classes);
    /// read-only during the backward worklist.
    ssa_alias_classes: FxHashMap<ArcVarId, u32>,

    /// Reverse index: class id → set of member vars. Sparse; populated
    /// from the same union-find pass that fills `ssa_alias_classes`.
    ///
    /// Enables PIN-4 class-liveness: `walk_dec.rs::emit_last_use_decs` skips
    /// `RcDec` emission unless `class_members(class_id).any(is_live_after)`
    /// is false — "no class member live after this instruction" means the
    /// class has reached its absolute last use.
    class_members: FxHashMap<u32, FxHashSet<ArcVarId>>,

    /// PIN-3 directional metadata: class id → set of source-arg vars that
    /// are apply-alias sources for some apply-result destination. Keyed by
    /// the SOURCE's class (NOT the destination's class) — for `Direct` and
    /// `Conditional` shapes the two coincide via union; for `Project`
    /// shape (no union) source-class is the source arg's pre-existing
    /// class, and keying by source-class is the only way the
    /// `should_suppress_apply_aliased_dec` helper can find the apply
    /// source for a Project return.
    class_apply_alias_source_candidates: FxHashMap<u32, FxHashSet<ArcVarId>>,

    /// Sparse event table: special-interest program points, indexed by block.
    events: FxHashMap<ArcBlockId, Vec<AimsEvent>>,

    /// Variables permanently marked as SCALAR (excluded from the AIMS product
    /// lattice). Indexed by `ArcVarId::index()`.
    scalars: Vec<bool>,

    /// L-9-excluded scalar liveness at block boundaries. These sets carry only
    /// whether copied scalar bits are used later; they never install an
    /// `AimsState` or `RawDemand` for a scalar variable. The backward fixed
    /// point uses them solely to select TF-14's scalar Project contribution.
    scalar_live_at_exit: Vec<FxHashSet<ArcVarId>>,
    scalar_live_at_entry: Vec<FxHashSet<ArcVarId>>,

    /// Variables carrying the backend-neutral IMMORTAL lifetime fact.
    /// Excluded from RC emission, COW annotation, reuse detection, and drop hints.
    /// Unlike scalars (which have no logical allocation identity), immortals may
    /// carry a stable identity but require no ownership-count or cleanup events.
    /// Indexed by `ArcVarId::index()`. True = immortal.
    immortals: Vec<bool>,

    /// Per-`(variable, field-path)` birth-site same-allocation partition.
    /// Admission per the T1 partition calculus (`AimsProof.Partition`).
    ///
    /// `None` until [`set_birth_site_partition`](Self::set_birth_site_partition)
    /// installs it — the pipeline populates it once on the converged state
    /// map, after Step 4a and before Step 4b burden emission. Read-only
    /// thereafter (PL-5 no-stale-summary invariant).
    birth_site_partition: Option<BirthSitePartition>,

    /// Function-level effect summary, accumulated during analysis.
    ///
    /// Accumulated per-block in the convergence loop via `accumulate_effect()`.
    /// Records whether the function allocates, shares references, or throws.
    /// Read by `extract_contract()` to set `MemoryContract.effects`.
    ///
    /// `HeapEscaping` locality accumulates into `may_share`.
    effect_summary: EffectSummary,

    /// FIP token balance: number of `Construct` instructions with reusable
    /// constructor kinds (struct, enum variant) on non-scalar destinations.
    /// Populated post-convergence by `populate_fip_balance()`.
    fip_construct_count: u32,

    /// FIP token balance: number of consumed non-scalar function parameters
    /// (Dead or Unrestricted consumption at function entry). Each consumed
    /// parameter provides a "reuse token" — its memory can be recycled by a
    /// Construct. Shape compatibility checked at emission time.
    /// Populated post-convergence by `populate_fip_balance()`.
    fip_consumed_count: u32,

    /// Per-variable shape classification, derived from definition instructions.
    ///
    /// Shape is a property of how a variable was *produced* (its definition
    /// instruction), not of how it's demanded. The backward analysis doesn't
    /// propagate shape through use sites — only through definitions. This
    /// side table makes shape available at all program points via
    /// [`var_shape`](Self::var_shape).
    ///
    /// Populated post-convergence by `populate_var_shapes()` (for Construct/
    /// Reuse/CollectionReuse) and `populate_call_result_states()` (for
    /// Apply/Invoke results from contracts).
    var_shapes: FxHashMap<ArcVarId, ShapeClass>,

    /// Per-variable contract-narrowed return uniqueness for Apply/Invoke results.
    ///
    /// Populated by `populate_call_result_states` post-convergence pass from
    /// each direct call's `MemoryContract.return_info.uniqueness` (or
    /// `CONSERVATIVE.uniqueness = MaybeShared` when no contract is available,
    /// per spec TF-5/TF-5a/TF-6c).
    ///
    /// Sparse — BOTTOM-default filter: `Unique` is the lattice BOTTOM for
    /// uniqueness and is NOT stored. `MaybeShared` and `Shared` ARE stored
    /// (they narrow below CONSERVATIVE). This asymmetry vs `var_shapes` is
    /// load-bearing: BOTTOM ≠ CONSERVATIVE for uniqueness. Without storing
    /// `MaybeShared` the call-result side table would silently drop
    /// `ori_list_slice_drop`'s contract, leaving `drop_hints` to read lattice
    /// BOTTOM=Unique → `ori_buffer_drop_unique` → a slice-rest double-free panic.
    ///
    /// Side-table extension feeds the lattice via JOIN, never overrides it.
    var_uniqueness: FxHashMap<ArcVarId, Uniqueness>,

    /// Per-variable contract-narrowed return locality for Apply/Invoke results.
    ///
    /// Populated by `populate_call_result_states`. Sparse — BOTTOM-default
    /// filter: `BlockLocal` is the lattice BOTTOM for locality and is NOT
    /// stored. `FunctionLocal`, `HeapEscaping`, and `Unknown` ARE stored.
    var_locality: FxHashMap<ArcVarId, Locality>,

    /// Per-Invoke-block-and-dst demand captured BEFORE the normal-successor's
    /// strip removes the Invoke-defined dst from its entry state. Keyed by
    /// `(invoke_owner_block, invoke_dst)` — predecessor lookups for an
    /// Invoke-terminator dst find the captured demand in this table, even though the
    /// successor's `block_entry_states` excludes the variable.
    ///
    /// # Why this exists
    ///
    /// `compute_block_entry_state` strips Invoke-defined dsts from successor
    /// entry states (block.rs §"Invoke defs" comment) so demand doesn't
    /// propagate backward past the def point. But this also erases the
    /// legitimate predecessor-exit demand on the Invoke dst — predecessors
    /// querying `var_state_at_block_exit(invoke_block, dst)` would get BOTTOM,
    /// missing the post-def demand from the normal successor (e.g.,
    /// Return-widened `HeapEscaping` locality). This side table captures the
    /// demand AT the strip site so predecessor lookups can recover it.
    ///
    /// # Population
    ///
    /// Populated by `analyze_function` from `BlockAnalysisResult.invoke_def_demand`
    /// AFTER each `compute_block_entry_state` call, keyed by the predecessor
    /// Invoke block (looked up via the inverse map `invoke_dst_to_owner`).
    /// Cleared at the start of each iteration so the captured demand reflects
    /// the current iteration's converged successor entry states.
    ///
    /// # Consumer
    ///
    /// `var_state_at_block_exit` consults this table FIRST for any var, falling
    /// back to standard `block_exit_states` when no entry exists. Empty by
    /// default — non-Invoke vars never have entries.
    invoke_def_demand: FxHashMap<(ArcBlockId, ArcVarId), AimsState>,

    /// Converged BACKWARD-demand state at an intra-block instruction's
    /// definition point, keyed by `(defining_block, dst)`.
    ///
    /// A var defined AND consumed entirely within one block (e.g. a fresh
    /// `Construct` used once as a borrow operand) is stripped from the demand
    /// map by `apply_instr_forward_transfer` before block exit, so
    /// `block_exit_states` returns BOTTOM for it. This table captures the
    /// pre-strip demand (the `seqAdd`-accumulated cardinality + consumption —
    /// `Once`/`Linear` for a single-use value, `Many` for a multi-use one) at
    /// the strip site, mirroring `invoke_def_demand` for the intra-block
    /// instruction-definition case rather than the Invoke-terminator case.
    ///
    /// Populated by `analyze_function` from `BlockAnalysisResult.def_demand`
    /// AFTER each `compute_block_entry_state` call; cleared each iteration.
    ///
    /// # Consumer
    ///
    /// `var_state_at_definition` consults this table FIRST, so DP-3
    /// (`is_additional_credit_elidable`) / DP-2 receive the proven converged-at-definition
    /// demand (TF-11 `seqAdd` accumulation) rather than the BOTTOM block-exit
    /// state. `var_state_at_block_exit` does NOT consult it (no blast radius to
    /// FIP / `LocalAlloc` consumers).
    def_demand: FxHashMap<(ArcBlockId, ArcVarId), AimsState>,

    /// Tracks whether any state changed in the last iteration.
    /// Reset to `false` at the start of each iteration; set to `true`
    /// by `update_block_entry` when a state changes.
    changed: bool,

    /// Whether any `canonicalize()` call during analysis required multiple
    /// rounds (cross-dimension chaining detected). Set by post-convergence
    /// verification in `verify_canonical_fixed_point()`. With current rules
    /// this should always be `false`.
    cross_dimension_detected: bool,
}

impl AimsStateMap {
    /// Create a new state map for the given function.
    ///
    /// All variables start at `BOTTOM` (most optimistic). Scalar variables
    /// must be marked separately via [`set_permanent_scalar`](Self::set_permanent_scalar).
    pub(crate) fn new(func: &ArcFunction) -> Self {
        let num_blocks = func.blocks.len();
        let num_vars = func.var_types.len();
        Self {
            block_exit_states: vec![FxHashMap::default(); num_blocks],
            block_entry_states: vec![FxHashMap::default(); num_blocks],
            invoke_edge_states: FxHashMap::default(),
            borrow_sources: FxHashMap::default(),
            apply_result_aliases: FxHashMap::default(),
            project_alias_sources: FxHashMap::default(),
            ssa_alias_classes: FxHashMap::default(),
            class_members: FxHashMap::default(),
            class_apply_alias_source_candidates: FxHashMap::default(),
            events: FxHashMap::default(),
            scalars: vec![false; num_vars],
            scalar_live_at_exit: vec![FxHashSet::default(); num_blocks],
            scalar_live_at_entry: vec![FxHashSet::default(); num_blocks],
            immortals: vec![false; num_vars],
            birth_site_partition: None,
            effect_summary: EffectSummary::default(),
            fip_construct_count: 0,
            fip_consumed_count: 0,
            var_shapes: FxHashMap::default(),
            var_uniqueness: FxHashMap::default(),
            var_locality: FxHashMap::default(),
            invoke_def_demand: FxHashMap::default(),
            def_demand: FxHashMap::default(),
            changed: false,
            cross_dimension_detected: false,
        }
    }

    // Scalar management

    /// Mark a variable as permanently SCALAR (excluded from analysis).
    ///
    /// Scalar variables require no ownership events, sharing observations, or
    /// reuse facts.
    /// This is irreversible — once marked, the variable returns `SCALAR`
    /// from all state queries.
    pub fn set_permanent_scalar(&mut self, var: ArcVarId) {
        if let Some(entry) = self.scalars.get_mut(var.index()) {
            *entry = true;
        }
    }

    /// Whether a variable is permanently SCALAR.
    #[inline]
    pub fn is_scalar(&self, var: ArcVarId) -> bool {
        self.scalars.get(var.index()).copied().unwrap_or(false)
    }

    // Immortal management

    /// Set the immortal bitvector from pre-computed detection results.
    ///
    /// Called after [`detect_immortals`](crate::aims::immortal::detect_immortals)
    /// in the pipeline, before analysis begins. Immortal variables are excluded
    /// from analysis (same treatment as scalars) and from all emission phases.
    pub fn set_immortals(&mut self, immortals: Vec<bool>) {
        self.immortals = immortals;
    }

    /// Whether a variable carries the IMMORTAL logical lifetime fact and is
    /// therefore excluded from ownership-count operations.
    #[inline]
    pub fn is_immortal(&self, var: ArcVarId) -> bool {
        self.immortals.get(var.index()).copied().unwrap_or(false)
    }

    /// Whether a variable should be excluded from analysis and emission
    /// (either SCALAR or IMMORTAL).
    #[inline]
    pub fn is_excluded(&self, var: ArcVarId) -> bool {
        self.is_scalar(var) || self.is_immortal(var)
    }

    // Birth-site partition management

    /// Install the birth-site partition side table.
    ///
    /// Called once by the pipeline on the converged state map, after Step 4a
    /// and before Step 4b burden emission. Read-only thereafter (PL-5).
    pub(crate) fn set_birth_site_partition(&mut self, partition: BirthSitePartition) {
        self.birth_site_partition = Some(partition);
    }

    /// Read the frozen same-allocation partition installed after convergence.
    ///
    /// Post-emission consumers use this to extend physical-owner facts across
    /// Construct-field / Project identities without rebuilding an independent
    /// alias relation after the class-ledger plan has been materialized.
    pub(crate) fn birth_site_partition(&self) -> Option<&BirthSitePartition> {
        self.birth_site_partition.as_ref()
    }
}
