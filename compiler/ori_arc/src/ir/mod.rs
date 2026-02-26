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

mod repr;

pub use repr::{compute_var_reprs, RcStrategy, ValueRepr};

use smallvec::{smallvec, SmallVec};

use ori_ir::{BinaryOp, DurationUnit, Name, SizeUnit, Span, UnaryOp};
use ori_types::Idx;

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
    Duration { value: u64, unit: DurationUnit },
    Size { value: u64, unit: SizeUnit },
    Unit,
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
/// refined to `Borrowed` by borrow inference (Section 06.2).
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

// Instructions

/// A single instruction in an ARC IR basic block.
///
/// Instructions are executed sequentially within a block. Most produce
/// a value bound to a `dst` variable. RC operations (`RcInc`, `RcDec`)
/// are inserted by Section 07 and optimized by Section 08.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum ArcInstr {
    /// Bind a value to a variable: `let dst: ty = value`.
    Let {
        dst: ArcVarId,
        ty: Idx,
        value: ArcValue,
    },

    /// Direct function call: `let dst: ty = func(args...)`.
    Apply {
        dst: ArcVarId,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        /// Per-argument ownership at this call site.
        /// Parallel to `args`: `arg_ownership[i]` describes `args[i]`.
        /// Defaults to all `Owned`; populated by RC insertion.
        arg_ownership: Vec<ArgOwnership>,
    },

    /// Indirect call through a closure: `let dst: ty = closure(args...)`.
    ApplyIndirect {
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
    },

    /// Partial application / closure creation: `let dst: ty = func(args...)`.
    ///
    /// Creates a closure that captures `args` and awaits remaining arguments.
    PartialApply {
        dst: ArcVarId,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
    },

    /// Field projection: `let dst: ty = value.field`.
    Project {
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
    },

    /// Constructor application: `let dst: ty = ctor(args...)`.
    Construct {
        dst: ArcVarId,
        ty: Idx,
        ctor: CtorKind,
        args: Vec<ArcVarId>,
    },

    // RC operations (inserted by Section 07)
    /// Increment reference count. `count` allows batched increments
    /// when a value is passed to multiple owned parameters. `strategy`
    /// tells the emitter how to perform the increment (no Pool queries).
    RcInc {
        var: ArcVarId,
        count: u32,
        strategy: RcStrategy,
    },

    /// Decrement reference count and free if zero. `strategy` tells
    /// the emitter the cleanup approach (no Pool queries).
    RcDec { var: ArcVarId, strategy: RcStrategy },

    // Reuse operations (inserted by Section 09)
    /// Test whether a value's reference count is 1 (uniquely owned).
    /// Result is a `bool` bound to `dst`.
    IsShared { dst: ArcVarId, var: ArcVarId },

    /// In-place field update: `base.field = value`.
    /// Only valid when the object is uniquely owned.
    Set {
        base: ArcVarId,
        field: u32,
        value: ArcVarId,
    },

    /// In-place tag update for enum variants: `base.tag = tag`.
    /// Only valid when the object is uniquely owned.
    SetTag { base: ArcVarId, tag: u64 },

    /// Reset intermediate: marks a value for potential reuse.
    /// Expanded by Section 09 into `IsShared` + conditional reuse.
    Reset { var: ArcVarId, token: ArcVarId },

    /// Reuse intermediate: construct using a reuse token's memory.
    /// Expanded by Section 09 into conditional alloc-or-reuse.
    Reuse {
        token: ArcVarId,
        dst: ArcVarId,
        ty: Idx,
        ctor: CtorKind,
        args: Vec<ArcVarId>,
    },

    /// Conditional value selection: `let dst: ty = if cond then true_val else false_val`.
    ///
    /// Maps directly to LLVM `select`. Used by the decision tree emitter
    /// to eliminate trivial match arm blocks — instead of
    /// `switch → arm_block(br) → merge(phi)`, we emit
    /// `icmp + select` inline, avoiding empty blocks.
    Select {
        dst: ArcVarId,
        ty: Idx,
        cond: ArcVarId,
        true_val: ArcVarId,
        false_val: ArcVarId,
    },
}

