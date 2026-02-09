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

use ori_ir::{BinaryOp, DurationUnit, Name, SizeUnit, Span, UnaryOp};
use ori_types::Idx;

use crate::Ownership;

// ── ID newtypes ─────────────────────────────────────────────────────

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

// ── Literal values ──────────────────────────────────────────────────

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

// ── Primitive operations ────────────────────────────────────────────

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

// ── Values ──────────────────────────────────────────────────────────

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

// ── Constructor kinds ───────────────────────────────────────────────

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

// ── Parameters ──────────────────────────────────────────────────────

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

// ── Instructions ────────────────────────────────────────────────────

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

    // ── RC operations (inserted by Section 07) ──────────────────
    /// Increment reference count. `count` allows batched increments
    /// when a value is passed to multiple owned parameters.
    RcInc { var: ArcVarId, count: u32 },

    /// Decrement reference count and free if zero.
    RcDec { var: ArcVarId },

    // ── Reuse operations (inserted by Section 09) ───────────────
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
            | ArcInstr::Reuse { dst, .. } => Some(*dst),

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
    /// Used by liveness analysis (Section 07.1) for computing gen sets.
    pub fn used_vars(&self) -> Vec<ArcVarId> {
        match self {
            ArcInstr::Let { value, .. } => match value {
                ArcValue::Var(v) => vec![*v],
                ArcValue::Literal(_) => vec![],
                ArcValue::PrimOp { args, .. } => args.clone(),
            },

            ArcInstr::Apply { args, .. }
            | ArcInstr::PartialApply { args, .. }
            | ArcInstr::Construct { args, .. } => args.clone(),

            ArcInstr::ApplyIndirect { closure, args, .. } => {
                let mut vars = Vec::with_capacity(1 + args.len());
                vars.push(*closure);
                vars.extend_from_slice(args);
                vars
            }

            ArcInstr::Project { value, .. } => vec![*value],

            ArcInstr::RcInc { var, .. }
            | ArcInstr::RcDec { var }
            | ArcInstr::IsShared { var, .. }
            | ArcInstr::Reset { var, .. } => vec![*var],

            ArcInstr::Set { base, value, .. } => vec![*base, *value],

            ArcInstr::SetTag { base, .. } => vec![*base],

            ArcInstr::Reuse { token, args, .. } => {
                let mut vars = Vec::with_capacity(1 + args.len());
                vars.push(*token);
                vars.extend_from_slice(args);
                vars
            }
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
            | ArcInstr::RcDec { var }
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
        }
    }
}

