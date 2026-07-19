//! ARC IR — backend-neutral basic-block carrier for AIMS analysis.
//!
//! Despite the historical name, this IR carries logical ownership, lifetime,
//! cleanup, and reuse facts shared by every physical realization. Borrow
//! inference, ownership-event realization/elision, and constructor reuse
//! operate on it after lowering from the typed AST.
//!
//! # Architecture
//!
//! The ARC IR uses the conventional basic-block structure also found in LLVM
//! IR, Lean 4's LCNF, and Rust's MIR; no one target owns its semantics:
//!
//! - **[`ArcFunction`]** — a function body: parameters, blocks, variable types
//! - **[`ArcBlock`]** — a basic block: parameters, body instructions, terminator
//! - **[`ArcInstr`]** — an instruction (let, call, construct, logical ownership event)
//! - **[`ArcTerminator`]** — block exit (return, jump, branch, switch)
//!
//! Values are named via [`ArcVarId`] (SSA-like). Control flow uses
//! [`ArcBlockId`] references between blocks.

pub mod format;
mod function;
mod instr;
mod primitive;
mod repr;
mod terminator;
pub mod validate;

pub use instr::ArcInstr;
pub use primitive::{PrimitiveFact, PrimitiveFacts};
pub(crate) use repr::derive_var_rc_strategies;
pub use repr::{
    compute_var_rc_strategies, compute_var_reprs, is_transitive_drop_strategy, RcAtomicity,
    RcStrategy, ValueRepr,
};

use ori_ir::canon::MethodProducerId;
use ori_ir::{BinaryOp, DurationUnit, Name, SizeUnit, Span, UnaryOp};
use ori_types::{DerivedCallPosition, Idx, MethodProducer};

use crate::uniqueness::{CowAnnotations, DropHints};
use crate::Ownership;

// Call-site argument ownership

/// Per-argument ownership at a call site.
///
/// Embedded directly in [`ArcInstr::Apply`] and [`ArcTerminator::Invoke`]
/// so downstream passes can read ownership without querying external data
/// (annotated signatures, string interner). Populated by the logical
/// ownership-event realization entry point
/// [`compute_arg_ownership`](crate::rc_insert); the module name reflects the
/// current transitional `Rc*` carrier.
///
/// Mirrors the two-state semantics of [`Ownership`] but scoped to a
/// specific call instruction rather than a function signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum ArgOwnership {
    /// One logical owner credit transfers to the callee. If the caller remains
    /// live, it needs another credit (currently carried by `RcInc`).
    Owned,
    /// No owner credit transfers; the source lifetime governs callee access.
    /// The caller retains the eventual discharge obligation (currently `RcDec`).
    Borrowed,
}

/// Jump-arg the edge passes for this param position, and whether the edge is a
/// loop BACK-EDGE (the target reaches the predecessor — the edge closes a CFG
/// cycle).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct ParamEdgeArg {
    /// The predecessor block jumping into the param's owning block.
    pub pred_block: usize,
    /// The Jump-arg passed at this param's position on THIS edge.
    pub arg: ArcVarId,
    /// True iff the edge closes a CFG cycle (the param's owning block reaches
    /// `pred_block`). Back-edge unification DECLINES conservatively: the
    /// back-edge arg is the NEXT iteration's value of the same lineage, never
    /// this iteration's allocation. Spec: Annex E §AIMS — merge-point
    /// filtering.
    pub is_back_edge: bool,
}

// ID newtypes

/// Variable ID within an ARC IR function.
///
/// Each `ArcVarId` identifies a unique SSA-like value within a single
/// [`ArcFunction`]. IDs are allocated sequentially starting from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct ArcVarId(u32);

impl ArcVarId {
    /// Sentinel value representing an invalid or uninitialized variable.
    ///
    /// Equal to `u32::MAX`. Functions that return `ArcVarId` should never
    /// produce this value; its presence indicates a lowering bug.
    pub const INVALID: Self = Self(u32::MAX);

    /// Create a new variable ID from a raw index.
    #[inline]
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Get the raw `u32` value.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }

    /// Get the index as `usize` (for indexing into `Vec`s).
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// Returns `true` if this is a valid (non-sentinel) variable ID.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

/// Basic block ID within an ARC IR function.
///
/// Each `ArcBlockId` identifies a basic block within a single
/// [`ArcFunction`]. IDs are allocated sequentially starting from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct ArcBlockId(u32);

impl ArcBlockId {
    /// Sentinel value representing an invalid or uninitialized block.
    ///
    /// Equal to `u32::MAX`. Functions that return `ArcBlockId` should never
    /// produce this value; its presence indicates a lowering bug.
    pub const INVALID: Self = Self(u32::MAX);

