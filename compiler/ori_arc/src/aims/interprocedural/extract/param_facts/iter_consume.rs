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
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let iter_name = interner.intern(ProtocolBuiltin::Iter.name());
    let iter_drop_name = interner.intern(ProtocolBuiltin::IterDrop.name());

    let mut iter_consumed: FxHashSet<usize> = FxHashSet::default();

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

    // Pass 2 (DIRECT): an `@iter` whose dst handle is dropped AND whose source
    // (arg 0, the collection being iterated) aliases a param marks that param
    // iter-consumed.
    if !dropped_handles.is_empty() {
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Apply {
                    dst, func: f, args, ..
                } = instr
                {
                    if *f == iter_name && dropped_handles.contains(dst) {
                        if let Some(&src) = args.first() {
                            if let Some(param_indices) = alias_to_param.get(&src) {
                                for &i in param_indices {
                                    iter_consumed.insert(i);
                                }
                            }
                        }
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
    let scan_forward = |callee: Name, args: &[ArcVarId], iter_consumed: &mut FxHashSet<usize>| {
        let Some(callee_contract) = sigs.get(&callee) else {
            return;
        };
        for (pos, &arg) in args.iter().enumerate() {
            if !callee_contract
                .params
                .get(pos)
                .is_some_and(|p| p.iter_consumes)
            {
                continue;
            }
            if let Some(param_indices) = alias_to_param.get(&arg) {
                for &i in param_indices {
                    iter_consumed.insert(i);
                }
            }
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                scan_forward(*callee, args, &mut iter_consumed);
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            scan_forward(*callee, args, &mut iter_consumed);
        }
    }

    tracing::debug!(
        target: "ori_arc::aims::interprocedural",
        fn_name = interner.lookup(func.name),
        ?iter_consumed,
        dropped_handle_count = dropped_handles.len(),
        "find_iter_consume_params verdict"
    );

    iter_consumed
}

/// Find the field-grained iter-consume record per param (RL-2 iter-consume
/// inward-transfer of a PROJECTED FIELD of a borrowed aggregate param).
///
/// Distinct from `find_iter_consume_params` (which handles the param-IS-the-
/// collection case): here the param is a borrowed AGGREGATE and a `Project
/// param.field` projection is iter-consumed (`@iter [own]` → `ori_iter_drop`
/// frees the projected collection inside the callee). The `for item in c.items`
/// lowering over a borrowed struct param `c` produces `%p = Project c.0; %it =
/// Apply @iter(%p [own]); ...; Apply @ori_iter_drop(%it [own])`.
///
/// Returns `param_index → field` for every param with EXACTLY ONE such consumed
/// projected field that is NOT the param itself (the whole-param case is
/// `find_iter_consume_params`). Over-fire guard: a param whose >1 distinct
/// fields are iter-consumed, or whose consumed field is also iter-consumed at
/// the whole-param grain, poisons to absent (no field-grained claim) — the
/// caller then keeps the whole-aggregate dec (a leak at worst, never a
/// double-free of a non-consumed field). Spec: Annex E §AIMS RL-2.
pub(in crate::aims::interprocedural::extract) fn find_aggregate_iter_consume_fields(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) -> FxHashMap<usize, u32> {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let iter_name = interner.intern(ProtocolBuiltin::Iter.name());
    let iter_drop_name = interner.intern(ProtocolBuiltin::IterDrop.name());

    // Map each param var to its index. The caller-side consumer only fires when
    // the matching arg is passed at a BORROWED Invoke position, so the consume's
    // inward-transfer is already gated there; the detection itself covers every
    // param whose projected field is `@iter`-consumed.
    let param_index: FxHashMap<ArcVarId, usize> = func
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| (p.var, i))
        .collect();
    if param_index.is_empty() {
        return FxHashMap::default();
    }

    // Direct `Project param.field` definitions: dst → (param_index, field). A
    // borrowed aggregate param projected exactly once per field. Let-Var aliases
    // of the param are also valid Project sources, so resolve a one-hop Let-Var
    // alias back to the param.
    let mut let_alias_of_param: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if let Some(&pi) = param_index.get(src) {
                    let_alias_of_param.insert(*dst, pi);
                }
            }
        }
    }
    let resolve_param = |v: ArcVarId| -> Option<usize> {
        param_index
            .get(&v)
            .copied()
            .or_else(|| let_alias_of_param.get(&v).copied())
    };

    // project_dst → (param_index, field)
    let mut project_of_param: FxHashMap<ArcVarId, (usize, u32)> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project {
                dst, value, field, ..
            } = instr
            {
                if let Some(pi) = resolve_param(*value) {
                    project_of_param.insert(*dst, (pi, *field));
                }
            }
        }
    }
    if project_of_param.is_empty() {
        return FxHashMap::default();
    }

    // Pass 1: iterator handles freed by `ori_iter_drop`.
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
    if dropped_handles.is_empty() {
        return FxHashMap::default();
    }

    // Pass 2: an `@iter` whose dst handle is dropped AND whose source is a
    // `Project param.field` records (param, field) as a field-grained consume.
    // Accumulate the consumed-field SET per param; only a UNIQUE field survives.
    let mut consumed: FxHashMap<usize, FxHashSet<u32>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: f, args, ..
            } = instr
            {
                if *f == iter_name && dropped_handles.contains(dst) {
                    if let Some(&src) = args.first() {
                        if let Some(&(pi, field)) = project_of_param.get(&src) {
                            consumed.entry(pi).or_default().insert(field);
                        }
                    }
                }
            }
        }
    }

    // Keep only params with EXACTLY ONE consumed projected field.
    consumed
        .into_iter()
        .filter_map(|(pi, fields)| {
            if fields.len() == 1 {
                fields.into_iter().next().map(|f| (pi, f))
            } else {
                None
            }
        })
        .collect()
}