// ── Terminators ─────────────────────────────────────────────────────

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
    pub fn used_vars(&self) -> Vec<ArcVarId> {
        match self {
            ArcTerminator::Return { value } => vec![*value],
            ArcTerminator::Jump { args, .. } | ArcTerminator::Invoke { args, .. } => args.clone(),
            ArcTerminator::Branch { cond, .. } => vec![*cond],
            ArcTerminator::Switch { scrutinee, .. } => vec![*scrutinee],
            ArcTerminator::Resume | ArcTerminator::Unreachable => vec![],
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

// ── Blocks ──────────────────────────────────────────────────────────

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

// ── Functions ───────────────────────────────────────────────────────

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
    /// Source spans for instructions, indexed by `[block_index][instr_index]`.
    /// `None` for synthetic instructions (e.g., inserted RC operations).
    ///
    /// Skipped during cache serialization — spans are source metadata not needed
    /// for cached codegen. Deserialized functions get empty span vectors.
    #[cfg_attr(feature = "cache", serde(skip))]
    pub spans: Vec<Vec<Option<Span>>>,
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

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::mem;

    use ori_ir::{BinaryOp, Name, UnaryOp};
    use ori_types::Idx;

    use crate::Ownership;

    use super::*;

    // ── ID newtypes ─────────────────────────────────────────────

    #[test]
    fn arc_var_id_basics() {
        let v = ArcVarId::new(42);
        assert_eq!(v.raw(), 42);
        assert_eq!(v.index(), 42);
    }

    #[test]
    fn arc_block_id_basics() {
        let b = ArcBlockId::new(7);
        assert_eq!(b.raw(), 7);
        assert_eq!(b.index(), 7);
    }

    #[test]
    fn arc_var_id_equality() {
        assert_eq!(ArcVarId::new(0), ArcVarId::new(0));
        assert_ne!(ArcVarId::new(0), ArcVarId::new(1));
    }

    #[test]
    fn arc_block_id_equality() {
        assert_eq!(ArcBlockId::new(0), ArcBlockId::new(0));
        assert_ne!(ArcBlockId::new(0), ArcBlockId::new(1));
    }

    #[test]
    fn arc_var_id_ordering() {
        assert!(ArcVarId::new(0) < ArcVarId::new(1));
        assert!(ArcVarId::new(5) > ArcVarId::new(3));
    }

    #[test]
    fn id_sizes() {
        assert_eq!(mem::size_of::<ArcVarId>(), 4);
        assert_eq!(mem::size_of::<ArcBlockId>(), 4);
    }

    // ── LitValue ────────────────────────────────────────────────

    #[test]
    fn lit_value_int() {
        let v = LitValue::Int(42);
        assert_eq!(v, LitValue::Int(42));
        assert_ne!(v, LitValue::Int(43));
    }

    #[test]
    fn lit_value_bool() {
        assert_ne!(LitValue::Bool(true), LitValue::Bool(false));
    }

    #[test]
    fn lit_value_unit() {
        assert_eq!(LitValue::Unit, LitValue::Unit);
    }

    #[test]
    fn lit_value_string() {
        let s = LitValue::String(Name::from_raw(100));
        assert_eq!(s, LitValue::String(Name::from_raw(100)));
    }

    #[test]
    fn lit_value_duration() {
        let d = LitValue::Duration {
            value: 500,
            unit: ori_ir::DurationUnit::Milliseconds,
        };
        assert_eq!(
            d,
            LitValue::Duration {
                value: 500,
                unit: ori_ir::DurationUnit::Milliseconds,
            }
        );
    }

    #[test]
    fn lit_value_size() {
        let s = LitValue::Size {
            value: 1024,
            unit: ori_ir::SizeUnit::Kilobytes,
        };
        assert_eq!(
            s,
            LitValue::Size {
                value: 1024,
                unit: ori_ir::SizeUnit::Kilobytes,
            }
        );
    }

    // ── PrimOp ──────────────────────────────────────────────────

    #[test]
    fn prim_op_binary() {
        let op = PrimOp::Binary(BinaryOp::Add);
        assert_eq!(op, PrimOp::Binary(BinaryOp::Add));
        assert_ne!(op, PrimOp::Binary(BinaryOp::Sub));
    }

    #[test]
    fn prim_op_unary() {
        let op = PrimOp::Unary(UnaryOp::Neg);
        assert_eq!(op, PrimOp::Unary(UnaryOp::Neg));
        assert_ne!(op, PrimOp::Unary(UnaryOp::Not));
    }

    #[test]
    fn prim_op_binary_vs_unary() {
        assert_ne!(PrimOp::Binary(BinaryOp::Add), PrimOp::Unary(UnaryOp::Neg),);
    }

    // ── ArcValue ────────────────────────────────────────────────

    #[test]
    fn arc_value_var() {
        let v = ArcValue::Var(ArcVarId::new(0));
        assert_eq!(v, ArcValue::Var(ArcVarId::new(0)));
    }

    #[test]
    fn arc_value_literal() {
        let v = ArcValue::Literal(LitValue::Int(99));
        assert_eq!(v, ArcValue::Literal(LitValue::Int(99)));
    }

    #[test]
    fn arc_value_prim_op() {
        let v = ArcValue::PrimOp {
            op: PrimOp::Binary(BinaryOp::Add),
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        };
        assert!(matches!(v, ArcValue::PrimOp { .. }));
    }

    // ── CtorKind ────────────────────────────────────────────────

    #[test]
    fn ctor_kind_struct() {
        let c = CtorKind::Struct(Name::from_raw(1));
        assert_eq!(c, CtorKind::Struct(Name::from_raw(1)));
    }

    #[test]
    fn ctor_kind_enum_variant() {
        let c = CtorKind::EnumVariant {
            enum_name: Name::from_raw(2),
            variant: 0,
        };
        assert!(matches!(c, CtorKind::EnumVariant { variant: 0, .. }));
    }

    #[test]
    fn ctor_kind_collection_literals() {
        // All three collection literal kinds are distinct.
        assert_ne!(CtorKind::ListLiteral, CtorKind::MapLiteral);
        assert_ne!(CtorKind::MapLiteral, CtorKind::SetLiteral);
        assert_ne!(CtorKind::ListLiteral, CtorKind::SetLiteral);
    }

    // ── ArcParam ────────────────────────────────────────────────

    #[test]
    fn arc_param_borrowed() {
        let p = ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Borrowed,
        };
        assert_eq!(p.ownership, Ownership::Borrowed);
    }

    #[test]
    fn arc_param_owned() {
        let p = ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        };
        assert_eq!(p.ownership, Ownership::Owned);
    }

    // ── ArcInstr ────────────────────────────────────────────────

    #[test]
    fn instr_let() {
        let instr = ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(42)),
        };
        assert!(matches!(instr, ArcInstr::Let { .. }));
    }

    #[test]
    fn instr_apply() {
        let instr = ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: Name::from_raw(10),
            args: vec![ArcVarId::new(0)],
        };
        assert!(matches!(instr, ArcInstr::Apply { .. }));
    }

    #[test]
    fn instr_construct() {
        let instr = ArcInstr::Construct {
            dst: ArcVarId::new(2),
            ty: Idx::UNIT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        };
        if let ArcInstr::Construct { ctor, args, .. } = &instr {
            assert_eq!(*ctor, CtorKind::Tuple);
            assert_eq!(args.len(), 2);
        } else {
            panic!("expected Construct");
        }
    }

    #[test]
    fn instr_project() {
        let instr = ArcInstr::Project {
            dst: ArcVarId::new(3),
            ty: Idx::INT,
            value: ArcVarId::new(2),
            field: 0,
        };
        if let ArcInstr::Project { field, .. } = &instr {
            assert_eq!(*field, 0);
        } else {
            panic!("expected Project");
        }
    }

    #[test]
    fn instr_rc_ops() {
        let inc = ArcInstr::RcInc {
            var: ArcVarId::new(0),
            count: 2,
        };
        let dec = ArcInstr::RcDec {
            var: ArcVarId::new(0),
        };
        assert!(matches!(inc, ArcInstr::RcInc { count: 2, .. }));
        assert!(matches!(dec, ArcInstr::RcDec { .. }));
    }

    #[test]
    fn instr_apply_indirect() {
        let instr = ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(5),
            ty: Idx::INT,
            closure: ArcVarId::new(4),
            args: vec![ArcVarId::new(0)],
        };
        if let ArcInstr::ApplyIndirect { closure, .. } = &instr {
            assert_eq!(*closure, ArcVarId::new(4));
        } else {
            panic!("expected ApplyIndirect");
        }
    }

    #[test]
    fn instr_partial_apply() {
        let instr = ArcInstr::PartialApply {
            dst: ArcVarId::new(6),
            ty: Idx::UNIT,
            func: Name::from_raw(20),
            args: vec![ArcVarId::new(0)],
        };
        assert!(matches!(instr, ArcInstr::PartialApply { .. }));
    }

    // ── ArcTerminator ───────────────────────────────────────────

    #[test]
    fn terminator_return() {
        let t = ArcTerminator::Return {
            value: ArcVarId::new(0),
        };
        assert!(matches!(t, ArcTerminator::Return { .. }));
    }

    #[test]
    fn terminator_jump() {
        let t = ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![ArcVarId::new(0)],
        };
        if let ArcTerminator::Jump { target, args } = &t {
            assert_eq!(*target, ArcBlockId::new(1));
            assert_eq!(args.len(), 1);
        } else {
            panic!("expected Jump");
        }
    }

    #[test]
    fn terminator_branch() {
        let t = ArcTerminator::Branch {
            cond: ArcVarId::new(0),
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        };
        if let ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } = &t
        {
            assert_ne!(then_block, else_block);
        } else {
            panic!("expected Branch");
        }
    }

    #[test]
    fn terminator_switch() {
        let t = ArcTerminator::Switch {
            scrutinee: ArcVarId::new(0),
            cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
            default: ArcBlockId::new(3),
        };
        if let ArcTerminator::Switch { cases, default, .. } = &t {
            assert_eq!(cases.len(), 2);
            assert_eq!(*default, ArcBlockId::new(3));
        } else {
            panic!("expected Switch");
        }
    }

    #[test]
    fn terminator_unreachable() {
        let t = ArcTerminator::Unreachable;
        assert!(matches!(t, ArcTerminator::Unreachable));
    }

    // ── ArcBlock ────────────────────────────────────────────────

    #[test]
    fn arc_block_construction() {
        let block = ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(2)),
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        };
        assert_eq!(block.id, ArcBlockId::new(0));
        assert_eq!(block.body.len(), 2);
        assert!(block.params.is_empty());
    }

    #[test]
    fn arc_block_with_params() {
        let block = ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![(ArcVarId::new(10), Idx::INT), (ArcVarId::new(11), Idx::STR)],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(10),
            },
        };
        assert_eq!(block.params.len(), 2);
        assert_eq!(block.params[0].0, ArcVarId::new(10));
        assert_eq!(block.params[1].1, Idx::STR);
    }

    // ── ArcFunction ─────────────────────────────────────────────

    #[test]
    fn arc_function_var_type_single() {
        let func = ArcFunction {
            name: Name::from_raw(1),
            params: vec![ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            }],
            return_type: Idx::INT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(0),
                },
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::INT],
            spans: vec![vec![]],
        };
        assert_eq!(func.var_type(ArcVarId::new(0)), Idx::INT);
    }

    #[test]
    fn arc_function_var_type_multiple() {
        let func = ArcFunction {
            name: Name::from_raw(2),
            params: vec![
                ArcParam {
                    var: ArcVarId::new(0),
                    ty: Idx::INT,
                    ownership: Ownership::Owned,
                },
                ArcParam {
                    var: ArcVarId::new(1),
                    ty: Idx::STR,
                    ownership: Ownership::Borrowed,
                },
            ],
            return_type: Idx::BOOL,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(2),
                },
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::INT, Idx::STR, Idx::BOOL],
            spans: vec![vec![None]],
        };
        assert_eq!(func.var_type(ArcVarId::new(0)), Idx::INT);
        assert_eq!(func.var_type(ArcVarId::new(1)), Idx::STR);
        assert_eq!(func.var_type(ArcVarId::new(2)), Idx::BOOL);
    }

    // ── ArcInstr::defined_var ──────────────────────────────────

    #[test]
    fn defined_var_let() {
        let instr = ArcInstr::Let {
            dst: ArcVarId::new(5),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(1)),
        };
        assert_eq!(instr.defined_var(), Some(ArcVarId::new(5)));
    }

    #[test]
    fn defined_var_apply() {
        let instr = ArcInstr::Apply {
            dst: ArcVarId::new(3),
            ty: Idx::STR,
            func: Name::from_raw(10),
            args: vec![ArcVarId::new(0)],
        };
        assert_eq!(instr.defined_var(), Some(ArcVarId::new(3)));
    }

    #[test]
    fn defined_var_apply_indirect() {
        let instr = ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(7),
            ty: Idx::INT,
            closure: ArcVarId::new(1),
            args: vec![ArcVarId::new(2)],
        };
        assert_eq!(instr.defined_var(), Some(ArcVarId::new(7)));
    }

    #[test]
    fn defined_var_project() {
        let instr = ArcInstr::Project {
            dst: ArcVarId::new(4),
            ty: Idx::INT,
            value: ArcVarId::new(0),
            field: 0,
        };
        assert_eq!(instr.defined_var(), Some(ArcVarId::new(4)));
    }

    #[test]
    fn defined_var_construct() {
        let instr = ArcInstr::Construct {
            dst: ArcVarId::new(2),
            ty: Idx::UNIT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0)],
        };
        assert_eq!(instr.defined_var(), Some(ArcVarId::new(2)));
    }

    #[test]
    fn defined_var_is_shared() {
        let instr = ArcInstr::IsShared {
            dst: ArcVarId::new(9),
            var: ArcVarId::new(1),
        };
        assert_eq!(instr.defined_var(), Some(ArcVarId::new(9)));
    }

    #[test]
    fn defined_var_reset() {
        let instr = ArcInstr::Reset {
            var: ArcVarId::new(0),
            token: ArcVarId::new(10),
        };
        assert_eq!(instr.defined_var(), Some(ArcVarId::new(10)));
    }

    #[test]
    fn defined_var_reuse() {
        let instr = ArcInstr::Reuse {
            token: ArcVarId::new(10),
            dst: ArcVarId::new(11),
            ty: Idx::STR,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0)],
        };
        assert_eq!(instr.defined_var(), Some(ArcVarId::new(11)));
    }

    #[test]
    fn defined_var_rc_inc_is_none() {
        let instr = ArcInstr::RcInc {
            var: ArcVarId::new(0),
            count: 1,
        };
        assert_eq!(instr.defined_var(), None);
    }

    #[test]
    fn defined_var_rc_dec_is_none() {
        let instr = ArcInstr::RcDec {
            var: ArcVarId::new(0),
        };
        assert_eq!(instr.defined_var(), None);
    }

    #[test]
    fn defined_var_set_is_none() {
        let instr = ArcInstr::Set {
            base: ArcVarId::new(0),
            field: 0,
            value: ArcVarId::new(1),
        };
        assert_eq!(instr.defined_var(), None);
    }

    #[test]
    fn defined_var_set_tag_is_none() {
        let instr = ArcInstr::SetTag {
            base: ArcVarId::new(0),
            tag: 0,
        };
        assert_eq!(instr.defined_var(), None);
    }

    // ── ArcInstr::used_vars ────────────────────────────────────

    #[test]
    fn used_vars_let_var() {
        let instr = ArcInstr::Let {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            value: ArcValue::Var(ArcVarId::new(0)),
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(0)]);
    }

    #[test]
    fn used_vars_let_literal() {
        let instr = ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(42)),
        };
        assert!(instr.used_vars().is_empty());
    }

    #[test]
    fn used_vars_let_primop() {
        let instr = ArcInstr::Let {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Add),
                args: vec![ArcVarId::new(0), ArcVarId::new(1)],
            },
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(0), ArcVarId::new(1)]);
    }

    #[test]
    fn used_vars_apply() {
        let instr = ArcInstr::Apply {
            dst: ArcVarId::new(3),
            ty: Idx::INT,
            func: Name::from_raw(10),
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(0), ArcVarId::new(1)]);
    }

    #[test]
    fn used_vars_apply_indirect() {
        let instr = ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(5),
            ty: Idx::INT,
            closure: ArcVarId::new(3),
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        };
        assert_eq!(
            instr.used_vars(),
            vec![ArcVarId::new(3), ArcVarId::new(0), ArcVarId::new(1)]
        );
    }

    #[test]
    fn used_vars_construct() {
        let instr = ArcInstr::Construct {
            dst: ArcVarId::new(4),
            ty: Idx::UNIT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
        };
        assert_eq!(
            instr.used_vars(),
            vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)]
        );
    }

    #[test]
    fn used_vars_project() {
        let instr = ArcInstr::Project {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            value: ArcVarId::new(0),
            field: 1,
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(0)]);
    }

    #[test]
    fn used_vars_rc_inc() {
        let instr = ArcInstr::RcInc {
            var: ArcVarId::new(3),
            count: 2,
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(3)]);
    }

    #[test]
    fn used_vars_rc_dec() {
        let instr = ArcInstr::RcDec {
            var: ArcVarId::new(7),
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(7)]);
    }

    #[test]
    fn used_vars_set() {
        let instr = ArcInstr::Set {
            base: ArcVarId::new(0),
            field: 1,
            value: ArcVarId::new(2),
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(0), ArcVarId::new(2)]);
    }

    #[test]
    fn used_vars_set_tag() {
        let instr = ArcInstr::SetTag {
            base: ArcVarId::new(5),
            tag: 3,
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(5)]);
    }

    #[test]
    fn used_vars_reset() {
        let instr = ArcInstr::Reset {
            var: ArcVarId::new(0),
            token: ArcVarId::new(10),
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(0)]);
    }

    #[test]
    fn used_vars_reuse() {
        let instr = ArcInstr::Reuse {
            token: ArcVarId::new(10),
            dst: ArcVarId::new(11),
            ty: Idx::STR,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        };
        assert_eq!(
            instr.used_vars(),
            vec![ArcVarId::new(10), ArcVarId::new(0), ArcVarId::new(1)]
        );
    }

    #[test]
    fn used_vars_is_shared() {
        let instr = ArcInstr::IsShared {
            dst: ArcVarId::new(9),
            var: ArcVarId::new(1),
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(1)]);
    }

    #[test]
    fn used_vars_partial_apply() {
        let instr = ArcInstr::PartialApply {
            dst: ArcVarId::new(6),
            ty: Idx::UNIT,
            func: Name::from_raw(20),
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        };
        assert_eq!(instr.used_vars(), vec![ArcVarId::new(0), ArcVarId::new(1)]);
    }

    // ── ArcTerminator::used_vars ───────────────────────────────

    #[test]
    fn terminator_used_vars_return() {
        let t = ArcTerminator::Return {
            value: ArcVarId::new(5),
        };
        assert_eq!(t.used_vars(), vec![ArcVarId::new(5)]);
    }

    #[test]
    fn terminator_used_vars_jump() {
        let t = ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        };
        assert_eq!(t.used_vars(), vec![ArcVarId::new(0), ArcVarId::new(1)]);
    }

    #[test]
    fn terminator_used_vars_branch() {
        let t = ArcTerminator::Branch {
            cond: ArcVarId::new(3),
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        };
        assert_eq!(t.used_vars(), vec![ArcVarId::new(3)]);
    }

    #[test]
    fn terminator_used_vars_switch() {
        let t = ArcTerminator::Switch {
            scrutinee: ArcVarId::new(7),
            cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
            default: ArcBlockId::new(3),
        };
        assert_eq!(t.used_vars(), vec![ArcVarId::new(7)]);
    }

    #[test]
    fn terminator_used_vars_invoke() {
        let t = ArcTerminator::Invoke {
            dst: ArcVarId::new(10),
            ty: Idx::INT,
            func: Name::from_raw(1),
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        };
        assert_eq!(t.used_vars(), vec![ArcVarId::new(0), ArcVarId::new(1)]);
    }

    #[test]
    fn terminator_used_vars_resume() {
        assert!(ArcTerminator::Resume.used_vars().is_empty());
    }

    #[test]
    fn terminator_used_vars_unreachable() {
        assert!(ArcTerminator::Unreachable.used_vars().is_empty());
    }

    // ── ArcFunction helpers ────────────────────────────────────────

    #[test]
    fn fresh_var_sequential_ids() {
        let mut func = ArcFunction {
            name: Name::from_raw(1),
            params: vec![ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            }],
            return_type: Idx::INT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(0),
                },
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::INT],
            spans: vec![vec![]],
        };

        let v1 = func.fresh_var(Idx::STR);
        assert_eq!(v1, ArcVarId::new(1));
        assert_eq!(func.var_type(v1), Idx::STR);

        let v2 = func.fresh_var(Idx::BOOL);
        assert_eq!(v2, ArcVarId::new(2));
        assert_eq!(func.var_type(v2), Idx::BOOL);
        assert_eq!(func.var_types.len(), 3);
    }

    // ── Serde roundtrip tests (cache feature) ──────────────────

    #[cfg(feature = "cache")]
    #[test]
    fn test_arc_ir_roundtrip() {
        let func = ArcFunction {
            name: Name::from_raw(42),
            params: vec![ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            }],
            return_type: Idx::INT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(1),
                },
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::INT, Idx::INT],
            spans: vec![vec![Some(ori_ir::Span::new(10, 20))]],
        };

        let bytes = bincode::serialize(&func).unwrap_or_else(|e| panic!("serialize failed: {e}"));
        let deserialized: ArcFunction =
            bincode::deserialize(&bytes).unwrap_or_else(|e| panic!("deserialize failed: {e}"));

        // Core data should match exactly
        assert_eq!(deserialized.name, func.name);
        assert_eq!(deserialized.params, func.params);
        assert_eq!(deserialized.return_type, func.return_type);
        assert_eq!(deserialized.blocks, func.blocks);
        assert_eq!(deserialized.entry, func.entry);
        assert_eq!(deserialized.var_types, func.var_types);

        // Spans are skipped during serialization — deserialized gets Default (empty vec)
        assert!(
            deserialized.spans.is_empty(),
            "spans should be empty after deserialization (skipped by serde)"
        );
    }

    #[cfg(feature = "cache")]
    #[test]
    fn test_arc_ir_all_instr_variants() {
        // Every ArcInstr variant must serialize/deserialize cleanly
        let instrs = vec![
            ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Let {
                dst: ArcVarId::new(1),
                ty: Idx::FLOAT,
                value: ArcValue::Var(ArcVarId::new(0)),
            },
            ArcInstr::Let {
                dst: ArcVarId::new(2),
                ty: Idx::INT,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                },
            },
            ArcInstr::Apply {
                dst: ArcVarId::new(3),
                ty: Idx::STR,
                func: Name::from_raw(10),
                args: vec![ArcVarId::new(0)],
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(4),
                ty: Idx::INT,
                closure: ArcVarId::new(3),
                args: vec![ArcVarId::new(0)],
            },
            ArcInstr::PartialApply {
                dst: ArcVarId::new(5),
                ty: Idx::UNIT,
                func: Name::from_raw(20),
                args: vec![ArcVarId::new(0)],
            },
            ArcInstr::Project {
                dst: ArcVarId::new(6),
                ty: Idx::INT,
                value: ArcVarId::new(3),
                field: 2,
            },
            ArcInstr::Construct {
                dst: ArcVarId::new(7),
                ty: Idx::UNIT,
                ctor: CtorKind::Tuple,
                args: vec![ArcVarId::new(0), ArcVarId::new(1)],
            },
            ArcInstr::RcInc {
                var: ArcVarId::new(0),
                count: 3,
            },
            ArcInstr::RcDec {
                var: ArcVarId::new(0),
            },
            ArcInstr::IsShared {
                dst: ArcVarId::new(8),
                var: ArcVarId::new(0),
            },
            ArcInstr::Set {
                base: ArcVarId::new(0),
                field: 1,
                value: ArcVarId::new(1),
            },
            ArcInstr::SetTag {
                base: ArcVarId::new(0),
                tag: 42,
            },
            ArcInstr::Reset {
                var: ArcVarId::new(0),
                token: ArcVarId::new(9),
            },
            ArcInstr::Reuse {
                token: ArcVarId::new(9),
                dst: ArcVarId::new(10),
                ty: Idx::STR,
                ctor: CtorKind::Struct(Name::from_raw(5)),
                args: vec![ArcVarId::new(0)],
            },
        ];

        for (i, instr) in instrs.iter().enumerate() {
            let bytes = bincode::serialize(instr)
                .unwrap_or_else(|e| panic!("serialize instr {i} failed: {e}"));
            let roundtripped: ArcInstr = bincode::deserialize(&bytes)
                .unwrap_or_else(|e| panic!("deserialize instr {i} failed: {e}"));
            assert_eq!(
                &roundtripped, instr,
                "roundtrip failed for instr variant {i}"
            );
        }

        // Also test all terminator variants
        let terminators = vec![
            ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
            ArcTerminator::Jump {
                target: ArcBlockId::new(1),
                args: vec![ArcVarId::new(0)],
            },
            ArcTerminator::Branch {
                cond: ArcVarId::new(0),
                then_block: ArcBlockId::new(1),
                else_block: ArcBlockId::new(2),
            },
            ArcTerminator::Switch {
                scrutinee: ArcVarId::new(0),
                cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
                default: ArcBlockId::new(3),
            },
            ArcTerminator::Invoke {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                func: Name::from_raw(10),
                args: vec![ArcVarId::new(0)],
                normal: ArcBlockId::new(1),
                unwind: ArcBlockId::new(2),
            },
            ArcTerminator::Resume,
            ArcTerminator::Unreachable,
        ];

        for (i, term) in terminators.iter().enumerate() {
            let bytes = bincode::serialize(term)
                .unwrap_or_else(|e| panic!("serialize terminator {i} failed: {e}"));
            let roundtripped: ArcTerminator = bincode::deserialize(&bytes)
                .unwrap_or_else(|e| panic!("deserialize terminator {i} failed: {e}"));
            assert_eq!(
                &roundtripped, term,
                "roundtrip failed for terminator variant {i}"
            );
        }
    }

    // ── ArcFunction helpers ────────────────────────────────────────

    #[test]
    fn next_block_id_and_push() {
        let mut func = ArcFunction {
            name: Name::from_raw(1),
            params: vec![],
            return_type: Idx::UNIT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![],
            spans: vec![vec![]],
        };

        assert_eq!(func.next_block_id(), ArcBlockId::new(1));

        func.push_block(ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            }],
            terminator: ArcTerminator::Unreachable,
        });

        assert_eq!(func.blocks.len(), 2);
        assert_eq!(func.spans.len(), 2);
        assert_eq!(func.spans[1].len(), 1); // one instr → one span slot
        assert_eq!(func.next_block_id(), ArcBlockId::new(2));
    }
}
