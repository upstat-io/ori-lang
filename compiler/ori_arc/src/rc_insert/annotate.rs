//! Argument ownership annotation for Apply/Invoke instructions.
//!
//! Populates the `arg_ownership` field on call-site instructions before
//! RC insertion, so that the Perceus pass can distinguish borrowed vs owned
//! arguments without re-deriving callee signatures.
//!
//! Both the legacy pipeline and the AIMS pipeline call
//! [`annotate_arg_ownership`] — the AIMS path supplies `AnnotatedSig`s
//! converted from `MemoryContract`s (see `aims::emit_rc::arg_ownership`).
//! The builtin override logic (borrowing receivers, COW consuming methods,
//! protocol builtins) runs in both cases.

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
        // COW method consuming only the receiver (e.g., remove, union, insert
        // for map/set). Non-receiver args are borrowed (comparison keys,
        // read-only sets). Must be checked BEFORE borrowing_builtins because
        // the registry marks these methods as borrowing (correct for type
        // checking) but the runtime has consuming semantics (takes ownership
        // of the receiver buffer).
        if consuming_receiver_only.contains(&callee) && arg_count > 0 {
            let mut ownership = vec![ArgOwnership::Borrowed; arg_count];
            ownership[0] = ArgOwnership::Owned;
            return ownership;
        }
        // Builtin method with borrowing receiver (e.g., len, is_empty).
        if borrowing_builtins.contains(&callee) {
            return vec![ArgOwnership::Borrowed; arg_count];
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
    // Missing params default to Borrowed (safe: caller retains cleanup).
    // This handles builtins seeded with fewer params than the call's arity
    // (e.g., `equals` seeded with 1 param but called with 2 args).
    if let Some(sig) = sigs.get(&callee) {
        return (0..arg_count)
            .map(|i| match sig.params.get(i) {
                Some(p) if p.ownership == Ownership::Owned => ArgOwnership::Owned,
                _ => ArgOwnership::Borrowed,
            })
            .collect();
    }

    vec![ArgOwnership::Owned; arg_count]
}

/// Populate `arg_ownership` on all `Apply` and `Invoke` instructions.
///
/// This is the single point where external-callee detection and per-param
/// ownership lookup happen. All downstream passes read from the field.
/// Called by AIMS pipeline step 4 (`emit_arg_ownership`).
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
    sigs: &rustc_hash::FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    builtins: &crate::BuiltinOwnershipSets,
    pool: &Pool,
) {
    let consuming_ctx = ConsumingCtx {
        consuming_receiver_builtins: &builtins.consuming_receiver,
        consuming_second_arg_builtins: &builtins.consuming_second_arg,
        var_types: &func.var_types,
        pool,
        interner,
    };

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
                apply_consuming_overrides(*callee, args, arg_ownership, &consuming_ctx);
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
            apply_consuming_overrides(*callee, args, arg_ownership, &consuming_ctx);
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
/// Context for [`apply_consuming_overrides`].
struct ConsumingCtx<'a> {
    consuming_receiver_builtins: &'a FxHashSet<ori_ir::Name>,
    consuming_second_arg_builtins: &'a FxHashSet<ori_ir::Name>,
    var_types: &'a [ori_types::Idx],
    pool: &'a Pool,
    interner: &'a ori_ir::StringInterner,
}

fn apply_consuming_overrides(
    callee: ori_ir::Name,
    args: &[ArcVarId],
    arg_ownership: &mut [ArgOwnership],
    ctx: &ConsumingCtx<'_>,
) {
    if args.is_empty() || arg_ownership.is_empty() {
        return;
    }

    // Check if the receiver is a collection (type-qualified gate).
    // COW methods consume the receiver for List, Map, and Set types.
    // Str is excluded — str builtins (iter, concat) borrow the receiver
    // because the runtime Inc's string data internally.
    let is_receiver_consuming = ctx.consuming_receiver_builtins.contains(&callee);
    if !is_receiver_consuming {
        return;
    }

    let receiver_var = args[0];
    let receiver_idx = ctx.var_types[receiver_var.index()];
    let resolved_receiver = ctx.pool.resolve_fully(receiver_idx);
    let tag = ctx.pool.tag(resolved_receiver);
    if !matches!(
        tag,
        ori_types::Tag::List | ori_types::Tag::Map | ori_types::Tag::Set
    ) {
        return;
    }

    // pop() is currently implemented as read-only (returns last element
    // without mutating). Keep as Borrowed until full COW pop is implemented.
    let callee_str = ctx.interner.lookup(callee);
    if callee_str == "pop" {
        return;
    }

    // Receiver is a collection (List/Map/Set) — mark it as Owned.
    arg_ownership[0] = ArgOwnership::Owned;

    // Also mark second arg as Owned if this method consumes list2.
    if ctx.consuming_second_arg_builtins.contains(&callee)
        && args.len() >= 2
        && arg_ownership.len() >= 2
    {
        arg_ownership[1] = ArgOwnership::Owned;
    }
}
