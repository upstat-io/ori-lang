//! State map data structure for intraprocedural analysis.
//!
//! [`AimsStateMap`] stores the computed [`AimsState`] for every variable at block
//! boundaries (entry and exit). Per-instruction states are NOT stored — they are
//! re-derived during emission by replaying transfer functions within each block.
//!
//! The state map is the **analysis fact source** for all downstream consumers:
//! RC emission, reuse emission, COW annotation, drop hints, and FIP certification.

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use expect for clearer failure messages"
)]
mod tests;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcBlockId, ArcFunction, ArcVarId};

use super::super::contract::EffectSummary;
use super::super::lattice::{AimsState, BorrowSource, Locality, ShapeClass, Uniqueness};
use super::project_aliases::ProjectSources;

// Per-invoke edge state

/// Per-edge demand state for Invoke terminators.
///
/// Normal and unwind are alternative paths (only one executes),
/// but they carry different variable sets: `dst` is defined only
/// on the normal path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvokeEdgeState {
    /// Demand flowing to the normal successor (`dst` IS defined here).
    pub normal: FxHashMap<ArcVarId, AimsState>,
    /// Demand flowing to the unwind successor (`dst` is NOT defined here).
    /// Used by RC emission to determine cleanup `RcDec` operations on
    /// the exception path.
    pub unwind: FxHashMap<ArcVarId, AimsState>,
}

// Sparse event table