impl ArcInstr {
    /// Returns the variable defined (written) by this instruction, if any.
    ///
    /// Value-producing instructions (`Let`, `Apply`, `ApplyIndirect`,
    /// `PartialApply`, `Project`, `Construct`, `IsShared`, `Reuse`)
    /// return `Some(dst)`. `Reset` returns `Some(token)` (the reuse token
    /// it defines). Side-effect-only instructions (`RcInc`, `RcDec`,
    /// `Set`, `SetTag`) return `None`.
    ///
    /// Used by liveness analysis (Section 07.1), RC insertion (07.2),
    /// and RC elimination (08).
    pub fn defined_var(&self) -> Option<ArcVarId> {
        match self {
            ArcInstr::Let { dst, .. }
            | ArcInstr::Apply { dst, .. }
            | ArcInstr::ApplyIndirect { dst, .. }
            | ArcInstr::PartialApply { dst, .. }
            | ArcInstr::Project { dst, .. }
            | ArcInstr::Construct { dst, .. }
            | ArcInstr::IsShared { dst, .. }
            | ArcInstr::Reuse { dst, .. }
            | ArcInstr::Select { dst, .. } => Some(*dst),

            ArcInstr::Reset { token, .. } => Some(*token),

            ArcInstr::RcInc { .. }
            | ArcInstr::RcDec { .. }
            | ArcInstr::Set { .. }
            | ArcInstr::SetTag { .. } => None,
        }
    }

    /// Returns all variables read (used) by this instruction.
    ///
    /// This collects every `ArcVarId` that appears in a "read" position —
    /// function arguments, closure targets, projected sources, RC targets,
    /// etc. The `dst` of value-producing instructions is NOT included
    /// (it's a definition, not a use).
    ///
    /// Returns `SmallVec<[ArcVarId; 4]>` to avoid heap allocation for the
    /// common case (most instructions use 0-3 variables). Called in tight
    /// inner loops by liveness, RC insertion, RC elimination, and reset/reuse.
    ///
    /// Used by liveness analysis (Section 07.1) for computing gen sets.
    pub fn used_vars(&self) -> SmallVec<[ArcVarId; 4]> {
        match self {
            ArcInstr::Let { value, .. } => match value {
                ArcValue::Var(v) => smallvec![*v],
                ArcValue::Literal(_) => SmallVec::new(),
                ArcValue::PrimOp { args, .. } => SmallVec::from_slice(args),
            },

            ArcInstr::Apply { args, .. }
            | ArcInstr::PartialApply { args, .. }
            | ArcInstr::Construct { args, .. } => SmallVec::from_slice(args),

            ArcInstr::ApplyIndirect { closure, args, .. } => {
                let mut vars = SmallVec::with_capacity(1 + args.len());
                vars.push(*closure);
                vars.extend_from_slice(args);
                vars
            }

            ArcInstr::Project { value, .. } => smallvec![*value],

            ArcInstr::RcInc { var, .. }
            | ArcInstr::RcDec { var, .. }
            | ArcInstr::IsShared { var, .. }
            | ArcInstr::Reset { var, .. } => smallvec![*var],

            ArcInstr::Set { base, value, .. } => smallvec![*base, *value],

            ArcInstr::SetTag { base, .. } => smallvec![*base],

            ArcInstr::Reuse { token, args, .. } => {
                let mut vars = SmallVec::with_capacity(1 + args.len());
                vars.push(*token);
                vars.extend_from_slice(args);
                vars
            }

            ArcInstr::Select {
                cond,
                true_val,
                false_val,
                ..
            } => smallvec![*cond, *true_val, *false_val],
        }
    }

    /// Check whether this instruction reads (uses) a specific variable.
    ///
    /// Zero-allocation alternative to `used_vars().contains(&var)`. Matches
    /// directly on instruction fields and short-circuits on the first hit.
    /// Used by reset/reuse detection (Section 07.6) and RC elimination (08)
    /// in inner loops where allocation per check is wasteful.
    pub fn uses_var(&self, target: ArcVarId) -> bool {
        match self {
            ArcInstr::Let { value, .. } => match value {
                ArcValue::Var(v) => *v == target,
                ArcValue::Literal(_) => false,
                ArcValue::PrimOp { args, .. } => args.contains(&target),
            },

            ArcInstr::Apply { args, .. }
            | ArcInstr::PartialApply { args, .. }
            | ArcInstr::Construct { args, .. } => args.contains(&target),

            ArcInstr::ApplyIndirect { closure, args, .. } => {
                *closure == target || args.contains(&target)
            }

            ArcInstr::Project { value, .. } => *value == target,

            ArcInstr::RcInc { var, .. }
            | ArcInstr::RcDec { var, .. }
            | ArcInstr::IsShared { var, .. }
            | ArcInstr::Reset { var, .. } => *var == target,

            ArcInstr::Set { base, value, .. } => *base == target || *value == target,

            ArcInstr::SetTag { base, .. } => *base == target,

            ArcInstr::Reuse { token, args, .. } => *token == target || args.contains(&target),

            ArcInstr::Select {
                cond,
                true_val,
                false_val,
                ..
            } => *cond == target || *true_val == target || *false_val == target,
        }
    }

