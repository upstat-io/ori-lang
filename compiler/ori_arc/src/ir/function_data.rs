//! Function-level ARC IR metadata and carrier.

use ori_ir::canon::MethodProducerId;
use ori_ir::{Name, Span};
use ori_types::{DerivedCallPosition, Idx, MethodProducer};

use crate::uniqueness::{CowAnnotations, DropHints};

use super::{
    ArcBlock, ArcBlockId, ArcParam, ArcVarId, PrimOp, PrimitiveFacts, RcStrategy, ValueRepr,
};

// Functions

/// Readiness of logical representation metadata and the transitional
/// RC-adapter strategy table.
///
/// The explicit state distinguishes an unrealized function from a realized
/// function with zero variables; both otherwise have three empty vectors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum VariableMetadataState {
    /// Lowering or an isolated pre-realization transform owns only types.
    #[default]
    Unrealized,
    /// Representation metadata is exact, but current carrier strategies are
    /// not yet derived.
    ///
    /// Canonical-to-ARC lowering enters this state before the AIMS pipeline.
    /// Keeping it distinct from [`Unrealized`](Self::Unrealized) makes an
    /// empty, zero-variable representation table unambiguous.
    RepresentationsReady,
    /// Logical representations and transitional carrier strategies are exact
    /// and parallel to types.
    Realized,
}

/// Source form of one direct method call preserved through ARC lowering.
///
/// The form determines whether the receiver is an explicit source operand.
/// It is semantic call-resolution provenance, not a backend dispatch choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum MethodCallForm {
    /// `value.method(...)`; the receiver is the first call operand.
    Instance,
    /// `Type.method(...)`; the owner is not a call operand.
    Associated,
}

/// Exact owner provenance for a direct method-call result register.
///
/// ARC transformations keep SSA registers stable, so the destination is a
/// durable key across block movement. User-impl closure and builtin-registry
/// closure both consume this fact; a free function with the same spelling has
/// no fact and therefore cannot be cross-wired to a method.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct MethodCallFact {
    /// Result register defined by the `Apply` or `Invoke` call.
    pub destination: ArcVarId,
    /// Semantic receiver/owner type selected by type checking.
    pub receiver_type: Idx,
    /// Whether the receiver is an explicit source operand.
    pub form: MethodCallForm,
    /// Exact executable producer when this call was compiler-generated.
    ///
    /// Source calls not yet frozen at this seam carry `None`; every generated
    /// call must carry `Some` before executable closure.
    pub producer: Option<MethodProducer>,
    /// Type-checker table handle for a source-selected method producer.
    ///
    /// Realization resolves this handle against the matching `TypedModule`
    /// before mono/local/imported target closure and fills [`Self::producer`].
    pub selected_producer: Option<MethodProducerId>,
    /// Structural generated-body position paired with [`Self::producer`].
    pub derived_position: Option<DerivedCallPosition>,
}

/// Source operator call awaiting one exact implementation target.
///
/// User-defined operators lower as ordinary may-unwind calls so their cleanup
/// and `catch(expr:)` edges exist while lexical catch context is available.
/// Realization consumes this fact before AIMS to replace the surface method
/// name with the exact implementation identity selected for `receiver_type`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct OperatorCallFact {
    /// Result register defined by the unresolved operator call.
    pub destination: ArcVarId,
    /// Stable source receiver whose realized type selects the implementation.
    pub receiver: ArcVarId,
    /// Source operation whose trait method owns the call.
    pub operation: PrimOp,
    /// Source span restored onto a builtin or projected result instruction.
    pub span: Option<Span>,
}

/// Exact producer provenance for a compiler-generated direct free call.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct DirectCallFact {
    /// Result register defined by the direct `Apply` or `Invoke`.
    pub destination: ArcVarId,
    /// Exact executable producer selected by type checking.
    pub producer: MethodProducer,
    /// Structural generated-body position.
    pub derived_position: DerivedCallPosition,
}

/// Stable identity for one yield-comprehension accumulator allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocationSiteId(u32);

impl AllocationSiteId {
    /// Construct an allocation-site identity from its deterministic function-local index.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Return the function-local allocation-site index.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Neutral upper-bound evidence for a yield-comprehension allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum YieldExtent {
    /// The lowering seam proved an exact constant element bound.
    StaticExact(u64),
    /// The bound is computed at runtime from the iterable's own length contract.
    RuntimeExact(ArcVarId),
    /// No safe bound is available; the growable fallback remains required.
    Unknown,
}

