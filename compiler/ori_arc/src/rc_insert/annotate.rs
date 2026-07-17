//! Argument ownership annotation for call-site instructions.
//!
//! Populates `arg_ownership` on direct and indirect call sites from frozen
//! signatures, callable identities, and builtin ownership policy.
//!
//! Indirect-call explicit arguments always use the borrowed closure ABI.
//! The concrete closure adapter retains exactly the target parameters whose
//! frozen ownership is `Owned`; callers therefore never guess a dynamic
//! target's ownership contract.

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use ori_ir::builtin_constants::protocol::ProtocolArgOwnership;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::ownership::Ownership;
use ori_types::Pool;

/// Compute per-argument ownership from a frozen signature or builtin policy.
/// Unknown Ori callees default to owned; external runtime and borrowing
/// builtins default to borrowed. Type-qualified collection policy is applied
/// after this baseline.
fn compute_arg_ownership(
    callee: ori_ir::Name,
    arg_count: usize,
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    borrowing_builtins: &FxHashSet<ori_ir::Name>,
    protocol_builtins: &FxHashMap<ori_ir::Name, &'static [ProtocolArgOwnership]>,
) -> Vec<ArgOwnership> {
    if tracing::enabled!(target: "ori_arc::rc_insert::annotate", tracing::Level::DEBUG) {
        tracing::debug!(
            target: "ori_arc::rc_insert::annotate",
            callee = interner.try_lookup(callee).unwrap_or("?"),
            in_sigs = sigs.contains_key(&callee),
            sig_param_ownership = ?sigs.get(&callee).map(|signature| {
                signature.params.iter().map(|param| param.ownership).collect::<Vec<_>>()
            }),
            arg_count,
            "arg_ownership verdict"
        );
    }

    if let Some(sig) = sigs.get(&callee) {
        return (0..arg_count)
            .map(|i| match sig.params.get(i) {
                Some(p) if p.ownership == Ownership::Owned => ArgOwnership::Owned,
                _ => ArgOwnership::Borrowed,
            })
            .collect();
    }

    // Why: `ori_iter_drop` has owned protocol semantics despite its runtime prefix.
    if let Some(ownership) = protocol_builtins.get(&callee) {
        assert_eq!(
            ownership.len(),
            arg_count,
            "ProtocolBuiltin arity mismatch: expected {}, got {} args",
            ownership.len(),
            arg_count,
        );
        return ownership
            .iter()
            .map(|ownership| match ownership {
                ProtocolArgOwnership::Owned => ArgOwnership::Owned,
                ProtocolArgOwnership::Borrowed => ArgOwnership::Borrowed,
            })
            .collect();
    }
    if interner
        .try_lookup(callee)
        .is_some_and(|name| name.starts_with("ori_"))
        || borrowing_builtins.contains(&callee)
    {
        return vec![ArgOwnership::Borrowed; arg_count];
    }

    vec![ArgOwnership::Owned; arg_count]
}

/// Populate argument ownership while protecting exact local/imported callable
/// identities from same-spelled builtin method policy.
pub(crate) fn annotate_arg_ownership(
    func: &mut ArcFunction,
    sigs: &rustc_hash::FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    builtins: &crate::BuiltinOwnershipSets,
    pool: &Pool,
    exact_callables: &FxHashSet<ori_ir::Name>,
) {
    let var_types = &func.var_types;

    for block in &mut func.blocks {
        for instr in &mut block.body {
            match instr {
                ArcInstr::Apply {
                    func: callee,
                    args,
                    arg_ownership,
                    ..
                } => {
                    *arg_ownership = compute_arg_ownership(
                        *callee,
                        args.len(),
                        sigs,
                        interner,
                        &builtins.borrowing,
                        &builtins.protocol,
                    );
                    if !exact_callables.contains(callee) {
                        apply_type_qualified_consuming_positions(
                            *callee,
                            args,
                            arg_ownership,
                            builtins,
                            var_types,
                            pool,
                        );
                    }
                }
                ArcInstr::ApplyIndirect {
                    args,
                    arg_ownership,
                    ..
                } => {
                    *arg_ownership = vec![ArgOwnership::Borrowed; args.len()];
                }
                _ => {}
            }
        }

        match &mut block.terminator {
            ArcTerminator::Invoke {
                func: callee,
                args,
                arg_ownership,
                ..
            } => {
                *arg_ownership = compute_arg_ownership(
                    *callee,
                    args.len(),
                    sigs,
                    interner,
                    &builtins.borrowing,
                    &builtins.protocol,
                );
                if !exact_callables.contains(callee) {
                    apply_type_qualified_consuming_positions(
                        *callee,
                        args,
                        arg_ownership,
                        builtins,
                        var_types,
                        pool,
                    );
                }
            }
            ArcTerminator::InvokeIndirect {
                args,
                arg_ownership,
                ..
            } => {
                *arg_ownership = vec![ArgOwnership::Borrowed; args.len()];
            }
            _ => {}
        }
    }
}

fn apply_type_qualified_consuming_positions(
    callee: ori_ir::Name,
    args: &[ArcVarId],
    arg_ownership: &mut [ArgOwnership],
    builtins: &crate::BuiltinOwnershipSets,
    var_types: &[ori_types::Idx],
    pool: &Pool,
) {
    if args.is_empty() || arg_ownership.is_empty() {
        return;
    }

    let arg_tags: SmallVec<[Option<ori_registry::TypeTag>; 3]> = args
        .iter()
        .map(|arg| {
            let resolved = pool.resolve_fully(var_types[arg.index()]);
            pool.builtin_type_tag(resolved)
        })
        .collect();
    let positions = builtins.type_qualified_consuming_positions(callee, &arg_tags);
    if positions.is_empty() {
        return;
    }

    if matches!(
        arg_tags.first(),
        Some(Some(
            ori_registry::TypeTag::List | ori_registry::TypeTag::Map | ori_registry::TypeTag::Set
        ))
    ) {
        arg_ownership.fill(ArgOwnership::Borrowed);
    }
    for position in positions {
        if let Some(ownership) = arg_ownership.get_mut(position) {
            *ownership = ArgOwnership::Owned;
        }
    }
}