    /// Check whether an argument position is "owned" — i.e., the value at
    /// that index in [`used_vars()`](Self::used_vars) will be stored on the
    /// heap or consumed by the callee.
    ///
    /// Borrowed-derived variables flowing into an owned position need an
    /// `RcInc` to transfer ownership. Positions are indices into `used_vars()`.
    ///
    /// Owned positions:
    /// - `Construct`, `PartialApply`: all args (`0..args.len()`)
    /// - `Apply`: args where `arg_ownership[pos] == Owned` (respects borrow inference)
    /// - `ApplyIndirect`: closure + all args (`0..=args.len()`)
    /// - Everything else: no owned positions (read-only uses)
    pub fn is_owned_position(&self, pos: usize) -> bool {
        match self {
            ArcInstr::Construct { args, .. } | ArcInstr::PartialApply { args, .. } => {
                pos < args.len()
            }
            ArcInstr::Apply {
                args,
                arg_ownership,
                ..
            } => {
                pos < args.len()
                    && arg_ownership
                        .get(pos)
                        .is_none_or(|o| *o == ArgOwnership::Owned)
            }
            ArcInstr::ApplyIndirect { args, .. } => pos <= args.len(),
            _ => false,
        }
    }

    /// Replace all occurrences of `old` with `new` in read positions.
    ///
    /// Defined variables (`dst`) are NOT substituted — only used variables.
    /// Used by constructor reuse expansion (Section 09) to substitute
    /// `reuse_dst → reset_var` on the fast path.
    pub fn substitute_var(&mut self, old: ArcVarId, new: ArcVarId) {
        fn sub(v: &mut ArcVarId, old: ArcVarId, new: ArcVarId) {
            if *v == old {
                *v = new;
            }
        }
        fn sub_args(args: &mut [ArcVarId], old: ArcVarId, new: ArcVarId) {
            for a in args {
                sub(a, old, new);
            }
        }
        match self {
            ArcInstr::Let { value, .. } => match value {
                ArcValue::Var(v) => sub(v, old, new),
                ArcValue::Literal(_) => {}
                ArcValue::PrimOp { args, .. } => sub_args(args, old, new),
            },
            ArcInstr::Apply { args, .. }
            | ArcInstr::PartialApply { args, .. }
            | ArcInstr::Construct { args, .. } => sub_args(args, old, new),
            ArcInstr::ApplyIndirect { closure, args, .. } => {
                sub(closure, old, new);
                sub_args(args, old, new);
            }
            ArcInstr::Project { value, .. } => sub(value, old, new),
            ArcInstr::RcInc { var, .. }
            | ArcInstr::RcDec { var, .. }
            | ArcInstr::IsShared { var, .. }
            | ArcInstr::Reset { var, .. } => sub(var, old, new),
            ArcInstr::Set { base, value, .. } => {
                sub(base, old, new);
                sub(value, old, new);
            }
            ArcInstr::SetTag { base, .. } => sub(base, old, new),
            ArcInstr::Reuse { token, args, .. } => {
                sub(token, old, new);
                sub_args(args, old, new);
            }
            ArcInstr::Select {
                cond,
                true_val,
                false_val,
                ..
            } => {
                sub(cond, old, new);
                sub(true_val, old, new);
                sub(false_val, old, new);
            }
        }
    }
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

    /// Call that may unwind (post-0.1-alpha, for panic/effect support).
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

