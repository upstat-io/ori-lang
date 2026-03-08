//! Ownership update pass for borrow inference.
//!
//! Contains the core logic for promoting Borrowed parameters to Owned
//! based on instruction analysis: returns, stores, callee argument
//! ownership, and tail call preservation.

use rustc_hash::FxHashMap;

use ori_ir::builtin_constants::protocol::ProtocolArgOwnership;
use ori_ir::Name;

use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::ownership::{AnnotatedSig, Ownership};

/// Context for callee argument ownership promotion.
///
/// Groups the shared state needed by [`promote_callee_args`] and
/// [`check_tail_call`], replacing their 8-parameter signatures.
pub(super) struct PromoteCtx<'a> {
    pub(super) func: &'a ArcFunction,
    pub(super) my_sig: &'a mut AnnotatedSig,
    pub(super) local_sigs: &'a FxHashMap<Name, AnnotatedSig>,
    pub(super) external_sigs: &'a FxHashMap<Name, AnnotatedSig>,
    pub(super) builtins: &'a BuiltinOwnershipSets,
    pub(super) aliases: &'a FxHashMap<ArcVarId, ArcVarId>,
}

/// Returns the index into `func.params` if `var` is a function parameter.
fn param_index(var: ArcVarId, func: &ArcFunction) -> Option<usize> {
    func.params.iter().position(|p| p.var == var)
}

/// Check whether a variable is "owned" in the current analysis state.
///
/// A variable is owned if:
/// - It is a function parameter with `Ownership::Owned`, OR
/// - It is any non-parameter local variable (locals always own their values
///   from the point of definition).
///
/// Resolves alias chains before checking, so `v1 = Var(v0)` where `v0` is
/// a Borrowed param will correctly report `v1` as not owned.
fn is_owned_var(
    var: ArcVarId,
    func: &ArcFunction,
    sig: &AnnotatedSig,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
) -> bool {
    let resolved = resolve_alias(var, aliases);
    match param_index(resolved, func) {
        Some(pidx) => sig.params[pidx].ownership == Ownership::Owned,
        None => true, // Local variables are always owned.
    }
}

/// Try to mark a parameter as Owned. Returns `true` if it changed.
fn mark_owned(sig: &mut AnnotatedSig, pidx: usize) -> bool {
    if sig.params[pidx].ownership == Ownership::Borrowed {
        sig.params[pidx].ownership = Ownership::Owned;
        true
    } else {
        false
    }
}

/// Try to mark a variable as Owned if it is a Borrowed parameter.
/// Returns `true` if a parameter was promoted.
///
/// Resolves alias chains: if `var` is a `Let { value: Var(x) }` alias
/// of a parameter, the parameter is promoted. This prevents cases where
/// a parameter is aliased then consumed (e.g., `v1 = v0; Construct([v1])`)
/// from incorrectly leaving the param as Borrowed.
fn try_mark_param_owned(
    var: ArcVarId,
    func: &ArcFunction,
    sig: &mut AnnotatedSig,
    aliases: &FxHashMap<ArcVarId, ArcVarId>,
) -> bool {
    let resolved = resolve_alias(var, aliases);
    if let Some(pidx) = param_index(resolved, func) {
        mark_owned(sig, pidx)
    } else {
        false
    }
}

/// Build a mapping from alias variables to their source.
///
/// For each `Let { dst, value: Var(src) }`, records `dst → src`. This allows
/// ownership promotion to trace through alias chains like `v1 = v0` where `v0`
/// is a parameter and `v1` is consumed by a Construct or returned.
fn build_alias_map(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut aliases = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                aliases.insert(*dst, *src);
            }
        }
    }
    aliases
}

/// Resolve an alias chain to the root variable.
///
/// Follows `Let { value: Var(x) }` chains: `v2 → v1 → v0 (param)`.
/// Terminates at the first non-alias variable or after a safety limit.
///
/// The 64-step limit is purely defensive: in practice, alias chains are 1–3
/// deep (one `Let { value: Var(x) }` per level). The limit guards against
/// pathological or cyclic alias maps — if hit, it indicates a bug in
/// `build_alias_map` or upstream lowering, not normal program structure.
fn resolve_alias(var: ArcVarId, aliases: &FxHashMap<ArcVarId, ArcVarId>) -> ArcVarId {
    let mut current = var;
    let mut steps = 0u32;
    while let Some(&src) = aliases.get(&current) {
        current = src;
        steps += 1;
        if steps > 64 {
            break;
        }
    }
    current
}

