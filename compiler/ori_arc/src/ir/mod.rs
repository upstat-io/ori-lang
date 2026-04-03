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

mod function;
mod instr;
mod repr;

pub use instr::ArcInstr;
pub use repr::{compute_var_reprs, RcStrategy, ValueRepr};

use smallvec::{smallvec, SmallVec};

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
    /// Used by the TRMC rewrite (Section 13.4) to fill constructor hole fields.
    /// The variable carrying this value has the field's declared type, but the
    /// runtime value is null (zero). RC operations on null are no-ops in the
    /// runtime (`ori_rc_inc`/`ori_buffer_rc_dec` check for null).
    ///
    /// # Contract
    ///
    /// A `Null` literal **must** be consumed by a `Construct` instruction whose
    /// corresponding field is overwritten by `Set` before any read of that field.
    /// The post-rewrite verifier (Section 13.5) enforces this invariant.
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
        normal: ArcBlockId,
        unwind: ArcBlockId,
    },

    /// Resume unwinding (post-2026).
    Resume,

    /// Marks a block as unreachable (e.g., after exhaustive match).
    Unreachable,
}

impl ArcTerminator {
    /// Returns all variables read (used) by this terminator.
    ///
    /// - `Return` uses the returned value.
    /// - `Jump` uses its arguments (passed to the target block's params).
    /// - `Branch` uses the condition variable.
    /// - `Switch` uses the scrutinee.
    /// - `Invoke` uses its arguments (the `dst` is a definition in the
    ///   normal successor, not a use here).
    /// - `Resume` / `Unreachable` use nothing.
    ///
    /// Returns `SmallVec<[ArcVarId; 4]>` to avoid heap allocation for the
    /// common case (max 1-3 variables per terminator, except large Jump/Invoke).
    pub fn used_vars(&self) -> SmallVec<[ArcVarId; 4]> {
        match self {
            ArcTerminator::Return { value } => smallvec![*value],
            ArcTerminator::Jump { args, .. } | ArcTerminator::Invoke { args, .. } => {
                SmallVec::from_slice(args)
            }
            ArcTerminator::InvokeIndirect { closure, args, .. } => {
                let mut vars = SmallVec::from_slice(args);
                vars.push(*closure);
                vars
            }
            ArcTerminator::Branch { cond, .. } => smallvec![*cond],
            ArcTerminator::Switch { scrutinee, .. } => smallvec![*scrutinee],
            ArcTerminator::Resume | ArcTerminator::Unreachable => SmallVec::new(),
        }
    }

    /// Check whether this terminator reads (uses) a specific variable.
    ///
    /// Zero-allocation alternative to `used_vars().contains(&var)`.
    pub fn uses_var(&self, target: ArcVarId) -> bool {
        match self {
            ArcTerminator::Return { value } => *value == target,
            ArcTerminator::Jump { args, .. } | ArcTerminator::Invoke { args, .. } => {
                args.contains(&target)
            }
            ArcTerminator::InvokeIndirect { closure, args, .. } => {
                *closure == target || args.contains(&target)
            }
            ArcTerminator::Branch { cond, .. } => *cond == target,
            ArcTerminator::Switch { scrutinee, .. } => *scrutinee == target,
            ArcTerminator::Resume | ArcTerminator::Unreachable => false,
        }
    }

    /// Replace all occurrences of `old` with `new` in variable positions.
    ///
    /// Used by constructor reuse expansion to substitute `reuse_dst → reset_var`
    /// on the fast path, where the result IS the original object.
    pub fn substitute_var(&mut self, old: ArcVarId, new: ArcVarId) {
        fn sub(v: &mut ArcVarId, old: ArcVarId, new: ArcVarId) {
            if *v == old {
                *v = new;
            }
        }
        match self {
            ArcTerminator::Return { value } => sub(value, old, new),
            ArcTerminator::Jump { args, .. } | ArcTerminator::Invoke { args, .. } => {
                for a in args {
                    sub(a, old, new);
                }
            }
            ArcTerminator::InvokeIndirect { closure, args, .. } => {
                sub(closure, old, new);
                for a in args {
                    sub(a, old, new);
                }
            }
            ArcTerminator::Branch { cond, .. } => sub(cond, old, new),
            ArcTerminator::Switch { scrutinee, .. } => sub(scrutinee, old, new),
            ArcTerminator::Resume | ArcTerminator::Unreachable => {}
        }
    }
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
    /// rewrite pass (§09.2). Skipped during cache serialization.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub tail_calls: Vec<crate::tail_call::TailCallSite>,
}

// Tests

#[cfg(test)]
mod tests;
