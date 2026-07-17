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
use smallvec::SmallVec;

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
/// Type-qualified collection and iterator ownership is applied after this
/// baseline by
/// [`crate::BuiltinOwnershipSets::type_qualified_consuming_positions`].
fn compute_arg_ownership(
    callee: ori_ir::Name,
    arg_count: usize,
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    borrowing_builtins: &FxHashSet<ori_ir::Name>,
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
/// `builtins.borrowing` identifies builtin method names (e.g., `len`,
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
/// The shared type-qualified authority produces `[Owned, Borrowed, ...]`.
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
    let var_types = &func.var_types;

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

/// Apply the shared type-qualified builtin consumption authority.
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