/// A special-interest event detected during analysis.
///
/// Stored sparsely per block — most blocks have no events. These track
/// program points that don't fit the per-variable-per-block model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AimsEvent {
    /// A TRMC constructor-context region opens at this point.
    /// (Stage 3 — populated by normalize/ pass, consumed by reuse emission)
    ContextOpen {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A TRMC constructor-context region closes at this point.
    ContextClose {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A candidate reusable allocation site.
    ReusableAllocation {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A local-allocation eligibility point (`Locality::FunctionLocal`
    /// or `BlockLocal` for a non-scalar allocation).
    LocalAllocCandidate {
        block: ArcBlockId,
        instr: usize,
        var: ArcVarId,
    },
    /// A FIP gate: a call site where the callee's `FipContract` is
    /// `Conditional`, and the preconditions are satisfied at this point.
    FipGate { block: ArcBlockId, instr: usize },
    /// Per-branch allocation credit balance at a Switch terminator's successor.
    ///
    /// Records the allocation balance (constructs - consumed deaths) for each
    /// successor of a Switch terminator. FIP certification requires each branch
    /// to independently maintain non-negative credit balance (`FIPTree` DMATCH! rule).
    /// Effect Activation.
    AllocCreditBalance {
        /// The Switch terminator's block.
        block: ArcBlockId,
        /// Index of the successor in the Switch's targets.
        successor_idx: usize,
        /// Balance: positive = more allocs than deaths (needs tokens),
        /// zero = balanced, negative = surplus deaths (provides tokens).
        balance: i32,
    },
}

impl AimsEvent {
    /// The block this event belongs to.
    fn block(&self) -> ArcBlockId {
        match self {
            Self::ContextOpen { block, .. }
            | Self::ContextClose { block, .. }
            | Self::ReusableAllocation { block, .. }
            | Self::LocalAllocCandidate { block, .. }
            | Self::FipGate { block, .. }
            | Self::AllocCreditBalance { block, .. } => *block,
        }
    }
}

// Apply-result alias side-table

/// Caller-side allocation-identity record for an Apply/Invoke destination.
///
/// When a callee's `MemoryContract` carries `ParamContract::return_alias =
/// Some(ReturnAliasShape::*)` for one or more params, the destination of an
/// Apply/Invoke at the caller IS the same heap allocation as the consumed
/// argument(s) — the callee transferred ownership through return. This
/// side-table records that identity so the caller's RC emission can avoid
/// double-decrementing the shared allocation (BUG-04-090).
///
/// Distinct from `borrow_sources` (Project-level borrow facts) and from
/// `project_alias_sources` (transitive Let/Jump alias chains) — see §1.9
/// Side-Table Domains. The three side-tables are composed at
/// `compute_project_alias_sources` so a single backward-walk query reaches
/// the canonical RC owner regardless of which mechanism produced the alias.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyAliasSource {
    /// Apply dst aliases the consumed arg directly (callee returns the
    /// param unchanged: `@id<T>(x: T) -> T = x`). At the caller,
    /// `Apply dst = @callee(arg)` makes `dst` the same allocation as `arg`.
    Direct(ArcVarId),
    /// Apply dst aliases a single-field projection of the consumed arg
    /// (callee returns `arg.field`: `@unwrap<T>(b: Box<T>) -> T = b.inner`).
    /// Single-field only — matches `ReturnAliasShape::Project { field: u32 }`
    /// and `BorrowSource::Exact { field: Option<u32> }` precedent. Nested
    /// projections defer to a future extension.
    Project { arg: ArcVarId, field: u32 },
    /// Apply dst structurally CONTAINS the consumed arg as a transitive-drop
    /// variant payload (callee constructs a wrapping variant: `@wrap_ok(m: T)
    /// -> Result<T, E> = Ok(m)`). Distinct from `Direct`/`Project`: the
    /// dst is a SEPARATE allocation (the constructed wrapper), and arg lives
    /// inside dst's payload via the structural drop walk. PIN-2 ANALOGOUS:
    /// `Wrapped` does NOT trigger `uf.union` in `compute_ssa_alias_classes`
    /// (the wrapper and the wrapped allocation are DIFFERENT RC slots), and
    /// does NOT seed `project_alias_sources` Step 1b in
    /// `compute_project_alias_sources` (containment is NOT a projection-
    /// derived alias chain). The sole consumer is `should_suppress_apply
    /// _aliased_dec`: suppresses the redundant
    /// caller-side canonical dec on `arg` because `arg`'s ownership was
    /// transferred into dst's payload via the wrapping construct. Without
    /// this suppression, both arg's caller-side dec AND dst's structural
    /// drop walk dec the same allocation → double-free.
    Wrapped(ArcVarId),
    /// Apply dst aliases ONE OF the candidates at runtime — the exact alias
    /// is path-conditional. Arises when 2+ params of the callee have
    /// `return_alias != None` (e.g., `match x { A -> a, B -> b }` in the
    /// callee). Caller's RC emission suppresses scope-exit decs on EVERY
    /// candidate (their ownership transferred at the Apply site, regardless
    /// of which path is taken at runtime); dst's own RC ops are retained as
    /// the canonical owner of the returned allocation.
    Conditional { candidates: Vec<ArcVarId> },
}

/// Same-class dec obligation entry.
///
/// Per `(block, class_id)`, records ordered intra-block dec obligation
/// points for class members + the set of class members live at block exit.
/// Consumed by `walk_dec.rs::class_alive_after` for path-sensitive
/// same-slot dec dedup across `Let{Var}` / `Jump` arg / `Conditional`
/// alias chains.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassObligationEntry {
    /// Ordered `(var, instr_idx)` tuples for class members whose last-use
    /// happens in this block. Sorted by `instr_idx` ascending. The
    /// `instr_idx == block.body.len()` value indicates last-use is in
    /// the terminator. Empty when no class member dies within this block.
    pub(crate) intra_block_obligations: Vec<(ArcVarId, usize)>,

    /// Set of class members live at block exit (continuing into successor
    /// blocks). Non-empty when class has cross-block lifetime; the
    /// successor block's obligation entry will fire the canonical class
    /// dec.
    ///
    /// Currently populated but NOT consulted by `class_alive_after` — the
    /// existing `pin4_class_emits_dec_set` + canonical-rep selection
    /// already coordinates cross-block decs. Retained for future
    /// refinement (e.g., scheduling when intra-class spans many blocks).
    #[allow(
        dead_code,
        reason = "populated for future cross-block obligation refinement; not yet consulted by class_alive_after"
    )]
    pub(crate) block_exit_members: FxHashSet<ArcVarId>,
}

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

    /// Borrow provenance side table (solutions.md Decision 1/5).
    /// Sparse: only entries for variables currently in `AccessClass::Borrowed`.
    /// Per-variable (not per-point) — see module doc for precision trade-off.
    borrow_sources: FxHashMap<ArcVarId, BorrowSource>,

    /// Apply-result allocation-identity side table (BUG-04-090; §1.9 third
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
    /// — a clone of the local worklist input is also installed here so the
    /// post-convergence pass `populate_class_payload_of_with_liveness` can
    /// query it after the lattice converges. Read-only thereafter (PL-5
    /// no-stale-summary invariant).
    ///
    /// Consumed by `class_lifetime_extends_past_path_sensitive` to widen the
    /// A-live witness set with Project-derived aliases of B-members. Project
    /// apply-aliases live in a DIFFERENT class than their source (PIN-2 at
    /// `ssa_alias_classes.rs:131`), so `class_members(class_a)` cannot see
    /// them; this side-table bridges the gap WITHOUT unifying classes
    /// (preserves "different RC slot" architectural rule).
    project_alias_sources: FxHashMap<ArcVarId, ProjectSources>,

    /// Dsts whose `project_alias_sources` entry is an R4 CFG-merge / R5 Select
    /// OVER-APPROXIMATION (sources span ≥2 distinct genuine same-allocation
    /// reps). Demand-side consumers (`propagate_project_source_demand`, the
    /// post-convergence witness extension) EXCLUDE these — the alias denotes a
    /// different allocation per CFG path, so keeping all spanned parents alive
    /// over-extends every path's lifetime. Populated alongside
    /// `project_alias_sources`; read-only thereafter (PL-5). Spec: Annex E
    /// §AIMS — `project_alias_sources` is the alias closure, not same-alloc-
    /// everywhere.
    alias_over_approximation_dsts: FxHashSet<ArcVarId>,

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
    /// class, and keying by source-class is the only way the downstream
    /// `should_suppress_apply_aliased_dec` helper can find the apply
    /// source for a Project return.
    class_apply_alias_source_candidates: FxHashMap<u32, FxHashSet<ArcVarId>>,

    /// PIN-6 inter-class payload-of relation (): class A id →
    /// set of class B ids whose drop transitively covers class A's RC slot
    /// via a transitive-drop `RcStrategy` (`Closure`, `AggregateFields`,
    /// `InlineEnum`, `HeapPointer`).
    ///
    /// Populated PRE-WALK by [`ssa_alias_classes::compute_ssa_alias_classes`]
    /// during the same union-find pass that builds `class_table` /
    /// `class_members` / `class_apply_alias_source_candidates`. An entry
    /// `(A → {B})` means: at some `Construct`/`Apply`/`Invoke`/`PartialApply`/
    /// `Set` instruction, a class-A member was `[own]`-consumed to construct
    /// or fill class B's payload, and class B's `RcStrategy` is in the
    /// transitive-drop set per [`is_transitive_drop_strategy`].
    ///
    /// Consumed by `walk_dec.rs::pin6_any_ancestor_will_cover` (body),
    /// `edge_cleanup.rs::pin6_any_ancestor_will_cover_edge`, and
    /// `dead_cleanup::pin6_any_ancestor_will_cover_entry` to suppress class
    /// A's canonical dec when class B's drop will cover. Self-loop entries
    /// (A → A from Direct apply-aliases that union into one class) are
    /// excluded at population time — those reduce to PIN-4 class-liveness
    /// and need no PIN-6 suppression.
    ///
    /// Singleton-class invariant: every class id appearing as a parent (set
    /// value) MUST have a matching entry in `class_members` —
    /// the population pass eagerly inserts singleton members so the BFS
    /// predicate's `class_members(parent_class)` lookup succeeds.
    ///
    /// [`ssa_alias_classes::compute_ssa_alias_classes`]: super::ssa_alias_classes::compute_ssa_alias_classes
    /// [`is_transitive_drop_strategy`]: crate::ir::is_transitive_drop_strategy
    class_payload_of: FxHashMap<u32, FxHashSet<u32>>,

    /// Same-class dec obligation table.
    ///
    /// Per `(block, class_id)`, the ordered intra-block dec obligation
    /// points + block-exit members. Consumed by
    /// `walk_dec.rs::class_alive_after` for path-sensitive same-slot dec
    /// dedup. Populated POST-CONVERGENCE by
    /// [`populate_class_dec_obligations`](super::post_convergence::populate_class_dec_obligations);
    /// read-only thereafter (PL-5 no-stale-summary invariant). AIMS
    /// Invariant #5(c) — typed pre-pass input on `AimsStateMap`.
    ///
    /// Sparse: only entries for `(block, class_id)` pairs where ≥1 class
    /// member has a last-use in the block OR ≥1 class member is live at
    /// block exit.
    class_dec_obligations: FxHashMap<(ArcBlockId, u32), ClassObligationEntry>,

    /// Sparse event table: special-interest program points, indexed by block.
    events: FxHashMap<ArcBlockId, Vec<AimsEvent>>,

    /// Variables permanently marked as SCALAR (excluded from analysis).
    /// Indexed by `ArcVarId::index()`. True = scalar (never analyzed).
    scalars: Vec<bool>,

    /// Variables marked as IMMORTAL (heap-allocated but with `MAX_REFCOUNT`).
    /// Excluded from RC emission, COW annotation, reuse detection, and drop hints.
    /// Unlike scalars (which have no heap allocation), immortals DO allocate
    /// but use pre-allocated singletons that never need RC operations.
    /// Indexed by `ArcVarId::index()`. True = immortal.
    immortals: Vec<bool>,

    /// Function-level effect summary, accumulated during analysis.
    ///
    /// Accumulated per-block in the convergence loop via `accumulate_effect()`.
    /// Records whether the function allocates, shares references, or throws.
    /// Read by `extract_contract()` to set `MemoryContract.effects`.
    ///
    /// `HeapEscaping` → `may_share` and Effect Activation.
    effect_summary: EffectSummary,

    /// FIP token balance: number of `Construct` instructions with reusable
    /// constructor kinds (struct, enum variant) on non-scalar destinations.
    /// Populated post-convergence by `populate_fip_balance()`.
    /// Effect Activation.
    fip_construct_count: u32,

    /// FIP token balance: number of consumed non-scalar function parameters
    /// (Dead or Unrestricted consumption at function entry). Each consumed
    /// parameter provides a "reuse token" — its memory can be recycled by a
    /// Construct. Shape compatibility checked at emission time.
    /// Populated post-convergence by `populate_fip_balance()`.
    /// Effect Activation.
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
    /// Shape Activation.
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
    /// BOTTOM=Unique → `ori_buffer_drop_unique` → BUG-04-086 panic.
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
    /// Invoke-terminator dst find the captured demand here, even though the
    /// successor's `block_entry_states` no longer carries the var.
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
    /// default — non-Invoke vars never have entries here.
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
    /// (`is_rc_inc_elidable`) / DP-2 receive the proven converged-at-definition
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
    ///
    /// Convergence Feedback.
    cross_dimension_detected: bool,

    /// Set of SSA-alias class ids fully covered by the burden walk
    /// (`emit_burden_ops`).
    ///
    /// `class_covered[C] = true` iff:
    /// 1. EVERY var `v` in `class_members(C)` satisfies
    ///    `func.burden_emitted[v.index()] = true`, AND
    /// 2. EVERY payload class `P` transitively reachable from `C` via
    ///    `class_payload_of` is also in `class_covered`.
    ///
    /// Populated POST-CONVERGENCE by
    /// [`populate_class_covered`](super::post_convergence::populate_class_covered)
    /// via fixed-point iteration on the finite class set (terminates per AIMS
    /// L-5). Sparse — only ids of covered classes are stored; absence ⇒ not
    /// covered.
    ///
    /// Consumed by `decide()` (`aims/realize/decide.rs`) — when a target var's
    /// class is in this set, `RcDecision` is forced to `None` (the burden walk
    /// owns the inc/dec; the predicate stack defers) per the coexistence
    /// handshake.
    ///
    /// AIMS Invariant #5(c) — typed pre-pass input on `AimsStateMap`, derived
    /// from `class_members` + `class_payload_of` + `func.burden_emitted`.
    class_covered: FxHashSet<u32>,
}