    /// Create a new block ID from a raw index.
    #[inline]
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Get the raw `u32` value.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }

    /// Get the index as `usize` (for indexing into `Vec`s).
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// Returns `true` if this is a valid (non-sentinel) block ID.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

// Literal values

/// Literal value in the ARC IR.
///
/// Mirrors the literal variants of `ExprKind` from `ori_ir`, but in a
/// form suitable for basic-block IR (no spans, no expression nesting).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum LitValue {
    Int(i64),
    Float(u64),
    Bool(bool),
    String(Name),
    Char(char),
    Duration {
        value: u64,
        unit: DurationUnit,
    },
    Size {
        value: u64,
        unit: SizeUnit,
    },
    Unit,
    /// Typed null reference — a zero-valued placeholder for reference fields
    /// that will be overwritten by `Set` before any read.
    ///
    /// Used by the TRMC rewrite to fill constructor hole fields.
    /// The variable carrying this value has the field's declared type, but the
    /// physical value is null (zero) and carries no logical ownership event.
    /// The shipped counter-backed runtime preserves that rule by making its
    /// transitional carrier helpers (`ori_rc_inc`/`ori_buffer_rc_dec`) no-ops
    /// on null; other physical plans must preserve the same hole semantics.
    ///
    /// # Contract
    ///
    /// A `Null` literal **must** be consumed by a `Construct` instruction whose
    /// corresponding field is overwritten by `Set` before any read of that field.
    /// The post-rewrite verifier enforces this invariant.
    Null,
}

// Primitive operations

/// Primitive operation — wraps `BinaryOp`/`UnaryOp` from `ori_ir`.
///
/// By wrapping rather than duplicating, we stay in sync automatically
/// when new operators are added to the language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimOp {
    Binary(BinaryOp),
    Unary(UnaryOp),
}

// Values

/// A value expression in the ARC IR.
///
/// Values are the right-hand side of `Let` instructions. They are
/// side-effect-free (except for primitive operations that may trap).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum ArcValue {
    /// Reference to an existing variable.
    Var(ArcVarId),
    /// A literal constant.
    Literal(LitValue),
    /// A primitive operation (arithmetic, comparison, logic, bitwise).
    PrimOp { op: PrimOp, args: Vec<ArcVarId> },
}

// Constructor kinds

/// The kind of constructor for a `Construct` instruction.
///
/// Distinguishes struct construction, enum variant construction, tuples,
/// collection literals, and closure captures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum CtorKind {
    /// Named struct: `Point { x: 1, y: 2 }`.
    Struct(Name),
    /// Enum variant by index: `Some(42)` → `EnumVariant { enum_name, variant: 0 }`.
    EnumVariant { enum_name: Name, variant: u32 },
    /// Tuple: `(1, "hello")`.
    Tuple,
    /// List literal: `[1, 2, 3]`.
    ListLiteral,
    /// Map literal: `{"a": 1}`.
    MapLiteral,
    /// Set literal: `{1, 2, 3}`.
    SetLiteral,
    /// Closure capture: packages captured variables into a closure object.
    Closure { func: Name },
}

impl CtorKind {
    /// True for the collection-literal constructors (`[..]` / `{k: v}` / `{..}`),
    /// each lowered to a fresh heap `CollectionBuffer` allocation. Canonical home
    /// for the `ListLiteral | MapLiteral | SetLiteral` classification consumed by
    /// the burden-lowering ownership scans.
    #[must_use]
    pub fn is_collection_literal(&self) -> bool {
        matches!(
            self,
            CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral
        )
    }
}

// Parameters

/// A function parameter in the ARC IR, annotated with ownership.
///
/// Ownership starts as `Owned` for all ref-typed parameters and is
/// refined to `Borrowed` by borrow inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct ArcParam {
    /// The variable ID bound to this parameter.
    pub var: ArcVarId,
    /// The parameter's type in the type pool.
    pub ty: Idx,
    /// Ownership annotation (set by borrow inference).
    pub ownership: Ownership,
}

// Terminators

