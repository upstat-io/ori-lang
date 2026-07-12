//! `impl ArcInstr` methods: variable definition/use analysis and read-position substitution.

use smallvec::{smallvec, SmallVec};

use crate::ir::{ArcValue, ArcVarId, ArgOwnership};

use super::ArcInstr;

impl ArcInstr {
    /// Returns the variable defined (written) by this instruction, if any.
    ///
    /// Value-producing instructions (`Let`, `Apply`, `ApplyIndirect`,
    /// `PartialApply`, `Project`, `Construct`, `IsShared`, `Reuse`)
    /// return `Some(dst)`. `Reset` returns `Some(token)` (the reuse token
    /// it defines). Side-effect-only instructions (`RcInc`, `RcDec`,
    /// `Set`, `SetTag`) return `None`.
    ///
    /// Used by liveness analysis, RC emission, and RC elimination.
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
            | ArcInstr::CollectionReuse { dst, .. }
            | ArcInstr::Select { dst, .. } => Some(*dst),

            ArcInstr::Reset { token, .. } => Some(*token),

            ArcInstr::RcInc { .. }
            | ArcInstr::RcDec { .. }
            | ArcInstr::RcDecPartial { .. }
            | ArcInstr::RcDecField { .. }
            | ArcInstr::RcDecVariant { .. }
            | ArcInstr::BurdenInc { .. }
            | ArcInstr::BurdenDec { .. }
            | ArcInstr::BurdenDecPartial { .. }
            | ArcInstr::BurdenDecField { .. }
            | ArcInstr::BurdenDecVariant { .. }
            | ArcInstr::Set { .. }
            | ArcInstr::SetTag { .. } => None,
        }
    }

    /// Whether this instruction is a release-cleanup op — a whole-var `RcDec`
    /// or its paired whole-var `BurdenDec`.
    ///
    /// Release-cleanup blocks (dead-at-entry cleanup, tail-call post-Apply
    /// cleanup) hold the burden-faithful release sequence emitted by the
    /// class-ledger's per-edge/per-death placement: a paired `BurdenDec`
    /// adjacent to each release `RcDec` whose var carries burden ops (Spec:
    /// Annex E §AIMS RL-2 / RL-4 / RL-5). That placement emits only the
    /// WHOLE-VAR `BurdenDec` variant alongside `RcDec` — never
    /// `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant`, which arise
    /// from partial-move / `Set` / `SetTag` payload positions, not release
    /// cleanup. The predicate therefore matches `RcDec | BurdenDec` only;
    /// broadening to the partial variants would let a real payload-dec block
    /// be misread as a skippable cleanup block.
    ///
    /// SSOT for the "block body is only release cleanup" test consumed by
    /// tail-call detection (`find_tail_apply_in_block` / `find_invoke_tail_calls`
    /// post-Apply / normal-block cleanup gates).
    #[inline]
    pub fn is_release_cleanup_instr(&self) -> bool {
        matches!(self, ArcInstr::RcDec { .. } | ArcInstr::BurdenDec { .. })
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
    /// Used by liveness analysis for computing gen sets.
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

            ArcInstr::CollectionReuse { old_var, args, .. } => {
                let mut vars = SmallVec::with_capacity(1 + args.len());
                vars.push(*old_var);
                vars.extend_from_slice(args);
                vars
            }

            ArcInstr::Project { value, .. } => smallvec![*value],

            ArcInstr::RcInc { var, .. }
            | ArcInstr::RcDec { var, .. }
            | ArcInstr::RcDecPartial { var, .. }
            | ArcInstr::RcDecVariant { var }
            | ArcInstr::BurdenInc { var }
            | ArcInstr::BurdenDec { var }
            | ArcInstr::BurdenDecPartial { var, .. }
            | ArcInstr::BurdenDecVariant { var }
            | ArcInstr::IsShared { var, .. }
            | ArcInstr::Reset { var, .. } => smallvec![*var],

            ArcInstr::RcDecField { base, .. }
            | ArcInstr::BurdenDecField { base, .. }
            | ArcInstr::SetTag { base, .. } => {
                smallvec![*base]
            }

            ArcInstr::Set { base, value, .. } => smallvec![*base, *value],

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
    /// Used by reset/reuse detection and RC elimination in inner loops
    /// where allocation per check is wasteful.
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

            ArcInstr::CollectionReuse { old_var, args, .. } => {
                *old_var == target || args.contains(&target)
            }

            ArcInstr::Project { value, .. } => *value == target,

            ArcInstr::RcInc { var, .. }
            | ArcInstr::RcDec { var, .. }
            | ArcInstr::RcDecPartial { var, .. }
            | ArcInstr::RcDecVariant { var }
            | ArcInstr::BurdenInc { var }
            | ArcInstr::BurdenDec { var }
            | ArcInstr::BurdenDecPartial { var, .. }
            | ArcInstr::BurdenDecVariant { var }
            | ArcInstr::IsShared { var, .. }
            | ArcInstr::Reset { var, .. } => *var == target,

            ArcInstr::RcDecField { base, .. }
            | ArcInstr::BurdenDecField { base, .. }
            | ArcInstr::SetTag { base, .. } => *base == target,

            ArcInstr::Set { base, value, .. } => *base == target || *value == target,

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
    /// - `ApplyIndirect`: args only (`1..=args.len()`); closure (pos 0) is borrowed
    /// - `Reuse`, `CollectionReuse`: args only (`1..=args.len()`); the token /
    ///   `old_var` at pos 0 is a consumed handle, not an owned RC position
    /// - Everything else: no owned positions (read-only uses)
    pub fn is_owned_position(&self, pos: usize) -> bool {
        match self {
            ArcInstr::Construct { args, .. } | ArcInstr::PartialApply { args, .. } => {
                pos < args.len()
            }
            // Reuse: token at position 0 is a consumed scalar reuse token (not
            //   owned); positions 1..=args.len() are owned (stored into the
            //   reused allocation — an RL-2 transfer, like Construct args).
            // CollectionReuse: old_var at position 0 is consumed (not owned);
            //   positions 1..=args.len() are owned (stored into buffer).
            ArcInstr::Reuse { args, .. } | ArcInstr::CollectionReuse { args, .. } => {
                pos >= 1 && pos <= args.len()
            }
            // ApplyIndirect: used_vars() returns [closure, ...args].
            // pos=0 is closure (always borrowed). pos 1..=args.len() are
            // user args. arg_ownership parallels args, so
            // arg_ownership[i] corresponds to used_vars position i+1.
            //
            // Empty arg_ownership (pre-annotation) → is_some_and returns
            // false → all positions NOT owned. This is safe (conservative:
            // caller retains cleanup). Differs from Apply's is_none_or
            // which defaults to Owned.
            ArcInstr::ApplyIndirect {
                args,
                arg_ownership,
                ..
            } => {
                if pos == 0 {
                    return false; // closure is always borrowed
                }
                let arg_idx = pos - 1;
                arg_idx < args.len()
                    && arg_ownership
                        .get(arg_idx)
                        .is_some_and(|o| *o == ArgOwnership::Owned)
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
            _ => false,
        }
    }

    /// Replace all occurrences of `old` with `new` in read positions.
    ///
    /// Defined variables (`dst`) are NOT substituted — only used variables.
    /// Used by constructor reuse expansion to substitute
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
            ArcInstr::CollectionReuse { old_var, args, .. } => {
                sub(old_var, old, new);
                sub_args(args, old, new);
            }
            ArcInstr::Project { value, .. } => sub(value, old, new),
            ArcInstr::RcInc { var, .. }
            | ArcInstr::RcDec { var, .. }
            | ArcInstr::RcDecPartial { var, .. }
            | ArcInstr::RcDecVariant { var }
            | ArcInstr::BurdenInc { var }
            | ArcInstr::BurdenDec { var }
            | ArcInstr::BurdenDecPartial { var, .. }
            | ArcInstr::BurdenDecVariant { var }
            | ArcInstr::IsShared { var, .. }
            | ArcInstr::Reset { var, .. } => sub(var, old, new),
            ArcInstr::RcDecField { base, .. }
            | ArcInstr::BurdenDecField { base, .. }
            | ArcInstr::SetTag { base, .. } => {
                sub(base, old, new);
            }
            ArcInstr::Set { base, value, .. } => {
                sub(base, old, new);
                sub(value, old, new);
            }
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