/// Promote borrowed parameters to owned based on callee argument ownership.
///
/// Dispatches through three strategies in order:
/// 1. **Known callee signature** (local SCC or external) — use per-param ownership.
/// 2. **Protocol builtin** — use `ProtocolBuiltin::arg_ownership()` data.
/// 3. **Unknown, non-borrowing callee** — conservatively promote all args to owned.
///
/// Callees in the `borrowing` set are assumed to borrow all arguments (no promotion).
///
/// Used by both `Apply` instructions and `Invoke` terminators — the ownership
/// promotion logic is identical for both call forms.
fn promote_callee_args(callee: Name, args: &[ArcVarId], ctx: &mut PromoteCtx<'_>) -> bool {
    let mut changed = false;
    if let Some(callee_sig) = ctx
        .local_sigs
        .get(&callee)
        .or_else(|| ctx.external_sigs.get(&callee))
    {
        for (i, &arg) in args.iter().enumerate() {
            if i < callee_sig.params.len() && callee_sig.params[i].ownership == Ownership::Owned {
                changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
            }
        }
    } else if !ctx.builtins.borrowing.contains(&callee) {
        if let Some(ownership) = ctx.builtins.protocol.get(&callee) {
            for (i, &arg) in args.iter().enumerate() {
                if ownership.get(i).copied() == Some(ProtocolArgOwnership::Owned) {
                    changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
                }
            }
        } else {
            for &arg in args {
                changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
            }
        }
    }
    changed
}

/// Core ownership update pass for a single function.
///
/// Scans all instructions and terminators, promoting Borrowed parameters to
/// Owned when required by usage (returned, stored, passed to owned position).
///
/// Callee signatures are looked up in two maps:
/// - `local_sigs`: SCC members currently being iterated (in-progress sigs).
/// - `external_sigs`: callees outside the SCC (already finalized).
///
/// Lookup order: `local_sigs` first, then `external_sigs`. This ensures
/// that within a recursive SCC, we see the latest in-progress signatures
/// of co-members, while external callees use their stable final sigs.
///
/// Returns `true` if any parameter's ownership changed.
pub(super) fn update_ownership_inner(
    func: &ArcFunction,
    my_sig: &mut AnnotatedSig,
    local_sigs: &FxHashMap<Name, AnnotatedSig>,
    external_sigs: &FxHashMap<Name, AnnotatedSig>,
    builtins: &BuiltinOwnershipSets,
) -> bool {
    let mut changed = false;
    let aliases = build_alias_map(func);

    let mut ctx = PromoteCtx {
        func,
        my_sig,
        local_sigs,
        external_sigs,
        builtins,
        aliases: &aliases,
    };

    for block in &func.blocks {
        // Scan instructions
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    args, func: callee, ..
                } => {
                    changed |= promote_callee_args(*callee, args, &mut ctx);
                }

                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    changed |= try_mark_param_owned(*closure, ctx.func, ctx.my_sig, ctx.aliases);
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
                    }
                }

                ArcInstr::PartialApply { args, .. } | ArcInstr::Construct { args, .. } => {
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
                    }
                }

                ArcInstr::CollectionReuse { old_var, args, .. } => {
                    // old_var is consumed (recycled buffer); args are stored elements.
                    changed |= try_mark_param_owned(*old_var, ctx.func, ctx.my_sig, ctx.aliases);
                    for &arg in args {
                        changed |= try_mark_param_owned(arg, ctx.func, ctx.my_sig, ctx.aliases);
                    }
                }

                ArcInstr::Project { dst, value, .. } => {
                    if is_owned_var(*dst, ctx.func, ctx.my_sig, ctx.aliases) {
                        changed |= try_mark_param_owned(*value, ctx.func, ctx.my_sig, ctx.aliases);
                    }
                }

                ArcInstr::Let { value, .. } => {
                    let _ = value;
                }

                // Select reads vars without consuming — no ownership propagation.
                // RC/reset/reuse instructions are handled by the ARC pass itself.
                ArcInstr::Select { .. }
                | ArcInstr::RcInc { .. }
                | ArcInstr::RcDec { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Reuse { .. } => {}
            }
        }

        // Scan terminator
        match &block.terminator {
            ArcTerminator::Return { value } => {
                changed |= try_mark_param_owned(*value, ctx.func, ctx.my_sig, ctx.aliases);
                changed |= check_tail_call(block, *value, &mut ctx);
            }

            ArcTerminator::Jump { args, .. } => {
                let _ = args;
            }

            ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. }
            | ArcTerminator::Unreachable
            | ArcTerminator::Resume => {}

            ArcTerminator::Invoke {
                args, func: callee, ..
            } => {
                changed |= promote_callee_args(*callee, args, &mut ctx);
            }
        }
    }

    changed
}

/// Check for tail call and promote borrowed params if needed.
///
/// A tail call is detected when the last instruction in a block is an
/// `Apply` whose `dst` is the same as the returned `value`. If the callee
/// expects an argument as Owned but the corresponding parameter in our
/// function is currently Borrowed, we must promote it to Owned to preserve
/// the tail call optimization.
fn check_tail_call(block: &ArcBlock, returned_value: ArcVarId, ctx: &mut PromoteCtx<'_>) -> bool {
    let tail_apply = block
        .body
        .iter()
        .rev()
        .find(|instr| matches!(instr, ArcInstr::Apply { dst, .. } if *dst == returned_value));

    if let Some(ArcInstr::Apply {
        func: callee, args, ..
    }) = tail_apply
    {
        promote_callee_args(*callee, args, ctx)
    } else {
        false
    }
}