/// Block terminator — how control leaves a basic block.
///
/// Every block ends with exactly one terminator. Terminators reference
/// successor blocks by [`ArcBlockId`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum ArcTerminator {
    /// Return a value from the function.
    Return { value: ArcVarId },

    /// Unconditional jump to a target block, passing arguments.
    Jump {
        target: ArcBlockId,
        args: Vec<ArcVarId>,
    },

    /// Conditional branch on a boolean.
    Branch {
        cond: ArcVarId,
        then_block: ArcBlockId,
        else_block: ArcBlockId,
    },

    /// Multi-way branch on an integer discriminant.
    Switch {
        scrutinee: ArcVarId,
        cases: Vec<(u64, ArcBlockId)>,
        default: ArcBlockId,
    },

    /// Call that may unwind (post-2026, for panic/effect support).
    /// On success, jumps to `normal`; on unwind, jumps to `unwind`.
    Invoke {
        dst: ArcVarId,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        /// Per-argument ownership at this call site.
        /// Parallel to `args`: `arg_ownership[i]` describes `args[i]`.
        /// Defaults to all `Owned`; populated by ownership-event realization.
        arg_ownership: Vec<ArgOwnership>,
        /// Abstract dispatch index for generic-instantiated calls.
        /// Mirrors the `mono_instance_id` slot on `ArcInstr::Apply`;
        /// `Invoke` is the may-unwind sibling carrier produced by
        /// `lower::calls::emit_call_or_invoke` when `is_nounwind_call`
        /// returns false. Sourced from `CanonResult.mono_dispatch_map_can`
        /// during ARC lowering. Physical realization consumers use it to
        /// identify the selected instance; `ori_llvm` currently maps it to
        /// `TypedModule.mono_instances[id.0]`. The evaluator independently
        /// consumes canonical dispatch provenance, not this ARC carrier.
        mono_instance_id: Option<ori_ir::canon::MonoInstanceId>,
        normal: ArcBlockId,
        unwind: ArcBlockId,
    },

    /// Indirect call (through closure) that may unwind.
    /// On success, jumps to `normal`; on unwind, jumps to `unwind`.
    /// Same as `Invoke` but calls through a closure fat pointer.
    InvokeIndirect {
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        /// Per-argument ownership at this indirect invoke site.
        /// Parallel to `args`: `arg_ownership[i]` describes `args[i]`.
        /// Empty before annotation; populated by ownership-event realization.
        /// Unlike `Invoke`, empty defaults to all-Borrowed (conservative for
        /// unknown callees — caller retains cleanup responsibility).
        arg_ownership: Vec<ArgOwnership>,
        normal: ArcBlockId,
        unwind: ArcBlockId,
    },

    /// Resume unwinding (post-2026).
    Resume,

    /// Marks a block as unreachable (e.g., after exhaustive match).
    Unreachable,
}

// Blocks

/// A basic block in the ARC IR.
///
/// Blocks have an ID, optional parameters (for phi-like values passed
/// via `Jump` arguments), a body of sequential instructions, and a
/// terminator that transfers control.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct ArcBlock {
    /// This block's identifier.
    pub id: ArcBlockId,
    /// Block parameters — values passed from predecessor blocks via `Jump`.
    pub params: Vec<(ArcVarId, Idx)>,
    /// Sequential instructions executed in order.
    pub body: Vec<ArcInstr>,
    /// How control leaves this block.
    pub terminator: ArcTerminator,
}

impl ArcBlock {
    /// Check whether a variable is defined in this block.
    ///
    /// A variable is "defined in block" if it's a block param, an instruction
    /// destination, or an Invoke/InvokeIndirect result (which defines `dst`
    /// in the normal successor). Used by logical cleanup realization to route
    /// edge releases (currently carried by `RcDec`).
    pub fn defines_var(&self, var: ArcVarId) -> bool {
        if self.params.iter().any(|&(p, _)| p == var) {
            return true;
        }
        for instr in &self.body {
            if instr.defined_var() == Some(var) {
                return true;
            }
        }
        match &self.terminator {
            ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } => {
                *dst == var
            }
            _ => false,
        }
    }
}

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

/// Flatten an ARC function cache into a single Vec (parents + lambdas).
///
/// This is the **single canonical implementation** of the flattening
/// algorithm for the `(Name, (ArcFunction, Vec<ArcFunction>))` cache shape.
/// All consumers that need a flat list of ARC functions MUST call this helper.
///
/// The cache uses `FxHashMap` which has non-deterministic iteration order.
/// Callers that need deterministic ordering must sort the result.
#[expect(
    clippy::implicit_hasher,
    reason = "internal function always called with FxHashMap"
)]
pub fn collect_all_arc_functions(
    arc_cache: &rustc_hash::FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)>,
) -> Vec<ArcFunction> {
    arc_cache
        .values()
        .flat_map(|(parent, lambdas)| std::iter::once(parent).chain(lambdas.iter()))
        .cloned()
        .collect()
}

// Tests

#[cfg(test)]
mod tests;
