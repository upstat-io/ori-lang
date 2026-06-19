//! ARC IR — basic-block intermediate representation for ARC analysis.
//!
//! All ARC analysis passes (borrow inference, RC insertion, RC elimination,
//! constructor reuse) operate on this IR. It is lowered from the typed AST
//! and then transformed in-place by each pass.
//!
//! # Architecture
//!
//! The ARC IR follows the same basic-block structure as LLVM IR, Lean 4's
//! LCNF, and Rust's MIR:
//!
//! - **[`ArcFunction`]** — a function body: parameters, blocks, variable types
//! - **[`ArcBlock`]** — a basic block: parameters, body instructions, terminator
//! - **[`ArcInstr`]** — a single instruction (let-binding, call, construct, RC op)
//! - **[`ArcTerminator`]** — block exit (return, jump, branch, switch)
//!
//! Values are named via [`ArcVarId`] (SSA-like). Control flow uses
//! [`ArcBlockId`] references between blocks.

pub mod format;
mod function;
mod instr;
mod repr;
mod terminator;
pub mod validate;

pub use instr::ArcInstr;
pub use repr::{
    compute_var_rc_strategies, compute_var_reprs, is_transitive_drop_strategy, RcAtomicity,
    RcStrategy, ValueRepr,
};

use ori_ir::{BinaryOp, DurationUnit, Name, SizeUnit, Span, UnaryOp};
use ori_types::Idx;

use crate::uniqueness::{CowAnnotations, DropHints};
use crate::Ownership;

// Call-site argument ownership

/// Per-argument ownership at a call site.
///
/// Embedded directly in [`ArcInstr::Apply`] and [`ArcTerminator::Invoke`]
/// so downstream passes can read ownership without querying external data
/// (annotated signatures, string interner). Populated by
/// [`compute_arg_ownership`](crate::rc_insert) during RC insertion.
///
/// Mirrors the two-state semantics of [`Ownership`] but scoped to a
/// specific call instruction rather than a function signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum ArgOwnership {
    /// Callee consumes: ownership transfers. Caller emits `RcInc` if live-after.
    Owned,
    /// Callee borrows: reads without consuming. Caller must `RcDec` at last use.
    Borrowed,
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
    /// runtime value is null (zero). RC operations on null are no-ops in the
    /// runtime (`ori_rc_inc`/`ori_buffer_rc_dec` check for null).
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
        /// Defaults to all `Owned`; populated by RC insertion.
        arg_ownership: Vec<ArgOwnership>,
        /// Abstract dispatch index for generic-instantiated calls.
        /// Mirrors the `mono_instance_id` slot on `ArcInstr::Apply`;
        /// `Invoke` is the may-unwind sibling carrier produced by
        /// `lower::calls::emit_call_or_invoke` when `is_nounwind_call`
        /// returns false. Sourced from `CanonResult.mono_dispatch_map_can`
        /// during ARC lowering; consumed downstream by `ori_llvm` and
        /// `ori_eval` to look up `TypedModule.mono_instances[id.0]`.
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
        /// Empty before annotation; populated by RC insertion.
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
    /// in the normal successor). Used by RC emission to route edge decs.
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
    /// Machine-level representation of each variable, indexed by `ArcVarId::index()`.
    ///
    /// Computed by [`compute_var_reprs`] at the start of the ARC pipeline.
    /// Empty until then (lowering produces an empty vec; the pipeline fills it).
    ///
    /// Skipped during cache serialization — derived from `var_types` + Pool,
    /// not an independent data source.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub var_reprs: Vec<ValueRepr>,
    /// Cached RC strategy for each variable, indexed by `ArcVarId::index()`.
    /// `None` for scalar variables (no RC ops). Computed alongside
    /// [`var_reprs`](Self::var_reprs) at the start of the ARC pipeline so
    /// downstream pre-walk passes (notably the AIMS PIN-6 `class_payload_of`
    /// population in `intraprocedural::ssa_alias_classes`) can classify a
    /// var's strategy without holding a `&Pool` reference.
    ///
    /// Empty until populated by the pipeline alongside `var_reprs`.
    /// Skipped during cache serialization for the same reason as
    /// `var_reprs` — derived from `var_types` + `var_reprs` + Pool, not an
    /// independent data source.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub var_rc_strategies: Vec<Option<RcStrategy>>,
    /// Source spans for instructions, indexed by `[block_index][instr_index]`.
    /// `None` for synthetic instructions (e.g., inserted RC operations).
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
    /// Used by the LLVM backend to detect non-capturing lambdas and
    /// skip trampoline wrapper generation.
    #[cfg_attr(feature = "cache", serde(default))]
    pub num_captures: usize,
    /// Per-instruction COW mode annotations from uniqueness analysis.
    ///
    /// Maps `(block_index, instr_index)` to [`CowMode`] for each COW
    /// operation. The LLVM arc emitter queries this to decide whether to
    /// emit only the fast path (`StaticUnique`), only the slow path
    /// (`StaticShared`), or the full runtime check (`Dynamic`).
    ///
    /// Populated by the ARC pipeline (after uniqueness analysis). Empty
    /// until then. Skipped during cache serialization — derived from the
    /// analysis, not an independent data source.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub cow_annotations: CowAnnotations,
    /// Per-instruction drop hints for unique collection drops.
    ///
    /// Identifies `RcDec` instructions where the target collection is
    /// provably unique (RC == 1), allowing the LLVM emitter to call
    /// `ori_buffer_drop_unique` instead of `ori_buffer_rc_dec`.
    ///
    /// Computed at the end of the ARC pipeline (after RC elimination).
    /// Skipped during cache serialization — derived data.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub drop_hints: DropHints,
    /// Detected self-recursive tail call sites.
    ///
    /// Each entry identifies the block and instruction index of an `Apply`
    /// in tail position. Populated by [`tail_call::detect_tail_calls`] in the
    /// ARC pipeline (after RC elimination). Consumed by the loop-lowering
    /// rewrite pass. Skipped during cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub tail_calls: Vec<crate::tail_call::TailCallSite>,
    /// Variables for which `emit_burden_ops` (`lower/burden_lower.rs`) has
    /// emitted at least one `BurdenInc` / `BurdenDec` / `BurdenDecPartial` /
    /// `BurdenDecField` / `BurdenDecVariant` instruction.
    ///
    /// Indexed by `ArcVarId::index()`. `true` = burden walker owns this var's
    /// RC traffic; the predicate-stack realization should defer to the burden
    /// walk for vars in this set when their containing SSA-alias class is
    /// fully covered (`AimsStateMap::class_covered`).
    ///
    /// Default: empty. Populated by `emit_burden_ops`. Read by the AIMS
    /// post-convergence `class_covered` computation and by `decide()` at
    /// realization. Skipped during cache serialization — derived data.
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
    /// - lets the LLVM emitter materialize a landing pad at each handler and
    ///   route ONLY a mapped checked-op's panic through `invoke` to ITS handler
    ///   (per-dispatch + per-handler scoping — a subsequent uncaught `1 / 0`,
    ///   or a checked-op caught by a different nested catch, routes correctly).
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
