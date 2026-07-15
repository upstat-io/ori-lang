//! Argument ownership annotation for call-site instructions.
//!
//! Populates the `arg_ownership` field on `Apply`, `Invoke`,
//! `ApplyIndirect`, and `InvokeIndirect` instructions before RC insertion,
//! so that the Perceus pass can distinguish borrowed vs owned arguments
//! without re-deriving callee signatures.
//!
//! Both the legacy pipeline and the AIMS pipeline call
//! [`annotate_arg_ownership`] — the AIMS path supplies `AnnotatedSig`s
//! converted from `MemoryContract`s (see `aims::emit_rc::arg_ownership`).
//! The builtin override logic (borrowing receivers, COW consuming methods,
//! protocol builtins) runs in both cases.
//!
//! Indirect-call explicit arguments always use the borrowed closure ABI.
//! The concrete closure adapter retains exactly the target parameters whose
//! frozen ownership is `Owned`; callers therefore never guess a dynamic
//! target's ownership contract.

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
/// receiver also get all-borrowed. Their shared semantic contract has no
/// argument ownership transfer; physical consumers implement that contract
/// directly (the LLVM emitter currently compiles them inline).
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
    let callee_name = interner
        .try_lookup(callee)
        .map_or_else(|| "?".to_string(), std::string::ToString::to_string);
    tracing::debug!(
        target: "ori_arc::rc_insert::annotate",
        callee = callee_name.as_str(),
        in_sigs = sigs.contains_key(&callee),
        sig_param_ownership = ?sigs.get(&callee).map(|s| {
            s.params.iter().map(|p| p.ownership).collect::<Vec<_>>()
        }),
        arg_count,
        "arg_ownership verdict"
    );
    // External C runtime: not in sigs, name starts with `ori_`.
    if !sigs.contains_key(&callee) {
        // Protocol builtins with explicit per-arg ownership — check FIRST.
        // Uses the ProtocolBuiltin::arg_ownership() table as source of truth.
        // Must precede the `ori_` prefix check because `ori_iter_drop` starts
        // with `ori_` but is a protocol builtin with Owned semantics, not an
        // external C runtime function (which defaults to all-Borrowed).
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

/// Populate `arg_ownership` on all `Apply`, `Invoke`, `ApplyIndirect`,
/// and `InvokeIndirect` instructions.
///
/// This is the single point where external-callee detection and per-param
/// ownership lookup happen. All downstream passes read from the field.
/// Called by AIMS pipeline step 4 (`emit_arg_ownership`).
///
/// Indirect calls use one uniform borrowed ABI for every explicit argument.
/// The closure adapter owns the target-specific retain bridge for both known
/// and opaque closure provenance.
///
/// `borrowing_builtins` identifies builtin method names (e.g., `len`,
/// `is_empty`) whose receiver is always borrowed. Their arguments are marked
/// Borrowed because the shared builtin contract performs no ownership
/// transfer, so the caller retains cleanup responsibility and inserts
/// `RcDec` at the argument's last use. LLVM inlining is one physical
/// implementation of that contract.
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
/// `builtins.consuming_third_arg` identifies COW methods (`updated`) where
/// the runtime moves the third argument (the inserted value) into the
/// collection. When the receiver is a collection and `args.len() >= 3`,
/// arg[2] is also `Owned`.
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
    annotate_arg_ownership_with_exact_callables(
        func,
        sigs,
        interner,
        builtins,
        pool,
        &FxHashSet::default(),
    );
}