impl AimsStateMap {
    /// Create a new state map for the given function.
    ///
    /// All variables start at `BOTTOM` (most optimistic). Scalar variables
    /// must be marked separately via [`set_permanent_scalar`](Self::set_permanent_scalar).
    pub fn new(func: &ArcFunction) -> Self {
        let num_blocks = func.blocks.len();
        let num_vars = func.var_types.len();
        Self {
            block_exit_states: vec![FxHashMap::default(); num_blocks],
            block_entry_states: vec![FxHashMap::default(); num_blocks],
            invoke_edge_states: FxHashMap::default(),
            borrow_sources: FxHashMap::default(),
            apply_result_aliases: FxHashMap::default(),
            project_alias_sources: FxHashMap::default(),
            alias_over_approximation_dsts: FxHashSet::default(),
            ssa_alias_classes: FxHashMap::default(),
            class_members: FxHashMap::default(),
            class_apply_alias_source_candidates: FxHashMap::default(),
            class_payload_of: FxHashMap::default(),
            class_dec_obligations: FxHashMap::default(),
            events: FxHashMap::default(),
            scalars: vec![false; num_vars],
            immortals: vec![false; num_vars],
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
            class_covered: FxHashSet::default(),
        }
    }

    // Scalar management

    /// Mark a variable as permanently SCALAR (excluded from analysis).
    ///
    /// Scalar variables never need RC operations, COW checks, or reuse.
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

    /// Whether a variable is marked IMMORTAL (heap-allocated constant with
    /// `MAX_REFCOUNT`, excluded from RC operations).
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

    // Block state accessors

    /// Get the state of a variable at a block's exit (after terminator).
    ///
    /// Returns `SCALAR` for scalar variables, `BOTTOM` for variables
    /// not present in the state map (no demand from successors).
    ///
    /// # Invoke-terminator dsts
    ///
    /// For variables defined by an `ArcTerminator::Invoke` in `block`,
    /// `block_exit_states[block][var]` is BOTTOM because the normal
    /// successor's strip in `compute_block_entry_state` erases the var
    /// from its entry state before the predecessor's exit JOIN reads
    /// it. The `invoke_def_demand` side table captures the pre-strip
    /// demand and is consulted FIRST for any `(block, var)` pair,
    /// recovering the post-def demand (e.g., Return-widened
    /// `HeapEscaping` locality from a successor that returns the dst).
    /// Non-Invoke vars never have entries in `invoke_def_demand`, so
    /// the fallthrough to standard `block_exit_states` covers all
    /// other queries.
    #[must_use]
    pub fn var_state_at_block_exit(&self, block: ArcBlockId, var: ArcVarId) -> AimsState {
        if self.is_scalar(var) || self.is_immortal(var) {
            return AimsState::SCALAR;
        }
        if let Some(&state) = self.invoke_def_demand.get(&(block, var)) {
            return state;
        }
        self.block_exit_states
            .get(block.index())
            .and_then(|states| states.get(&var))
            .copied()
            .unwrap_or(AimsState::BOTTOM)
    }

