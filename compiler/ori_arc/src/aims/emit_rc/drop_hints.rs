//! Drop hint helpers for AIMS.
//!
//! Contains helper functions used by `realize/` for drop hint decisions.
//! The legacy `compute_aims_drop_hints()` entry point has been removed —
//! drop hints are now computed by `realize_annotations()` (Section 10).

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
/// Runtime functions may internally call `ori_rc_inc` on borrowed arguments
/// (e.g., `ori_list_slice` increments the original buffer's refcount to keep
/// the slice alive). These runtime-internal RC ops are invisible to AIMS
/// analysis. When the callee's contract has `effects.may_share == true`,
/// we conservatively exclude borrowed args from the unique-drop fast path.
///
/// When `effects.may_share == false` (pure functions, simple accessors),
/// the callee provably doesn't create shared references, so borrowed args
/// preserve uniqueness and can safely use `drop_unique`.
///
/// Also propagates through Let alias chains: if `%x` is borrowed in a call
/// and `%y = %x`, then `%y` is also excluded.
pub(crate) fn collect_borrowed_call_args(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    builtins: &BuiltinOwnershipSets,
) -> FxHashSet<ArcVarId> {
    let mut borrowed = FxHashSet::default();
    let mut alias_edges: Vec<Option<ArcVarId>> = vec![None; func.var_types.len()];

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
                    // Builtin contracts are not trusted for this because their
                    // runtime implementations may do hidden RC ops.
                    if !is_safe_non_sharing_callee(*callee, contracts, builtins) {
                        for (i, &arg) in args.iter().enumerate() {
                            if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                                borrowed.insert(arg);
                            }
                        }
                    }
                }
                ArcInstr::Let {
                    dst,
                    value: crate::ir::ArcValue::Var(src),
                    ..
                } => {
                    alias_edges[dst.index()] = Some(*src);
                }
                _ => {}
            }
        }

        // Scan terminator for Invoke with Borrowed args.
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            if !is_safe_non_sharing_callee(*callee, contracts, builtins) {
                for (i, &arg) in args.iter().enumerate() {
                    if arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed) {
                        borrowed.insert(arg);
                    }
                }
            }
        }
    }

    // Propagate through alias chains: if src is borrowed, dst shares the
    // same object and should also be excluded from unique drops.
    loop {
        let mut changed = false;
        for (i, alias) in alias_edges.iter().enumerate() {
            if let Some(src) = alias {
                if borrowed.contains(src) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "ARC IR var counts fit in u32"
                    )]
                    let dst = ArcVarId::new(i as u32);
                    if borrowed.insert(dst) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    borrowed
}

/// Check if a callee is a user function proven to not share references.
///
/// Returns `true` only for user-analyzed functions (not builtins) where
/// `effects.may_share == false`. Builtin contracts are not trusted here
/// because runtime implementations may do hidden `ori_rc_inc` ops
/// (e.g., `ori_list_slice` increments the backing buffer's refcount).
fn is_safe_non_sharing_callee(
    callee: Name,
    contracts: &FxHashMap<Name, MemoryContract>,
    builtins: &BuiltinOwnershipSets,
) -> bool {
    // If the callee is a known builtin, always conservative.
    if builtins.is_builtin(callee) {
        return false;
    }
    // For user functions: trust the contract's may_share flag.
    contracts.get(&callee).is_some_and(|c| !c.effects.may_share)
}
