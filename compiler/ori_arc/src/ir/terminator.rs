//! Methods on [`ArcTerminator`].
//!
//! Variable queries, substitution, and ownership position checks for
//! block terminators. Mirrors [`ArcInstr`](super::ArcInstr) methods
//! in `instr.rs`.

use smallvec::{smallvec, SmallVec};

use super::{ArcTerminator, ArcVarId, ArgOwnership};

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

    /// Check whether a `used_vars()` position is "owned" for this terminator.
    ///
    /// Mirrors [`ArcInstr::is_owned_position`] but for terminators.
    ///
    /// - `Invoke`: `arg_ownership[pos]`, defaults to Owned (same as `Apply`).
    /// - `InvokeIndirect`: `used_vars()` = `[...args, closure]` (closure LAST).
    ///   Positions `0..args.len()` map to `arg_ownership[pos]`, defaults to
    ///   Borrowed (conservative). The closure at position `args.len()` is
    ///   always borrowed.
    /// - All others: no owned positions.
    pub fn is_owned_position(&self, pos: usize) -> bool {
        match self {
            ArcTerminator::Invoke {
                args,
                arg_ownership,
                ..
            } => {
                pos < args.len()
                    && arg_ownership
                        .get(pos)
                        .is_none_or(|o| *o == ArgOwnership::Owned)
            }
            ArcTerminator::InvokeIndirect {
                args,
                arg_ownership,
                ..
            } => {
                // used_vars() = [...args, closure]. Closure is at the end,
                // not the beginning (opposite of ApplyIndirect). So pos
                // maps directly to args[pos] with no offset.
                pos < args.len()
                    && arg_ownership
                        .get(pos)
                        .is_some_and(|o| *o == ArgOwnership::Owned)
            }
            _ => false,
        }
    }
}
