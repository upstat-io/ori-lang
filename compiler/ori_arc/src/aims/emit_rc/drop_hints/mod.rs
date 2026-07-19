//! Drop hint helpers for AIMS.
//!
//! Contains helper functions used by `realize/` for drop hint decisions.
//! Drop hints are computed by `realize_annotations()`.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::{Pool, Tag};

use crate::aims::contract::MemoryContract;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};

/// Check if a variable's type is a collection (List, Set, Map).
pub(crate) fn is_collection_var(func: &ArcFunction, var: ArcVarId, pool: &Pool) -> bool {
    let Some(&idx) = func.var_types.get(var.index()) else {
        return false;
    };
    let resolved = pool.resolve_fully(idx);
    let tag = pool.tag(resolved);
    matches!(tag, Tag::List | Tag::Set | Tag::Map)
}

/// Collect variables passed as `Borrowed` arguments to Apply/Invoke calls
/// whose callees may share references (`effects.may_share == true`).
///
/// A callee may mint a logical owner credit on a borrowed argument's storage
/// identity, as a sharing view does. When its contract has
/// `effects.may_share == true`, conservatively exclude the argument from
/// single-owner cleanup.
///
/// When `effects.may_share == false` (pure functions, simple accessors),
/// the callee provably doesn't create shared references, so borrowed args
/// preserve uniqueness and remain eligible for single-owner cleanup.
///
/// Also propagates through Let alias chains: if `%x` is borrowed in a call
/// and `%y = %x`, then `%y` is also excluded.
pub(crate) fn collect_borrowed_call_args(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    builtins: &BuiltinOwnershipSets,
    func_names: &FxHashSet<Name>,
) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    let mut alias_children: Vec<Vec<ArcVarId>> = vec![Vec::new(); func.var_types.len()];

    for block in &func.blocks {
        // Scan body instructions for Apply with Borrowed args.
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    func: callee,
                    args,
                    arg_ownership,
                    ..
                } => {
                    // For user functions (not builtins) with may_share==false,
                    // the callee provably doesn't share backing storage →
                    // borrowed args preserve uniqueness → safe for drop_unique.
                    // The transitional builtin contract surface is incomplete;
                    // production typed runtime descriptors close this fact.
                    if !is_safe_non_sharing_callee(*callee, contracts, builtins, func_names) {
                        for (i, &arg) in args.iter().enumerate() {
                            if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                                borrowed.insert(arg);
                            }
                        }
                    }
                }
                // ApplyIndirect: use arg_ownership when populated, with
                // conservative all-borrowed fallback for unannotated calls.
                ArcInstr::ApplyIndirect {
                    args,
                    arg_ownership,
                    ..
                } => {
                    insert_borrowed_from_ownership(args, arg_ownership, &mut borrowed);
                }
                ArcInstr::Let {
                    dst,
                    value: crate::ir::ArcValue::Var(src),
                    ..
                } => {
                    alias_children[src.index()].push(*dst);
                }
                _ => {}
            }
        }

        // Scan terminator for Invoke/InvokeIndirect with Borrowed args.
        match &block.terminator {
            ArcTerminator::Invoke {
                func: callee,
                args,
                arg_ownership,
                ..
            } => {
                if !is_safe_non_sharing_callee(*callee, contracts, builtins, func_names) {
                    for (i, &arg) in args.iter().enumerate() {
                        if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                            borrowed.insert(arg);
                        }
                    }
                }
            }
            ArcTerminator::InvokeIndirect {
                args,
                arg_ownership,
                ..
            } => {
                insert_borrowed_from_ownership(args, arg_ownership, &mut borrowed);
            }
            _ => {}
        }
    }

    // Propagate through alias chains once per reachable edge. A repeated
    // full-vector fixed-point scan is quadratic for long alias chains.
    let mut pending: Vec<ArcVarId> = borrowed.iter().copied().collect();
    while let Some(src) = pending.pop() {
        for &dst in &alias_children[src.index()] {
            if borrowed.insert(dst) {
                pending.push(dst);
            }
        }
    }

    borrowed
}

/// Insert borrowed args from ownership annotations, with conservative fallback.
///
/// When `arg_ownership` is populated, only args with `ArgOwnership::Borrowed`
/// are inserted. When `arg_ownership` is empty (unannotated indirect call),
/// ALL args are conservatively marked as borrowed to preserve safety in
/// release builds where the `debug_assert!` in `arg_ownership/mod.rs` is stripped.
fn insert_borrowed_from_ownership(
    args: &[ArcVarId],
    arg_ownership: &[ArgOwnership],
    borrowed: &mut FxHashSet<ArcVarId>,
) {
    if arg_ownership.is_empty() {
        // Fallback: unannotated indirect call — conservative all-borrowed.
        for &arg in args {
            borrowed.insert(arg);
        }
    } else {
        for (i, &arg) in args.iter().enumerate() {
            if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                borrowed.insert(arg);
            }
        }
    }
}

/// Check if a callee is a user function proven to not share references.
///
/// Returns `true` only for user-analyzed functions (not builtins) where
/// `effects.may_share == false`. Builtin contracts are not trusted here
/// because the transitional runtime descriptor surface does not yet prove all
/// sharing-credit behavior. Production executable descriptors remove this
/// conservative exception.
fn is_safe_non_sharing_callee(
    callee: Name,
    contracts: &FxHashMap<Name, MemoryContract>,
    builtins: &BuiltinOwnershipSets,
    func_names: &FxHashSet<Name>,
) -> bool {
    // If the callee is a known builtin, always conservative.
    if builtins.contains(callee) {
        return false;
    }
    // For user functions: trust the contract's may_share flag. A None lookup
    // is legitimate only for FFI / external / DCE'd callees not in this
    // compilation unit; those are conservatively non-safe (may_share = true).
    // Why: a callee present in func_names but missing from contracts is an
    // IC-1 pipeline-ordering violation (every analyzed function gets a
    // MemoryContract after the interprocedural fixpoint).
    if let Some(c) = contracts.get(&callee) {
        !c.effects.may_share
    } else {
        debug_assert!(
            !func_names.contains(&callee),
            "AIMS Invariant IC-1: callee {callee:?} is in func_names but \
             missing from contracts map; pipeline ordering violation"
        );
        false
    }
}

#[cfg(test)]
mod tests;