    /// Resume unwinding (post-0.1-alpha).
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
            ArcTerminator::Branch { cond, .. } => *cond == target,
            ArcTerminator::Switch { scrutinee, .. } => *scrutinee == target,
            ArcTerminator::Resume | ArcTerminator::Unreachable => false,
        }
    }

    /// Replace all occurrences of `old` with `new` in variable positions.
    ///
    /// Used by constructor reuse expansion (Section 09) to substitute
    /// `reuse_dst → reset_var` on the fast path, where the result IS
    /// the original object.
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
}

impl ArcFunction {
    /// Look up the type of a variable.
    ///
    /// # Panics
    ///
    /// Debug-panics if `var` is out of bounds.
    #[inline]
    pub fn var_type(&self, var: ArcVarId) -> Idx {
        debug_assert!(
            var.index() < self.var_types.len(),
            "ArcVarId {} out of bounds (have {} vars)",
            var.raw(),
            self.var_types.len(),
        );
        self.var_types[var.index()]
    }

    /// Look up the machine representation of a variable.
    ///
    /// Only valid after [`compute_var_reprs`] has been called (i.e., during
    /// or after the ARC pipeline). Returns `None` if `var_reprs` is empty
    /// (pre-pipeline).
    #[inline]
    pub fn var_repr(&self, var: ArcVarId) -> Option<ValueRepr> {
        if self.var_reprs.is_empty() {
            return None;
        }
        debug_assert!(
            var.index() < self.var_reprs.len(),
            "ArcVarId {} out of bounds for var_reprs (have {} entries)",
            var.raw(),
            self.var_reprs.len(),
        );
        Some(self.var_reprs[var.index()])
    }

    /// Allocate a fresh variable with the given type.
    ///
    /// Returns a new [`ArcVarId`] that does not collide with any existing
    /// variable in this function. The variable's type is recorded in
    /// [`var_types`](Self::var_types).
    ///
    /// Used by ARC passes that introduce synthetic variables (e.g., the
    /// `IsShared` result in constructor reuse expansion, reuse tokens in
    /// reset/reuse detection).
    pub fn fresh_var(&mut self, ty: Idx) -> ArcVarId {
        let id = u32::try_from(self.var_types.len())
            .unwrap_or_else(|_| panic!("variable count exceeds u32::MAX"));
        self.var_types.push(ty);
        // Keep var_reprs in sync if it has been populated.
        // Uses Scalar as a placeholder — callers that know the correct repr
        // should use `fresh_var_repr` instead.
        if !self.var_reprs.is_empty() {
            self.var_reprs.push(ValueRepr::Scalar);
        }
        ArcVarId::new(id)
    }

    /// Allocate a fresh variable with an explicit [`ValueRepr`].
    ///
    /// Like [`fresh_var`](Self::fresh_var), but stores the provided `repr`
    /// instead of a `Scalar` placeholder. Used by ARC passes that create
    /// variables of known representation (e.g., reuse tokens, merge params).
    pub fn fresh_var_repr(&mut self, ty: Idx, repr: ValueRepr) -> ArcVarId {
        let id = u32::try_from(self.var_types.len())
            .unwrap_or_else(|_| panic!("variable count exceeds u32::MAX"));
        self.var_types.push(ty);
        if !self.var_reprs.is_empty() {
            self.var_reprs.push(repr);
        }
        ArcVarId::new(id)
    }

    /// Append a new basic block to this function.
    ///
    /// The block's `id` must equal the next sequential block index
    /// (`self.blocks.len()`). Span entries are initialized to `None` for
    /// each instruction in the block body.
    ///
    /// # Panics
    ///
    /// Debug-panics if `block.id` does not match the expected index.
    pub fn push_block(&mut self, block: ArcBlock) {
        let expected = ArcBlockId::new(
            u32::try_from(self.blocks.len())
                .unwrap_or_else(|_| panic!("block count exceeds u32::MAX")),
        );
        debug_assert_eq!(
            block.id,
            expected,
            "block ID {} does not match expected index {}",
            block.id.raw(),
            expected.raw(),
        );
        self.spans.push(vec![None; block.body.len()]);
        self.blocks.push(block);
    }

    /// Return the [`ArcBlockId`] that the next [`push_block`](Self::push_block)
    /// call will use.
    pub fn next_block_id(&self) -> ArcBlockId {
        ArcBlockId::new(
            u32::try_from(self.blocks.len())
                .unwrap_or_else(|_| panic!("block count exceeds u32::MAX")),
        )
    }
}

// Tests

#[cfg(test)]
mod tests;
