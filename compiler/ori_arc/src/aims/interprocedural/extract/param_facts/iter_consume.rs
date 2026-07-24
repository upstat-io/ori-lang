//! Whole-param and field-grained iter-consume detection (RL-2 inward-transfer
//! facts).

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::super::super::super::contract::MemoryContract;

/// Find params that are iter-consumed-and-freed by the callee body (RL-2
/// iter-consume transfer).
///
/// A param qualifies when its lineage (through the `alias_to_param` map, which
/// already threads Let-Var / Jump-arg / Select aliasing back to param indices)
/// is passed at the source position of an `@iter` (`Iter`-protocol) `Apply`
/// whose resulting iterator handle is later consumed by an `ori_iter_drop`
/// (`IterDrop`-protocol) `Apply` in the same body. The `for x in coll` lowering
/// produces exactly this shape: `%a = Let Var(param); %it = Apply @iter(%a
/// [own]); ...; Apply @ori_iter_drop(%it [own])`.
///
/// PRECISION (the over-fire guard): the gate
/// requires BOTH the `@iter` source AND a matching `ori_iter_drop` of that
/// handle, matching the explicit `@iter`→`ori_iter_drop` handle pair (not merely
/// "flows to any iterator") so the fire is scoped to the genuine free. A
/// borrow-read callee (`xs.fold(..)`) routes its iterator to a fold/collect that
/// frees the handle WITHOUT freeing the borrowed source via this `@iter`+drop
/// pair; the discriminator is the `for ... do/yield` lowering of a BORROWED
/// collection into the iterator source consumed by `ori_iter_drop`.
pub(in crate::aims::interprocedural::extract) fn find_iter_consume_params(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<usize> {
    let call_args = find_iter_consume_call_args(func, sigs, interner, None);
    let mut iter_consumed = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::Apply { dst, args, .. } = instr else {
                continue;
            };
            absorb_iter_call_params(*dst, args, &call_args, alias_to_param, &mut iter_consumed);
        }
        if let ArcTerminator::Invoke { dst, args, .. } = &block.terminator {
            absorb_iter_call_params(*dst, args, &call_args, alias_to_param, &mut iter_consumed);
        }
    }

    tracing::debug!(
        target: "ori_arc::aims::interprocedural",
        fn_name = interner.lookup(func.name),
        ?iter_consumed,
        "find_iter_consume_params verdict"
    );

    iter_consumed
}

/// Return call-destination to iter-consuming argument positions. Direct
/// `@iter` evidence requires the matching dropped handle; transitive evidence
/// comes from another callee's parameter contract. `excluded_callee` lets the
/// oracle suppress the subject contract while extraction retains SCC behavior.
pub(crate) fn find_iter_consume_call_args(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    excluded_callee: Option<Name>,
) -> FxHashMap<ArcVarId, FxHashSet<usize>> {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let iter_name = interner.intern(ProtocolBuiltin::Iter.name());
    let iter_drop_name = interner.intern(ProtocolBuiltin::IterDrop.name());

    // Pass 1: collect iterator handles that are `ori_iter_drop`'d (freed).
    let mut dropped_handles: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply { func: f, args, .. } = instr {
                if *f == iter_drop_name {
                    if let Some(&handle) = args.first() {
                        dropped_handles.insert(handle);
                    }
                }
            }
        }
    }

    let mut call_args: FxHashMap<ArcVarId, FxHashSet<usize>> = FxHashMap::default();

    // Pass 2 (DIRECT): an `@iter` whose dst handle is dropped consumes arg 0.
    if !dropped_handles.is_empty() {
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Apply {
                    dst, func: f, args, ..
                } = instr
                {
                    if *f == iter_name && dropped_handles.contains(dst) && !args.is_empty() {
                        call_args.entry(*dst).or_default().insert(0);
                    }
                }
            }
        }
    }

    // Pass 3 (TRANSITIVE forwarding): a forwarding wrapper that passes a param's
    // alias to a callee whose CORRESPONDING param `iter_consumes` is itself an
    // iter-consumer of that param (`@wrapper(words)` -> `iterate_words(words)`
    // where `iterate_words.iter_consumes[0]`). The buffer is freed by the
    // forwarded callee's iterator machinery, so the wrapper transfers ownership
    // inward exactly as a direct iter-consumer does (RL-2 inward transfer). The
    // SCC fixpoint propagates this: `iterate_words`'s contract (computed first,
    // callees-before-callers per IC-1) carries `iter_consumes`, which this pass
    // reads from `sigs` to mark `wrapper`'s param. Borrow-read forwarders
    // (`@sum_list(xs)` -> `xs.fold(..)`) do NOT qualify — `fold`'s param is not
    // `iter_consumes` (it borrows, no `@iter`->`ori_iter_drop`), so the
    // forwarder's param stays non-iter-consuming (the borrow-read over-fire
    // boundary). Spec: Annex E §AIMS RL-2.
    let mut scan_forward = |dst: ArcVarId, callee: Name, args: &[ArcVarId]| {
        if excluded_callee == Some(callee) {
            return;
        }
        let Some(callee_contract) = sigs.get(&callee) else {
            return;
        };
        for (pos, _) in args.iter().enumerate() {
            if !callee_contract
                .params
                .get(pos)
                .is_some_and(|p| p.iter_consumes)
            {
                continue;
            }
            call_args.entry(dst).or_default().insert(pos);
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                scan_forward(*dst, *callee, args);
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            scan_forward(*dst, *callee, args);
        }
    }
    call_args
}

fn absorb_iter_call_params(
    dst: ArcVarId,
    args: &[ArcVarId],
    call_args: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    iter_consumed: &mut FxHashSet<usize>,
) {
    let Some(positions) = call_args.get(&dst) else {
        return;
    };
    for &position in positions {
        let Some(arg) = args.get(position) else {
            continue;
        };
        if let Some(param_indices) = alias_to_param.get(arg) {
            iter_consumed.extend(param_indices);
        }
    }
}