    /// Get the converged BACKWARD-demand state at a variable's DEFINITION.
    ///
    /// Consults `def_demand` (intra-block instruction definitions) FIRST, then
    /// `invoke_def_demand` (Invoke-terminator definitions), then falls back to
    /// `block_exit_states`. Unlike `var_state_at_block_exit`, this recovers the
    /// pre-strip demand for a var defined+consumed within one block (where
    /// block-exit returns BOTTOM), giving DP-3 / DP-2 the proven TF-11
    /// `seqAdd`-accumulated cardinality (`Once` single-use, `Many` multi-use).
    #[must_use]
    pub fn var_state_at_definition(&self, block: ArcBlockId, var: ArcVarId) -> AimsState {
        if self.is_scalar(var) || self.is_immortal(var) {
            return AimsState::SCALAR;
        }
        if let Some(&state) = self.def_demand.get(&(block, var)) {
            return state;
        }
        if let Some(&state) = self.invoke_def_demand.get(&(block, var)) {
            return state;
        }
        self.block_exit_states
            .get(block.index())
            .and_then(|states| states.get(&var))
            .copied()
            .unwrap_or(AimsState::BOTTOM)
    }

    /// Record the converged pre-strip demand for an intra-block
    /// instruction-defined dst.
    ///
    /// Called by `analyze_function` after `compute_block_entry_state` returns
    /// the captured demand for the block's stripped instruction-defined vars.
    /// Keyed by `(defining_block, dst)`. See `def_demand` field doc.
    pub(crate) fn set_def_demand(&mut self, block: ArcBlockId, var: ArcVarId, state: AimsState) {
        self.def_demand.insert((block, var), state);
    }

    /// Record the pre-strip demand for an Invoke-terminator-defined dst.
    ///
    /// Called by `analyze_function` after `compute_block_entry_state` returns
    /// the captured demand for the normal successor's stripped vars. Keyed by
    /// `(invoke_owner_block, invoke_dst)` — the owner block is the predecessor
    /// whose terminator is the Invoke that defines `var`.
    ///
    /// See `invoke_def_demand` field doc for the full mechanism.
    pub(crate) fn set_invoke_def_demand(
        &mut self,
        block: ArcBlockId,
        var: ArcVarId,
        state: AimsState,
    ) {
        self.invoke_def_demand.insert((block, var), state);
    }

    /// Clear the invoke-def demand side table.
    ///
    /// Called at the start of each `analyze_function` iteration so the
    /// captured demand reflects the current iteration's successor entry
    /// states (which converge across iterations).
    pub(crate) fn clear_invoke_def_demand(&mut self) {
        self.invoke_def_demand.clear();
        self.def_demand.clear();
    }

    /// Get the state of a variable at a block's entry (before first instruction).
    ///
    /// Returns `SCALAR` for scalar variables, `BOTTOM` for variables
    /// not present in the state map.
    #[must_use]
    pub fn var_state_at_block_entry(&self, block: ArcBlockId, var: ArcVarId) -> AimsState {
        if self.is_scalar(var) || self.is_immortal(var) {
            return AimsState::SCALAR;
        }
        self.block_entry_states
            .get(block.index())
            .and_then(|states| states.get(&var))
            .copied()
            .unwrap_or(AimsState::BOTTOM)
    }

    /// Get the full entry state map for a block (all variables with non-BOTTOM state).
    ///
    /// Returns `None` for out-of-bounds block indices.
    #[must_use]
    pub fn block_entry_states(&self, block: ArcBlockId) -> Option<&FxHashMap<ArcVarId, AimsState>> {
        self.block_entry_states.get(block.index())
    }

    /// Get the full exit state map for a block (all variables with non-BOTTOM state).
    ///
    /// Returns `None` for out-of-bounds block indices.
    #[must_use]
    pub fn block_exit_states(&self, block: ArcBlockId) -> Option<&FxHashMap<ArcVarId, AimsState>> {
        self.block_exit_states.get(block.index())
    }

    // Block state mutation

    /// Update the entry state for a block. Returns `true` if any state changed.
    ///
    /// Called by the worklist loop: if this returns `true`, predecessors
    /// need to be re-analyzed.
    pub fn update_block_entry(
        &mut self,
        block: ArcBlockId,
        new_entry: FxHashMap<ArcVarId, AimsState>,
    ) -> bool {
        let idx = block.index();
        if idx >= self.block_entry_states.len() {
            return false;
        }
        let current = &self.block_entry_states[idx];
        if *current == new_entry {
            return false;
        }
        self.block_entry_states[idx] = new_entry;
        self.changed = true;
        true
    }

    /// Update the exit state for a block. Returns `true` if any state changed.
    pub fn update_block_exit(
        &mut self,
        block: ArcBlockId,
        new_exit: FxHashMap<ArcVarId, AimsState>,
    ) -> bool {
        let idx = block.index();
        if idx >= self.block_exit_states.len() {
            return false;
        }
        let current = &self.block_exit_states[idx];
        if *current == new_exit {
            return false;
        }
        self.block_exit_states[idx] = new_exit;
        self.changed = true;
        true
    }

    // Convergence tracking

    /// Whether the analysis has converged (no state changed in last iteration).
    #[must_use]
    pub fn is_converged(&self) -> bool {
        !self.changed
    }

    /// Reset the change tracker for a new iteration.
    pub fn reset_changed(&mut self) {
        self.changed = false;
    }

    /// Whether cross-dimension canonicalize chaining was detected during
    /// analysis (any canonicalize call required more than one round).
    ///
    /// With current rules, this should always be `false`.
    /// A `true` value indicates a new rule created a cross-dimension chain.
    ///
    /// Convergence Feedback.
    #[must_use]
    pub fn cross_dimension_detected(&self) -> bool {
        self.cross_dimension_detected
    }

    /// Record that cross-dimension chaining was detected.
    pub fn set_cross_dimension_detected(&mut self) {
        self.cross_dimension_detected = true;
    }

