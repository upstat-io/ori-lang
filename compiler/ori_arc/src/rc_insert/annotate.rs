//! Argument ownership annotation for Apply/Invoke instructions.
//!
//! Populates the `arg_ownership` field on call-site instructions before
//! RC insertion, so that the Perceus pass can distinguish borrowed vs owned
//! arguments without re-deriving callee signatures.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::builtin_constants::protocol::ProtocolArgOwnership;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::ownership::Ownership;
use ori_types::Pool;

/// Compute per-argument ownership for a single call.
///
/// Determines whether each argument is borrowed (callee reads without
/// consuming) or owned (ownership transfers to callee). Uses the callee's
/// `AnnotatedSig` when available, falls back to all-owned for unknown callees.
///
/// External callees (C runtime `ori_*` functions not in the `sigs` map) get
/// all-borrowed — they don't participate in Perceus ownership.
///
/// Builtin methods (e.g., `len`, `is_empty`) that are known to borrow their
/// receiver also get all-borrowed — they're compiled inline by the LLVM
/// emitter and don't consume their arguments.
///
/// COW methods that consume only the receiver (e.g., `remove`, `union`) get
/// `[Owned, Borrowed, ...]` — the receiver is consumed by the COW runtime,
/// but other arguments (comparison keys, read-only collections) are borrowed.
fn compute_arg_ownership(
    callee: ori_ir::Name,
    arg_count: usize,
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    borrowing_builtins: &FxHashSet<ori_ir::Name>,
    consuming_receiver_only: &FxHashSet<ori_ir::Name>,
    protocol_builtins: &FxHashMap<ori_ir::Name, &'static [ProtocolArgOwnership]>,
) -> Vec<ArgOwnership> {
    // External C runtime: not in sigs, name starts with `ori_`.
    if !sigs.contains_key(&callee) {
        if interner
            .try_lookup(callee)
            .is_some_and(|name_str| name_str.starts_with("ori_"))
        {
            return vec![ArgOwnership::Borrowed; arg_count];
        }
        // Builtin method with borrowing receiver (e.g., len, is_empty).
        if borrowing_builtins.contains(&callee) {
            return vec![ArgOwnership::Borrowed; arg_count];
        }
        // COW method consuming only the receiver (e.g., remove, union).
        // Non-receiver args are borrowed (comparison keys, read-only sets).
        if consuming_receiver_only.contains(&callee) && arg_count > 0 {
            let mut ownership = vec![ArgOwnership::Borrowed; arg_count];
            ownership[0] = ArgOwnership::Owned;
            return ownership;
        }
        // Protocol builtins with explicit per-arg ownership.
        // Uses the ProtocolBuiltin::arg_ownership() table as source of truth.
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
                .map(|o| match o {
                    ProtocolArgOwnership::Owned => ArgOwnership::Owned,
                    ProtocolArgOwnership::Borrowed => ArgOwnership::Borrowed,
                })
                .collect();
        }
        // Unknown non-external: conservative owned.
        return vec![ArgOwnership::Owned; arg_count];
    }

    // Ori function with known signature: per-param ownership.
    if let Some(sig) = sigs.get(&callee) {
        return (0..arg_count)
            .map(|i| {
                if sig
                    .params
                    .get(i)
                    .is_some_and(|p| p.ownership == Ownership::Borrowed)
                {
                    ArgOwnership::Borrowed
                } else {
                    ArgOwnership::Owned
                }
            })
            .collect();
    }

    vec![ArgOwnership::Owned; arg_count]
}

