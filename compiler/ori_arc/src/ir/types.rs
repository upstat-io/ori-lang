//! Core ARC IR identifiers, values, blocks, and terminators.
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

use std::num::NonZeroU32;

use ori_ir::{BinaryOp, DurationUnit, Name, SizeUnit, UnaryOp};
use ori_types::Idx;

use crate::Ownership;

use super::ArcInstr;

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

#[inline]
fn encode_arc_id(raw: u32) -> NonZeroU32 {
    if raw == u32::MAX {
        return NonZeroU32::MAX;
    }
    assert!(
        raw < u32::MAX - 1,
        "ARC ID table exceeded niche-encoded u32 capacity"
    );
    let Some(encoded) = NonZeroU32::new(raw + 1) else {
        unreachable!("adding one to a valid ARC ID produced zero");
    };
    encoded
}

#[inline]
fn decode_arc_id(encoded: NonZeroU32) -> u32 {
    if encoded == NonZeroU32::MAX {
        u32::MAX
    } else {
        encoded.get() - 1
    }
}

/// Variable ID within an ARC IR function.
///
/// Each `ArcVarId` identifies a unique SSA-like value within a single
/// [`ArcFunction`]. IDs are allocated sequentially starting from 0.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct ArcVarId(NonZeroU32);

impl ArcVarId {
    /// Sentinel value representing an invalid or uninitialized variable.
    ///
    /// Equal to `u32::MAX`. Functions that return `ArcVarId` should never
    /// produce this value; its presence indicates a lowering bug.
    pub const INVALID: Self = Self(NonZeroU32::MAX);

    /// Create a new variable ID from a raw index.
    #[inline]
    pub fn new(raw: u32) -> Self {
        Self(encode_arc_id(raw))
    }

    /// Get the raw `u32` value.
    #[inline]
    pub fn raw(self) -> u32 {
        decode_arc_id(self.0)
    }

    /// Get the index as `usize` (for indexing into `Vec`s).
    #[inline]
    pub fn index(self) -> usize {
        self.raw() as usize
    }

    /// Returns `true` if this is a valid (non-sentinel) variable ID.
    #[inline]
    pub fn is_valid(self) -> bool {
        self != Self::INVALID
    }
}

impl std::fmt::Debug for ArcVarId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("ArcVarId")
            .field(&self.raw())
            .finish()
    }
}

/// Basic block ID within an ARC IR function.
///
/// Each `ArcBlockId` identifies a basic block within a single
/// [`ArcFunction`]. IDs are allocated sequentially starting from 0.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct ArcBlockId(NonZeroU32);

impl ArcBlockId {
    /// Sentinel value representing an invalid or uninitialized block.
    ///
    /// Equal to `u32::MAX`. Functions that return `ArcBlockId` should never
    /// produce this value; its presence indicates a lowering bug.
    pub const INVALID: Self = Self(NonZeroU32::MAX);

    /// Create a new block ID from a raw index.
    #[inline]
    pub fn new(raw: u32) -> Self {
        Self(encode_arc_id(raw))
    }

    /// Get the raw `u32` value.
    #[inline]
    pub fn raw(self) -> u32 {
        decode_arc_id(self.0)
    }

    /// Get the index as `usize` (for indexing into `Vec`s).
    #[inline]
    pub fn index(self) -> usize {
        self.raw() as usize
    }

    /// Returns `true` if this is a valid (non-sentinel) block ID.
    #[inline]
    pub fn is_valid(self) -> bool {
        self != Self::INVALID
    }
}

impl std::fmt::Debug for ArcBlockId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("ArcBlockId")
            .field(&self.raw())
            .finish()
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
    /// that `Set` overwrites before any read.
    ///
    /// TRMC constructor holes carry this placeholder until field initialization.
    /// The variable carrying this value has the field's declared type, but the
    /// physical value is null (zero) and carries no logical ownership event.
    /// The shipped counter-backed runtime preserves that rule by making its
    /// transitional carrier helpers (`ori_rc_inc`/`ori_buffer_rc_dec`) no-ops
    /// on null; other physical plans must preserve the same hole semantics.
    ///
    /// # Contract
    ///
    /// A `Null` literal **must** be consumed by a `Construct` instruction whose
    /// corresponding field is overwritten by `Set` before that field is read.
    /// The post-rewrite verifier enforces this invariant.
    Null,
}

// Primitive operations

/// Primitive operation backed by the canonical `ori_ir` operator types.
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