    /// Count cross-dimension canonicalize rule effects on converged states.
    ///
    /// Examines all block exit states and counts variable-block pairs where
    /// the converged state shows evidence of cross-dimensional rule effects:
    /// - Cross-dim: `BlockLocal + Owned + ≤Once + Unique` (from FRESH/transfer)
    /// - Rule 6: `HeapEscaping/Unknown + MaybeShared` where Unique was demoted
    /// - Rule 8: `Borrowed + ≤FunctionLocal` where locality was capped
    ///
    /// Returns total count of cross-dim influenced variable-block pairs.
    #[must_use]
    pub fn count_cross_dim_states(&self) -> usize {
        use super::super::lattice::{AccessClass, Cardinality, Locality, Uniqueness};

        let mut count = 0;
        for exit_map in &self.block_exit_states {
            for state in exit_map.values() {
                if state.is_scalar() {
                    continue;
                }
                // Cross-dim evidence: state has Unique + BlockLocal + Owned + ≤Once.
                // Reachable from FRESH allocation or transfer functions.
                if state.uniqueness == Uniqueness::Unique
                    && state.locality == Locality::BlockLocal
                    && state.access == AccessClass::Owned
                    && state.cardinality <= Cardinality::Once
                {
                    count += 1;
                    continue;
                }
                // Rule 8 evidence: Borrowed + ≤FunctionLocal.
                // The locality cap is from cross-dim reasoning.
                if state.access == AccessClass::Borrowed
                    && state.locality <= Locality::FunctionLocal
                    && state.locality != Locality::BlockLocal
                {
                    count += 1;
                }
            }
        }
        count
    }

    // Borrow provenance

    /// Get the borrow provenance for a variable.
    ///
    /// Returns `None` if the variable is Owned or not tracked.
    #[must_use]
    pub fn borrow_source(&self, var: ArcVarId) -> Option<&BorrowSource> {
        self.borrow_sources.get(&var)
    }

    /// Update borrow provenance during transfer function application.
    ///
    /// Called by `Project` and pattern binding transfers.
    pub fn set_borrow_source(&mut self, var: ArcVarId, source: BorrowSource) {
        self.borrow_sources.insert(var, source);
    }

    /// Remove provenance when a variable transitions to `AccessClass::Owned`.
    pub fn clear_borrow_source(&mut self, var: ArcVarId) {
        self.borrow_sources.remove(&var);
    }