/// AIMS' frozen lifetime verdict for one yield allocation result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum YieldAllocationLocality {
    /// Analysis has not frozen a verdict yet; physical consumers fail closed.
    #[default]
    Unknown,
    /// The result is bounded by the owning function or a nested block.
    Local,
    /// The result may outlive the owning function or otherwise lacks proof.
    Escaping,
}

/// Representation-owned dynamic execution evidence for one allocation site.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum YieldAllocationExecution {
    /// Analysis has not proved that one physical slot represents every dynamic
    /// instance. This includes allocation sites inside CFG cycles.
    #[default]
    RepeatedOrUnknown,
    /// The defining block cannot execute more than once per function call.
    SingleExecution,
}

/// A lowering-owned yield allocation, keyed independently of instruction position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct YieldAllocationFact {
    /// Stable identity within the owning function.
    pub site: AllocationSiteId,
    /// Scratch `ori_list_new` result consumed by push/take operations.
    pub builder: ArcVarId,
    /// List value produced by `ori_list_take`.
    pub result: ArcVarId,
    /// Semantic list element type.
    pub elem_ty: Idx,
    /// Physical compatibility size passed to the current list runtime ABI.
    pub elem_size: u64,
    /// Backend-neutral capacity evidence.
    pub extent: YieldExtent,
    /// AIMS-owned lifetime verdict, frozen after convergence.
    pub locality: YieldAllocationLocality,
    /// Dynamic execution verdict, frozen from the final post-AIMS CFG.
    pub execution: YieldAllocationExecution,
    /// Whether the physical collection backing must retain the runtime RC
    /// header immediately before its element data.
    ///
    /// Lowering initializes this conservatively. AIMS may clear it only for a
    /// closed primitive-scalar lineage whose complete use set needs neither
    /// sharing state nor element cleanup.
    pub requires_runtime_header: bool,
}

