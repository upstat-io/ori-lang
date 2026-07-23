//! Function-level ARC IR metadata and carrier.

use ori_ir::canon::MethodProducerId;
use ori_ir::{Name, Span};
use ori_types::{DerivedCallPosition, Idx, MethodProducer};

use crate::uniqueness::{CowAnnotations, DropHints};

use super::{
    ArcBlock, ArcBlockId, ArcParam, ArcVarId, PrimOp, PrimitiveFacts, RcStrategy, ValueRepr,
};

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
    /// Representation metadata is exact, while carrier strategies are absent.
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
    /// Source calls unresolved at this seam carry `None`; every generated call
    /// carries `Some` before executable closure.
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
    /// Lowered element-size operand shared by the allocation and push calls.
    pub elem_size_var: ArcVarId,
    /// Physical compatibility size passed to the current list runtime ABI.
    pub elem_size: u64,
    /// Backend-neutral capacity evidence.
    pub extent: YieldExtent,
    /// AIMS-owned lifetime verdict, frozen after convergence.
    pub locality: YieldAllocationLocality,
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
    /// Backend-neutral ownership shape indexed by `ArcVarId::index()`.
    /// Lowering computes it and AIMS validates it before realization; physical
    /// layout choices remain projection-owned. Cache serialization skips this
    /// table because it is derived from `var_types` and the type pool.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub var_reprs: Vec<ValueRepr>,
    /// Transitional RC adapter strategy indexed by `ArcVarId::index()`.
    /// `None` means no counter carrier is required. In the realized state this
    /// table is parallel to `var_types` and derived from the ready representation
    /// table, so cache serialization skips it.
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
    /// Zero captures permit physical projections to omit a trampoline wrapper.
    #[cfg_attr(feature = "cache", serde(default))]
    pub num_captures: usize,
    /// Per-instruction COW mode annotations from uniqueness analysis.
    ///
    /// Maps `(block_index, instr_index)` to [`CowMode`] for each COW
    /// operation. Physical consumers may use this AIMS verdict to select only
    /// the fast path (`StaticUnique`), only the slow path (`StaticShared`), or
    /// the full runtime check (`Dynamic`). The ARC pipeline populates this table
    /// after uniqueness analysis; cache serialization skips the derived data.
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
    /// Marks `RcDec` instructions whose collection has one logical owner, allowing
    /// physical consumers to select `ori_buffer_drop_unique`. Computed after
    /// ownership-event pair elision and skipped during cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub drop_hints: DropHints,
    /// Detected self-recursive tail call sites.
    ///
    /// Each entry identifies the block and instruction index of an `Apply`
    /// in tail position after current-carrier ownership-event elision.
    /// Skipped during cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub tail_calls: Vec<crate::tail_call::TailCallSite>,
    /// Variables included in the per-variable burden-balance verifier.
    ///
    /// Indexed by `ArcVarId::index()`. Direct verifier inputs may mark variables
    /// whose `BurdenInc` / `BurdenDec*` traffic must net to zero.
    ///
    /// Production class-ledger emission leaves this empty because it verifies
    /// the plan at class grain before mechanical Phase-7 lowering. Skipped
    /// during cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub burden_emitted: Vec<bool>,
    /// Mutable-binding reassignment death points as `(old_var, new_var)` pairs.
    /// Lowering records the SSA rebind; release analysis consumes the old value
    /// as the scope-death target. These structural facts are skipped during
    /// cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub reassign_deaths: Vec<(ArcVarId, ArcVarId)>,
    /// Maps may-panic inline checked-op results to same-frame catch handlers.
    /// The mapping keeps otherwise edgeless handlers live and routes only their
    /// lexical operations. SSA-stable variable keys survive compaction; handler
    /// IDs are remapped. A vector preserves structural hashing and deterministic
    /// insertion order. Cache serialization skips these lowering-derived facts.
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
    /// Whether the instruction stream contains a verified class-ledger plan.
    /// Realization lowers such burden operations mechanically and skips the
    /// alternate redundant-alias cleanup. Cache restoration clears this flag;
    /// every pipeline run derives it again before use.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub class_ledger_emission: bool,
}

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