    /// Find all borrows from a given source variable.
    ///
    /// Returns an iterator of `(borrow_var, field)` pairs, where `field` is
    /// `Some(idx)` for field-level borrows (from `Project`) and `None` for
    /// whole-object borrows. Used by the disjoint-field COW optimization
    /// to check whether a mutation conflicts with live borrows.
    pub fn borrows_from_source(
        &self,
        source: ArcVarId,
    ) -> impl Iterator<Item = (ArcVarId, Option<u32>)> + '_ {
        self.borrow_sources.iter().filter_map(move |(var, bs)| {
            if let BorrowSource::Exact { source: src, field } = bs {
                if *src == source {
                    return Some((*var, *field));
                }
            }
            None
        })
    }

    /// Merge provenance at control flow join points.
    ///
    /// Same source → keep `Exact`; different sources → `Unknown`.
    pub fn join_borrow_sources(&mut self, var: ArcVarId, other: BorrowSource) {
        match self.borrow_sources.get(&var) {
            Some(existing) => {
                let joined = existing.join(other);
                self.borrow_sources.insert(var, joined);
            }
            None => {
                self.borrow_sources.insert(var, other);
            }
        }
    }

    // Apply-result allocation-identity provenance (BUG-04-090)

    /// Look up the Apply-result allocation-identity record for a variable.
    ///
    /// Returns `Some(_)` only when `var` is an Apply/Invoke destination AND
    /// the callee's `MemoryContract` carried `ParamContract::return_alias !=
    /// None` for one or more Owned params at the time
    /// `populate_apply_result_aliases` ran. Returns `None` for fresh
    /// allocations, indirect calls, and Apply/Invoke destinations whose
    /// callees do not transfer ownership through return.
    #[must_use]
    pub fn apply_result_alias(&self, var: ArcVarId) -> Option<&ApplyAliasSource> {
        self.apply_result_aliases.get(&var)
    }

    /// Read-only borrow of the entire Apply-result alias map.
    ///
    /// Consumed by `compute_project_alias_sources` Step 1b (composition with
    /// the project alias graph) and by File 13's forward-walk
    /// `is_ownership_transfer` / `is_owned_call_position` classification.
    #[must_use]
    pub fn apply_result_aliases(&self) -> &FxHashMap<ArcVarId, ApplyAliasSource> {
        &self.apply_result_aliases
    }

    /// Bulk-install the pre-computed Apply-result alias map.
    ///
    /// Called once per function during `analyze_function`'s pre-walk setup,
    /// BEFORE `compute_project_alias_sources` and BEFORE the worklist loop.
    /// The map is read-only after this point per PL-5 (no-stale-summary
    /// invariant).
    pub fn set_apply_result_aliases(&mut self, aliases: FxHashMap<ArcVarId, ApplyAliasSource>) {
        self.apply_result_aliases = aliases;
    }

    // Project-derived alias graph

    /// Read-only borrow of the entire Project-derived alias source map.
    ///
    /// Consumed by `class_lifetime_extends_past_path_sensitive` (in the
    /// post-convergence pass) to widen the A-live witness set with Project-
    /// derived aliases of B-members; the predicate iterates this map's
    /// entries and treats any var whose `ProjectSources` intersect any
    /// B-member as an extended A-witness.
    #[must_use]
    pub(crate) fn project_alias_sources(&self) -> &FxHashMap<ArcVarId, ProjectSources> {
        &self.project_alias_sources
    }

    /// Bulk-install the pre-computed Project-derived alias source map.
    ///
    /// Called once per function during `analyze_function`'s pre-walk setup,
    /// AFTER `compute_project_alias_sources` runs. The local map is also
    /// kept by `analyze_function` for `propagate_project_source_demand`
    /// during the worklist; this setter persists a clone on the state map
    /// so the post-convergence pass can consume it after lattice
    /// convergence. Read-only after this point per PL-5.
    pub(crate) fn set_project_alias_sources(
        &mut self,
        sources: FxHashMap<ArcVarId, ProjectSources>,
    ) {
        self.project_alias_sources = sources;
    }

    /// Read-only borrow of the alias over-approximation dst set (R4 merge / R5
    /// Select entries whose sources span ≥2 genuine same-alloc reps). Consumed
    /// by `class_lifetime_extends_past_path_sensitive` to exclude conditional
    /// aliases from the same-alloc witness extension. Spec: Annex E §AIMS.
    #[must_use]
    pub(crate) fn alias_over_approximation_dsts(&self) -> &FxHashSet<ArcVarId> {
        &self.alias_over_approximation_dsts
    }

    /// Bulk-install the alias over-approximation dst set. Called once per
    /// function during `analyze_function` pre-walk alongside
    /// `set_project_alias_sources`. Read-only after this point per PL-5.
    pub(crate) fn set_alias_over_approximation_dsts(&mut self, dsts: FxHashSet<ArcVarId>) {
        self.alias_over_approximation_dsts = dsts;
    }

    /// Whether `var` is the destination of an Apply/Invoke whose callee
    /// `MemoryContract` carried `return_alias != None` for one or more
    /// Owned params. O(1) lookup against the pre-walk-populated
    /// `apply_result_aliases` map.
    ///
    /// Consumed for narrowing `should_suppress_return_transfer_dec`
    /// interactions on apply-alias destinations.
    #[must_use]
    pub fn is_apply_alias_destination(&self, var: ArcVarId) -> bool {
        self.apply_result_aliases.contains_key(&var)
    }

    // SSA-alias equivalence-class accessors ( )

    /// Return the equivalence-class id for `var` if it participates in a
    /// multi-member class; `None` for singletons.
    #[must_use]
    pub fn ssa_alias_class_of(&self, var: ArcVarId) -> Option<u32> {
        self.ssa_alias_classes.get(&var).copied()
    }

    /// Return the set of class members for `class_id`, if any.
    #[must_use]
    pub fn class_members(&self, class_id: u32) -> Option<&FxHashSet<ArcVarId>> {
        self.class_members.get(&class_id)
    }

    /// Return the set of source-candidate vars recorded for `class_id`.
    /// Used by `should_suppress_apply_aliased_dec` to detect apply-source
    /// roles for caller-side dec suppression.
    #[must_use]
    pub fn class_apply_alias_source_candidates(
        &self,
        class_id: u32,
    ) -> Option<&FxHashSet<ArcVarId>> {
        self.class_apply_alias_source_candidates.get(&class_id)
    }

    /// Iterate every `(class_id, members)` entry. `populate_class_covered`
    /// walks every class to compute coverage.
    pub(crate) fn class_members_iter(&self) -> impl Iterator<Item = (u32, &FxHashSet<ArcVarId>)> {
        self.class_members.iter().map(|(k, v)| (*k, v))
    }

    /// Return the set of parent class ids whose drop transitively covers
    /// `class_id`'s RC slot via a transitive-drop `RcStrategy`.
    ///
    /// PIN-6 inter-class payload-of relation per entry
    /// `class_id → {parent_id, ...}` means: at some `Construct` /
    /// `Apply` / `Invoke` / `PartialApply` / `Set` instruction, a
    /// `class_id` member was `[own]`-consumed to construct or fill a
    /// payload of a class-`parent_id` aggregate, and `parent_id`'s
    /// `RcStrategy` is in the transitive-drop set per
    /// [`is_transitive_drop_strategy`].
    ///
    /// [`is_transitive_drop_strategy`]: crate::ir::is_transitive_drop_strategy
    #[must_use]
    pub fn class_payload_of(&self, class_id: u32) -> Option<&FxHashSet<u32>> {
        self.class_payload_of.get(&class_id)
    }

    /// Bulk-install the pre-computed SSA-alias-class output. Read-only after
    /// this point per PL-5 (no-stale-summary invariant).
    pub fn set_ssa_alias_output(
        &mut self,
        class_table: FxHashMap<ArcVarId, u32>,
        class_members: FxHashMap<u32, FxHashSet<ArcVarId>>,
        class_apply_alias_source_candidates: FxHashMap<u32, FxHashSet<ArcVarId>>,
        class_payload_of: FxHashMap<u32, FxHashSet<u32>>,
    ) {
        self.ssa_alias_classes = class_table;
        self.class_members = class_members;
        self.class_apply_alias_source_candidates = class_apply_alias_source_candidates;
        self.class_payload_of = class_payload_of;
    }

    // class_covered accessors

    /// Whether the SSA-alias class `class_id` is fully burden-covered (every
    /// member's burden walk owns its RC traffic AND every transitive payload
    /// class is also covered).
    ///
    /// Returns `false` for unknown class ids (sparse — absence means not
    /// covered). Consumed by `decide()` at realization to force `RcDecision::None`
    /// when the burden walk owns the var's RC ops.
    ///
    /// See [`class_covered`](Self::class_covered) field doc.
    #[must_use]
    pub fn is_class_covered(&self, class_id: u32) -> bool {
        self.class_covered.contains(&class_id)
    }

    /// Number of classes currently marked covered. Test/diagnostic accessor.
    #[must_use]
    pub fn class_covered_count(&self) -> usize {
        self.class_covered.len()
    }

    /// Install the post-convergence `class_covered` set.
    ///
    /// Called by [`populate_class_covered`](super::post_convergence::populate_class_covered)
    /// after the fixed-point computation completes. Read-only thereafter per
    /// PL-5 (no-stale-summary invariant).
    pub(crate) fn set_class_covered(&mut self, covered: FxHashSet<u32>) {
        self.class_covered = covered;
    }

    /// Install the post-convergence `class_payload_of` edge map.
    ///
    /// Bypasses the bulk `set_ssa_alias_output` which runs at step 4
    /// pre-worklist. The post-convergence pass computes a path-sensitive
    /// edge set using converged `AimsStateMap` liveness, then installs it
    /// here.
    pub(crate) fn set_class_payload_of(&mut self, payload_map: FxHashMap<u32, FxHashSet<u32>>) {
        self.class_payload_of = payload_map;
    }

    /// Return the `class_dec_obligations` table.
    ///
    /// Empty by default; populated by the post-convergence pass
    /// `populate_class_dec_obligations` when multi-member SSA alias classes
    /// require path-sensitive same-slot dec dedup. Consumed by
    /// `walk_dec.rs::class_alive_after`.
    pub(crate) fn class_dec_obligations(
        &self,
    ) -> &FxHashMap<(ArcBlockId, u32), ClassObligationEntry> {
        &self.class_dec_obligations
    }

    /// Install the `class_dec_obligations` table after post-convergence
    /// computation — typed pre-pass input on `AimsStateMap` per
    /// AIMS Invariant #5(c). Read-only thereafter.
    pub(crate) fn set_class_dec_obligations(
        &mut self,
        obligations: FxHashMap<(ArcBlockId, u32), ClassObligationEntry>,
    ) {
        self.class_dec_obligations = obligations;
    }

    /// Materialize a singleton `class_members` entry for `class_id` if absent.
    ///
    /// Singleton id == `ArcVarId::raw()` per the existing scheme; recovers
    /// the var via `ArcVarId::new(class_id)` and inserts both the
    /// class-members and ssa-alias-classes entries idempotently.
    /// Required by the post-convergence pass after recording new edges so
    /// PIN-6's `class_members(parent)` lookup succeeds for singleton
    /// parents/children.
    pub(crate) fn ensure_singleton_class(&mut self, class_id: u32) {
        if self.class_members.contains_key(&class_id) {
            return;
        }
        let var = ArcVarId::new(class_id);
        let mut singleton: FxHashSet<ArcVarId> = FxHashSet::default();
        singleton.insert(var);
        self.class_members.insert(class_id, singleton);
        self.ssa_alias_classes.entry(var).or_insert(class_id);
    }

    /// Resolve a var's class id, falling back to its raw u32 (singleton id).
    ///
    /// `ssa_alias_class_of(var)` returns `Some(class_id)` for vars in a
    /// multi-member class and `None` for singletons. Singleton class id
    /// equals `var.raw()` per the existing materialization scheme; this
    /// helper consolidates the lookup so callers don't repeat the fallback.
    /// Used by the post-convergence edge recorder to resolve arg/dst class ids
    /// without re-running the local `UnionFind` from `compute_ssa_alias_classes`
    ///
    pub(crate) fn class_id_of(&self, var: ArcVarId) -> u32 {
        self.ssa_alias_class_of(var).unwrap_or_else(|| var.raw())
    }

    // Invoke edge states

    /// Get the per-edge demand state for a block ending in Invoke.
    ///
    /// Returns `None` for blocks that don't end in Invoke.
    #[must_use]
    pub fn invoke_edge_state(&self, block: ArcBlockId) -> Option<&InvokeEdgeState> {
        self.invoke_edge_states.get(&block)
    }

    /// Store per-edge state during analysis when processing an Invoke terminator.
    pub fn set_invoke_edge_state(&mut self, block: ArcBlockId, state: InvokeEdgeState) {
        self.invoke_edge_states.insert(block, state);
    }

    // Per-variable shape

    /// Get the shape classification for a variable from its definition.
    ///
    /// Returns `NonReusable` for variables without a recorded shape
    /// (block parameters, function parameters, or variables defined by
    /// non-shaping instructions).
    ///
    /// This is a per-variable property (set at the definition point),
    /// NOT a per-block state. Unlike the backward-computed lattice dimensions,
    /// shape doesn't change across block boundaries.
    ///
    /// Shape Activation.
    #[must_use]
    pub fn var_shape(&self, var: ArcVarId) -> ShapeClass {
        self.var_shapes
            .get(&var)
            .copied()
            .unwrap_or(ShapeClass::NonReusable)
    }

    /// Record the shape for a variable, derived from its definition instruction.
    ///
    /// Called by `populate_var_shapes()` post-convergence.
    pub fn set_var_shape(&mut self, var: ArcVarId, shape: ShapeClass) {
        if !matches!(shape, ShapeClass::NonReusable) {
            self.var_shapes.insert(var, shape);
        }
    }

    // Per-variable contract-narrowed call-result side tables ( )

    /// Record the contract-narrowed uniqueness for a call-result variable.
    ///
    /// BOTTOM-default sparse filter: `Uniqueness::Unique` is the lattice
    /// BOTTOM and is NOT stored — effective queries fall through to lattice
    /// (which is also Unique by default), giving identical behavior. The
    /// filter SHALL skip `Unique` (NOT `MaybeShared`); skipping `MaybeShared`
    /// would erase the load-bearing `ori_list_slice_drop` case where
    /// `return_info.uniqueness = MaybeShared` overrides the optimistic
    /// lattice default — that override is what fixes BUG-04-086.
    pub fn set_var_uniqueness(&mut self, var: ArcVarId, uniq: Uniqueness) {
        if !matches!(uniq, Uniqueness::Unique) {
            self.var_uniqueness.insert(var, uniq);
        }
    }

    /// Record the contract-narrowed locality for a call-result variable.
    ///
    /// BOTTOM-default sparse filter: `Locality::BlockLocal` is the lattice
    /// BOTTOM and is NOT stored. `FunctionLocal`, `HeapEscaping`, and
    /// `Unknown` (the CONSERVATIVE default for direct-no-contract calls)
    /// ARE stored. The filter SHALL skip BOTTOM, NOT CONSERVATIVE — the
    /// asymmetry is the same as `set_var_uniqueness` and serves the same
    /// architectural purpose.
    pub fn set_var_locality(&mut self, var: ArcVarId, loc: Locality) {
        if !matches!(loc, Locality::BlockLocal) {
            self.var_locality.insert(var, loc);
        }
    }

    /// Get the contract-narrowed uniqueness if the side table has an entry
    /// for `var`. Returns `None` when no contract narrowing applies.
    ///
    /// Distinguishes "unset" from "set to BOTTOM" (also None — BOTTOM never
    /// inserts). This presence-awareness is load-bearing for the
    /// `effective_uniqueness_at_block_*` JOIN semantics — an unset variable
    /// is NOT semantically equivalent to one set to `MaybeShared`, despite
    /// both differing from Unique. The override-pattern alternative is an AIMS
    /// Invariant 5 violation; presence-aware lookup + JOIN is the correct fix.
    #[must_use]
    pub fn contract_uniqueness(&self, var: ArcVarId) -> Option<Uniqueness> {
        self.var_uniqueness.get(&var).copied()
    }

    /// Get the contract-narrowed locality if the side table has an entry
    /// for `var`. Returns `None` when no contract narrowing applies.
    #[must_use]
    pub fn contract_locality(&self, var: ArcVarId) -> Option<Locality> {
        self.var_locality.get(&var).copied()
    }

    /// Effective uniqueness combining contract-narrowed forward state with
    /// the lattice's block-entry value for a call-result variable.
    ///
    /// Semantics: presence-aware lookup with lattice JOIN. When the side
    /// table is unset, returns the lattice value directly (no contract
    /// narrowing). When set, JOINs the contract value with the lattice
    /// value.
    ///
    /// JOIN preserves lattice widening: a contract claiming Unique that
    /// conflicts with backward demand's `MaybeShared` converges to `MaybeShared`,
    /// not Unique. This is the unified-model semantics
    /// Invariant 5 — the side table FEEDS INTO the lattice via JOIN, never
    /// overrides it. The override alternative (returning the side-table
    /// value when present, ignoring the lattice) suppresses backward demand
    /// widening and is rejected.
    #[must_use]
    pub fn effective_uniqueness_at_block_entry(
        &self,
        block: ArcBlockId,
        var: ArcVarId,
    ) -> Uniqueness {
        let lattice = self.var_state_at_block_entry(block, var).uniqueness;
        match self.contract_uniqueness(var) {
            Some(contract) => contract.join(lattice),
            None => lattice,
        }
    }

    /// Effective uniqueness combining contract-narrowed forward state with
    /// the lattice's block-exit value. See [`effective_uniqueness_at_block_entry`]
    /// for JOIN semantics; this variant queries the exit-side lattice value.
    ///
    /// Entry-side and exit-side variants are NOT interchangeable — consumer
    /// sites that read different sides (COW reads entry, `drop_hints` read
    /// exit) MUST call the matching helper.
    #[must_use]
    pub fn effective_uniqueness_at_block_exit(
        &self,
        block: ArcBlockId,
        var: ArcVarId,
    ) -> Uniqueness {
        let lattice = self.var_state_at_block_exit(block, var).uniqueness;
        match self.contract_uniqueness(var) {
            Some(contract) => contract.join(lattice),
            None => lattice,
        }
    }

    /// Effective locality combining contract-narrowed forward state with
    /// the lattice's block-entry value. JOIN semantics (`max` per
    /// §1.5: `BlockLocal` < `FunctionLocal` < `HeapEscaping` <
    /// Unknown — shipped 4-value chain; the spec's 5-value `ArgEscaping`
    /// is target-only per the spec's vocabulary-changes preamble).
    #[must_use]
    pub fn effective_locality_at_block_entry(&self, block: ArcBlockId, var: ArcVarId) -> Locality {
        let lattice = self.var_state_at_block_entry(block, var).locality;
        match self.contract_locality(var) {
            Some(contract) => contract.join(lattice),
            None => lattice,
        }
    }

    /// Effective locality combining contract-narrowed forward state with
    /// the lattice's block-exit value. See [`effective_locality_at_block_entry`].
    #[must_use]
    pub fn effective_locality_at_block_exit(&self, block: ArcBlockId, var: ArcVarId) -> Locality {
        let lattice = self.var_state_at_block_exit(block, var).locality;
        match self.contract_locality(var) {
            Some(contract) => contract.join(lattice),
            None => lattice,
        }
    }

    // Sparse event table

    /// Get the event slice for a specific block.
    ///
    /// Returns an empty slice if no events recorded for that block.
    #[must_use]
    pub fn events_in_block(&self, block: ArcBlockId) -> &[AimsEvent] {
        self.events
            .get(&block)
            .map_or(&[], |events| events.as_slice())
    }

    /// Append a sparse event to the block's event list.
    pub fn record_event(&mut self, event: AimsEvent) {
        let block = event.block();
        self.events.entry(block).or_default().push(event);
    }

    // Effect summary

    /// Get the accumulated function-level effect summary.
    ///
    /// Populated by `populate_effect_summary()` after convergence.
    /// Returns `EffectSummary::default()` (all false) if not yet populated.
    #[must_use]
    pub fn effect_summary(&self) -> EffectSummary {
        self.effect_summary
    }

    /// Join an effect into the function-level accumulator.
    ///
    /// Each flag is OR'd: once set, it stays set.
    ///
    /// Note: `has_unbounded_stack` is NOT set during per-block accumulation.
    /// It remains `false` here; `extract_contract()` sets it from SCC
    /// membership and syntactic tail-position analysis.
    pub fn accumulate_effect(&mut self, effect: EffectSummary) {
        self.effect_summary = self.effect_summary.join(&effect);
    }

    // FIP token balance

    /// Set the FIP allocation balance counts from post-convergence analysis.
    ///
    /// `construct_count`: non-scalar `Construct` instructions with reusable ctor kinds.
    /// `consumed_count`: consumed values with `ReusableCtor` shape (provide reuse tokens).
    /// Effect Activation.
    pub fn set_fip_balance(&mut self, construct_count: u32, consumed_count: u32) {
        self.fip_construct_count = construct_count;
        self.fip_consumed_count = consumed_count;
    }

    /// Number of non-scalar `Construct` instructions with reusable ctor kinds.
    ///
    /// Effect Activation.
    #[must_use]
    pub fn fip_construct_count(&self) -> u32 {
        self.fip_construct_count
    }

    /// Whether the function's allocations are token-balanced by consumed values.
    ///
    /// `true` means consumed values with reusable shape >= construct allocations,
    /// so every Construct can potentially reuse memory from a consumed value.
    /// This is a necessary condition for FIP certification.
    /// Effect Activation.
    #[must_use]
    pub fn fip_token_balanced(&self) -> bool {
        self.fip_consumed_count >= self.fip_construct_count
    }

    /// Net allocation count: constructs beyond what consumed values can supply.
    ///
    /// Returns 0 when balanced (FIP), positive when the function needs more
    /// allocations than it can reuse. Used for `FipContract::Bounded(n)`.
    /// Effect Activation.
    #[must_use]
    pub fn fip_net_allocation(&self) -> u32 {
        self.fip_construct_count
            .saturating_sub(self.fip_consumed_count)
    }

    // Summary queries

    /// Number of blocks in the state map.
    #[must_use]
    pub fn num_blocks(&self) -> usize {
        self.block_entry_states.len()
    }

    /// Number of variables tracked (including scalars).
    #[must_use]
    pub fn num_vars(&self) -> usize {
        self.scalars.len()
    }
}