/// A complete function in the ARC IR.
///
/// Contains everything needed for ARC analysis: the function signature
/// with ownership annotations, basic blocks, and metadata mapping
/// variables back to types and source spans.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct ArcFunction {
    /// The function's mangled name.
    pub name: Name,
    /// Function parameters with ownership annotations.
    pub params: Vec<ArcParam>,
    /// The return type.
    pub return_type: Idx,
    /// Basic blocks in definition order. `blocks[entry.index()]` is the entry.
    pub blocks: Vec<ArcBlock>,
    /// The entry block ID.
    pub entry: ArcBlockId,
    /// Type of each variable, indexed by `ArcVarId::index()`.
    pub var_types: Vec<Idx>,
    /// Backend-neutral ownership-relevant shape of each variable, indexed by
    /// `ArcVarId::index()`.
    ///
    /// Canonical-to-ARC lowering computes this table and enters
    /// [`VariableMetadataState::RepresentationsReady`]. The AIMS pipeline
    /// recomputes and validates it before entering the fully realized state.
    /// Physical width, offset, ABI, register, and VM-slot choices belong to
    /// the selected layout projection and are not stored here.
    ///
    /// Skipped during cache serialization — derived from `var_types` + Pool,
    /// not an independent data source.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub var_reprs: Vec<ValueRepr>,
    /// Cached transitional RC adapter strategy for each variable, indexed by
    /// `ArcVarId::index()`.
    /// `None` means no current counter-carrier strategy is required. Computed
    /// alongside [`var_reprs`](Self::var_reprs) at the start of the ARC pipeline so
    /// downstream pre-walk passes (the SSA alias-class computation in
    /// `intraprocedural::ssa_alias_classes` + the transitive-drop edge
    /// materialization in `intraprocedural::post_convergence`) can classify a
    /// var's strategy without holding a `&Pool` reference.
    ///
    /// Empty until the current AIMS pipeline derives it from the ready
    /// representation table, then parallel to `var_types` in the fully realized
    /// state. This preserves shipped behavior but is not the production AIMS
    /// seam; stable logical value/cleanup IDs replace it before physical planning.
    /// Skipped during cache serialization for the same reason as
    /// `var_reprs` — derived from `var_types` + `var_reprs` + Pool, not an
    /// independent data source.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub var_rc_strategies: Vec<Option<RcStrategy>>,
    /// Authoritative lifecycle of the two derived variable metadata tables.
    ///
    /// This is authoritative even when all three variable tables are empty.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub var_metadata_state: VariableMetadataState,
    /// Source spans for instructions, indexed by `[block_index][instr_index]`.
    /// `None` for synthetic instructions (for example, materialized logical
    /// ownership events in the current `Rc*` carrier).
    ///
    /// Skipped during cache serialization — spans are source metadata not needed
    /// for cached codegen. Deserialized functions get empty span vectors.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub spans: Vec<Vec<Option<Span>>>,
    /// Whether this function is annotated `#fbip` for constructor-reuse enforcement.
    ///
    /// When true, the pipeline checks that all constructor allocations are
    /// reused in-place. Missed reuse produces an `ArcProblem::FbipViolation`.
    #[cfg_attr(feature = "cache", serde(default))]
    pub is_fbip: bool,
    /// Number of leading parameters that are captures (not user parameters).
    ///
    /// Set by `lower_lambda()` — top-level functions always have `0`.
    /// Available to physical projections when choosing a closure encoding.
    /// The LLVM backend currently uses it to detect non-capturing lambdas and
    /// skip trampoline wrapper generation; that projection does not own the
    /// shared capture fact.
    #[cfg_attr(feature = "cache", serde(default))]
    pub num_captures: usize,
    /// Per-instruction COW mode annotations from uniqueness analysis.
    ///
    /// Maps `(block_index, instr_index)` to [`CowMode`] for each COW
    /// operation. Physical consumers may use this AIMS verdict to select only
    /// the fast path (`StaticUnique`), only the slow path (`StaticShared`), or
    /// the full runtime check (`Dynamic`). The LLVM ARC emitter is the current
    /// compiled projection of that shared fact.
    ///
    /// Populated by the ARC pipeline (after uniqueness analysis). Empty
    /// until then. Skipped during cache serialization — derived from the
    /// analysis, not an independent data source.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub cow_annotations: CowAnnotations,
    /// Typed primitive-operation facts resolved once by AIMS.
    ///
    /// Keyed by stable destination variable, validated for exact `PrimOp`
    /// coverage, and included in structural identity and artifact hashes.
    /// Skipped by the source cache because it is deterministically re-resolved
    /// from typed IR before analysis.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub primitive_facts: PrimitiveFacts,
    /// Per-instruction drop hints for unique collection drops.
    ///
    /// Identifies `RcDec` instructions where the target collection has exactly
    /// one logical owner, allowing a physical consumer to select its
    /// unique-drop fast path. LLVM currently maps that verdict to
    /// `ori_buffer_drop_unique` instead of `ori_buffer_rc_dec`.
    ///
    /// Computed after logical ownership-event pair elision in the current
    /// transitional carrier.
    /// Skipped during cache serialization — derived data.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub drop_hints: DropHints,
    /// Detected self-recursive tail call sites.
    ///
    /// Each entry identifies the block and instruction index of an `Apply`
    /// in tail position. Populated by [`tail_call::detect_tail_calls`] in the
    /// AIMS pipeline (after current-carrier ownership-event elision). Consumed by the loop-lowering
    /// rewrite pass. Skipped during cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub tail_calls: Vec<crate::tail_call::TailCallSite>,
    /// Variables included in the per-variable burden-balance verifier.
    ///
    /// Indexed by `ArcVarId::index()`. Direct verifier inputs may mark variables
    /// whose `BurdenInc` / `BurdenDec*` traffic must net to zero.
    ///
    /// Production class-ledger emission leaves this empty because it verifies
    /// the plan at class grain before mechanical Phase-7 lowering. Read by the
    /// VF-1 burden-balance verifier and skipped during cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub burden_emitted: Vec<bool>,
    /// Mutable-`Ident` reassignment death points: `(old_var, new_var)` pairs
    /// recorded by `lower_assign` when `x = e` rebinds a mutable binding.
    /// `old_var` is the binding's pre-reassignment value (orphaned by the SSA
    /// rebind); `new_var` is the value `x` now holds (the `Let { dst: new_var,
    /// value: Var(rhs) }`). The burden Phase-5 reassign-release scan
    /// (`compute_reassign_rebind_releases`) consumes these to emit the
    /// scope-rebind `BurdenDec(old_var)` per `Spec: Annex E §AIMS RL-2` (the
    /// binding's prior value reaches its scope-death at the rebind).
    ///
    /// Lowering-recorded structural facts (def-use shape), not derived from
    /// analysis. Skipped during cache serialization alongside the other
    /// lowering-time vecs.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub reassign_deaths: Vec<(ArcVarId, ArcVarId)>,
    /// Maps each may-panic inline checked-op `PrimOp` result var (integer
    /// div / mod / floor-div / shift, or add / sub / mul / neg overflow per
    /// `Spec: Clause 14.3`) lowered lexically inside a same-frame `catch(expr:)`
    /// body to that catch's handler block.
    ///
    /// An inline checked-op never references the catch handler via an `Invoke`
    /// unwind edge, so without this the handler would be dead-eliminated and
    /// the checked-op panic would abort instead of being caught. This map:
    /// - keeps every distinct handler block live (`compact_blocks` seeds the
    ///   reachability DFS from each map value);
    /// - lets each physical projection route ONLY a mapped checked-op's panic
    ///   to ITS handler. LLVM currently materializes landing pads and uses
    ///   `invoke`; the per-dispatch + per-handler scope ensures a subsequent
    ///   uncaught `1 / 0`, or a checked-op caught by a different nested catch,
    ///   routes correctly.
    ///
    /// `ArcVarId`s are SSA-stable across the pipeline's block-merge / compaction
    /// (vars are never renumbered), so the keys survive those passes; the
    /// handler `ArcBlockId` values are remapped by `compact_blocks`. Empty when
    /// no same-frame catch body carries an inline checked-op (the existing
    /// `Invoke`-unwind path handles callee-function panics).
    ///
    /// Stored as a `Vec` of `(var, handler)` pairs (not a map) so `ArcFunction`
    /// keeps its `Hash`/`Eq` derives — `FxHashMap` is neither; the Vec also
    /// preserves deterministic lowering-insertion order. Lowering-derived;
    /// skipped during cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub catch_scoped_checked_ops: Vec<(ArcVarId, ArcBlockId)>,
    /// Type-checker-selected owner provenance for direct method calls.
    ///
    /// Stored as a deterministic destination-keyed vector so `ArcFunction`
    /// retains structural equality and hashing. Free calls are represented by
    /// absence. Entries survive block compaction because variables are never
    /// renumbered; executable closure validates them against the final call
    /// stream before selecting any physical backend.
    #[cfg_attr(feature = "cache", serde(default))]
    pub method_call_facts: Vec<MethodCallFact>,
    /// User-defined operator calls awaiting exact target closure.
    ///
    /// Lowering-owned and consumed transactionally during pre-AIMS batch
    /// preparation. A closed executable must never retain an entry.
    #[cfg_attr(feature = "cache", serde(default))]
    pub operator_call_facts: Vec<OperatorCallFact>,
    /// Exact compiler-generated free-call producer facts.
    #[cfg_attr(feature = "cache", serde(default))]
    pub direct_call_facts: Vec<DirectCallFact>,
    /// Stable yield-comprehension allocation facts recorded during lowering.
    ///
    /// AIMS and representation planning consume these identities without
    /// rediscovering syntax or depending on mutable instruction positions.
    #[cfg_attr(feature = "cache", serde(default))]
    pub yield_allocations: Vec<YieldAllocationFact>,
    /// Whether the class-ledger emitter committed its verified Step-4b plan for
    /// this function.
    ///
    /// `true` = the burden ops in the instruction stream ARE the class-ledger
    /// plan: realization lowers them mechanically in Phase 7. Step-10's
    /// redundant-project-alias dec cleanup is also skipped because it is not
    /// part of the class-ledger path. Set only by
    /// `aims::class_ledger::attempt_replacement`. Skipped during cache
    /// serialization: a cache-restored function deserializes to `false`,
    /// which is safe only because the flag is re-derived on every pipeline
    /// run (function-level caching of post-pipeline IR is not wired; if it
    /// lands, this flag must be cached or re-derived with it).
    #[cfg_attr(feature = "cache", serde(skip))]
    pub class_ledger_emission: bool,
}
