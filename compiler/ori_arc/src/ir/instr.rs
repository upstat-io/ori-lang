//! ARC IR instruction definitions.
//!
//! [`ArcInstr`] represents a single instruction in an ARC IR basic block.
//! Most produce a value bound to a `dst` variable. RC operations (`RcInc`,
//! `RcDec`) are inserted by Section 07 and optimized by Section 08.

use smallvec::{smallvec, SmallVec};

use ori_types::Idx;

use super::{ArcValue, ArcVarId, ArgOwnership, CtorKind, RcStrategy};

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
        func: ori_ir::Name,
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
        func: ori_ir::Name,
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
    /// `switch -> arm_block(br) -> merge(phi)`, we emit
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
    /// `reuse_dst -> reset_var` on the fast path.
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