/// Populate `arg_ownership` on all `Apply` and `Invoke` instructions.
///
/// Must be called **before** [`insert_rc_ops_with_ownership`](super::insert::insert_rc_ops_with_ownership)
/// so that the RC insertion pass can read ownership from the IR instead of
/// re-deriving it from sigs + interner.
///
/// This is the single point where external-callee detection and per-param
/// ownership lookup happen. All downstream passes read from the field.
///
/// `borrowing_builtins` identifies builtin method names (e.g., `len`,
/// `is_empty`) whose receiver is always borrowed. These are compiled inline
/// by the LLVM emitter — their args must be marked Borrowed so that the
/// caller retains ownership and inserts `RcDec` at the arg's last use.
///
/// `builtins.consuming_receiver` identifies COW list methods (e.g., `push`,
/// `reverse`) where the receiver's RC is managed internally by the runtime.
/// When the receiver type is `List`, the first arg is marked `Owned` to
/// prevent the ARC pipeline from emitting a duplicate `RcDec`.
///
/// `builtins.consuming_second_arg` identifies COW list methods (e.g., `add`,
/// `concat`) where the runtime also consumes the second argument (list2).
/// When the receiver is `List` and `args.len() >= 2`, arg[1] is also `Owned`.
///
/// `builtins.consuming_receiver_only` identifies COW methods (e.g., `remove`,
/// `union`) that consume the receiver but only read/compare other arguments.
/// These produce `[Owned, Borrowed, ...]` in `compute_arg_ownership`.
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn annotate_arg_ownership(
    func: &mut ArcFunction,
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    builtins: &crate::BuiltinOwnershipSets,
    pool: &Pool,
) {
    // Split borrow: var_types is read-only, blocks are mutated.
    let var_types = &func.var_types;

    for block in &mut func.blocks {
        // Annotate body instructions.
        for instr in &mut block.body {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                *arg_ownership = compute_arg_ownership(
                    *callee,
                    args.len(),
                    sigs,
                    interner,
                    &builtins.borrowing,
                    &builtins.consuming_receiver_only,
                    &builtins.protocol,
                );
                apply_consuming_overrides(
                    *callee,
                    args,
                    arg_ownership,
                    &builtins.consuming_receiver,
                    &builtins.consuming_second_arg,
                    var_types,
                    pool,
                );
            }
        }

        // Annotate terminator.
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &mut block.terminator
        {
            *arg_ownership = compute_arg_ownership(
                *callee,
                args.len(),
                sigs,
                interner,
                &builtins.borrowing,
                &builtins.consuming_receiver_only,
                &builtins.protocol,
            );
            apply_consuming_overrides(
                *callee,
                args,
                arg_ownership,
                &builtins.consuming_receiver,
                &builtins.consuming_second_arg,
                var_types,
                pool,
            );
        }
    }
}

/// Override borrowing ownership for COW list methods with consuming semantics.
///
/// Applies two overrides for list-typed receivers:
/// 1. **Receiver** (arg[0]): When `callee` is in `consuming_receiver_builtins`
///    and the receiver is `List`, marks arg[0] as `Owned`.
/// 2. **Second arg** (arg[1]): When `callee` is *also* in
///    `consuming_second_arg_builtins` and the receiver is `List`, marks
///    arg[1] as `Owned` too — the runtime consumes list2's buffer.
///
/// Type-qualified: `"add"` and `"concat"` are shared names — borrowing for
/// strings, consuming for lists. Only the list case is overridden here.
fn apply_consuming_overrides(
    callee: ori_ir::Name,
    args: &[ArcVarId],
    arg_ownership: &mut [ArgOwnership],
    consuming_receiver_builtins: &FxHashSet<ori_ir::Name>,
    consuming_second_arg_builtins: &FxHashSet<ori_ir::Name>,
    var_types: &[ori_types::Idx],
    pool: &Pool,
) {
    if args.is_empty() || arg_ownership.is_empty() {
        return;
    }

    // Check if the receiver is a list (type-qualified gate).
    let is_receiver_consuming = consuming_receiver_builtins.contains(&callee);
    if !is_receiver_consuming {
        return;
    }

    let receiver_var = args[0];
    let receiver_idx = var_types[receiver_var.index()];
    let resolved_receiver = pool.resolve_fully(receiver_idx);
    if pool.tag(resolved_receiver) != ori_types::Tag::List {
        return;
    }

    // Receiver is List — mark it as Owned.
    arg_ownership[0] = ArgOwnership::Owned;

    // Also mark second arg as Owned if this method consumes list2.
    if consuming_second_arg_builtins.contains(&callee)
        && args.len() >= 2
        && arg_ownership.len() >= 2
    {
        arg_ownership[1] = ArgOwnership::Owned;
    }
}