/// Populate argument ownership while protecting exact local/imported callable
/// identities from same-spelled builtin method policy.
pub(crate) fn annotate_arg_ownership_with_exact_callables(
    func: &mut ArcFunction,
    sigs: &rustc_hash::FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    builtins: &crate::BuiltinOwnershipSets,
    pool: &Pool,
    exact_callables: &FxHashSet<ori_ir::Name>,
) {
    let consuming_ctx = ConsumingCtx {
        consuming_receiver_builtins: &builtins.consuming_receiver,
        consuming_second_arg_builtins: &builtins.consuming_second_arg,
        consuming_third_arg_builtins: &builtins.consuming_third_arg,
        var_types: &func.var_types,
        pool,
        zip_name: interner.intern("zip"),
        chain_name: interner.intern("chain"),
        pop_name: interner.intern("pop"),
    };

    for block in &mut func.blocks {
        // Annotate body instructions.
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
                        &builtins.consuming_receiver_only,
                        &builtins.protocol,
                    );
                    if !exact_callables.contains(callee) {
                        apply_consuming_overrides(*callee, args, arg_ownership, &consuming_ctx);
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

        // Annotate terminator.
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
                    &builtins.consuming_receiver_only,
                    &builtins.protocol,
                );
                if !exact_callables.contains(callee) {
                    apply_consuming_overrides(*callee, args, arg_ownership, &consuming_ctx);
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

/// Override borrowing ownership for COW list methods with consuming semantics.
///
/// Applies three overrides for collection-typed receivers:
/// 1. **Receiver** (arg[0]): When `callee` is in `consuming_receiver_builtins`
///    and the receiver is `List`/`Map`/`Set`, marks arg[0] as `Owned`.
/// 2. **Second arg** (arg[1]): When `callee` is *also* in
///    `consuming_second_arg_builtins` and the receiver is `List`/`Map`/`Set`,
///    marks arg[1] as `Owned` too — the runtime consumes the second operand's buffer.
/// 3. **Third arg** (arg[2]): When `callee` is *also* in
///    `consuming_third_arg_builtins` (`updated`), marks arg[2] as `Owned` —
///    the runtime moves the inserted value into the collection.
///
/// Type-qualified: `"add"` and `"concat"` are shared names — borrowing for
/// strings, consuming for lists. Only the list case is overridden here.
/// Context for [`apply_consuming_overrides`].
struct ConsumingCtx<'a> {
    consuming_receiver_builtins: &'a FxHashSet<ori_ir::Name>,
    consuming_second_arg_builtins: &'a FxHashSet<ori_ir::Name>,
    consuming_third_arg_builtins: &'a FxHashSet<ori_ir::Name>,
    var_types: &'a [ori_types::Idx],
    pool: &'a Pool,
    /// Pre-interned method names for identity comparison.
    zip_name: ori_ir::Name,
    chain_name: ori_ir::Name,
    pop_name: ori_ir::Name,
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

    let receiver_var = args[0];
    let receiver_idx = ctx.var_types[receiver_var.index()];
    let resolved_receiver = ctx.pool.resolve_fully(receiver_idx);
    let tag = ctx.pool.tag(resolved_receiver);

    // Iterator receivers — every user-facing iterator method
    // (adapters and consumers) consumes the receiver via
    // `Box::from_raw(iter.cast::<IterState>())`. This is true for
    // `Iterator<T>` AND `DoubleEndedIterator<T>`. The registry records
    // this with `Ownership::Owned` on iterator methods, but the
    // borrowing-builtins path still classifies calls as borrowing
    // because `count`/`map`/`filter` are also List methods with
    // `Ownership::Borrow`, and the name-based lookup in
    // `compute_arg_ownership` can't disambiguate by receiver type.
    //
    // This type-qualified override is the disambiguation point: if the
    // receiver is an iterator, the call is always consuming. Protocol
    // builtins (`__iter_next`, `__collect_set`) bypass this function
    // entirely — they go through `compute_arg_ownership`'s protocol
    // path, which already has per-arg ownership from
    // `ProtocolBuiltin::arg_ownership()`.
    if matches!(
        tag,
        ori_types::Tag::Iterator | ori_types::Tag::DoubleEndedIterator
    ) {
        arg_ownership[0] = ArgOwnership::Owned;
        // zip/chain take a second iterator — also consume it.
        if (callee == ctx.zip_name || callee == ctx.chain_name)
            && args.len() >= 2
            && arg_ownership.len() >= 2
        {
            let other_var = args[1];
            let other_idx = ctx.var_types[other_var.index()];
            let other_tag = ctx.pool.tag(ctx.pool.resolve_fully(other_idx));
            if matches!(
                other_tag,
                ori_types::Tag::Iterator | ori_types::Tag::DoubleEndedIterator
            ) {
                arg_ownership[1] = ArgOwnership::Owned;
            }
        }
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

    if !matches!(
        tag,
        ori_types::Tag::List | ori_types::Tag::Map | ori_types::Tag::Set
    ) {
        return;
    }

    // pop() is currently implemented as read-only (returns last element
    // without mutating). Keep as Borrowed until full COW pop is implemented.
    if callee == ctx.pop_name {
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

    // Also mark third arg as Owned if this method moves the inserted value
    // into the collection (`updated(key, value)` — IndexSet).
    if ctx.consuming_third_arg_builtins.contains(&callee)
        && args.len() >= 3
        && arg_ownership.len() >= 3
    {
        arg_ownership[2] = ArgOwnership::Owned;
    }
}
