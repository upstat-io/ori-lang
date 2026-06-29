//! Per-parameter fact detection: iter-consume transfer and
//! borrowed-read-only forwarding safety.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::super::super::contract::MemoryContract;
use super::super::super::lattice::AccessClass;

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
pub(super) fn find_iter_consume_params(
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
pub(super) fn find_aggregate_iter_consume_fields(
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

/// Find non-scalar params that flow ONLY to BORROWED positions in the callee
/// body: no owned-position consumer (no COW-mutation, no transfer, no
/// iter-consume). The affirmative read-only complement consumed by the caller
/// carve-out gate (`compute_user_call_arg_lineages`): a Borrowed collection
/// passed to such a param SURVIVES the call (`ApplyToBorrowedParam`, RL-2
/// NON-transfer, caller decs).
///
/// A param is read-only iff EVERY occurrence of its alias (via `alias_to_param`)
/// as an `Apply`/`Invoke` arg sits at a BORROWED arg position. "Owned position"
/// and forward-safety are decided by `borrowed_ro_arg_is_owned_position` /
/// `borrowed_ro_arg_forward_safe`, mirroring `compute_arg_ownership`. The COW
/// `xs.push(v)` (`@push` receiver, pos 0) or an iter-consume (`@iter [own]`)
/// clears the fact; pure builtin reads (`@len`/`@length`/`@__index`) leave it.
/// Params not appearing as any call arg are NOT read-only by this fact (the
/// caller gate handles those lineages via its other exclusions).
/// Spec: Annex E §AIMS RL-2 (`RL2_borrowed_param_emits_caller_dec`).
pub(super) fn find_borrowed_read_only_params(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<usize> {
    // The COW-builtin ownership sets are a pure function of the interner; the
    // ownership authority here matches `compute_arg_ownership`. Constructed
    // locally (not threaded) to keep the contract-extraction surface stable.
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);

    // Params whose alias appears at an OWNED or unsafe-forward position (NOT
    // read-only), and params that appear as SOME call arg (gate-eligible).
    let mut not_read_only: FxHashSet<usize> = FxHashSet::default();
    let mut used_as_call_arg: FxHashSet<usize> = FxHashSet::default();

    let mut scan = |callee: Option<Name>, args: &[ArcVarId]| {
        for (pos, &arg) in args.iter().enumerate() {
            let Some(param_indices) = alias_to_param.get(&arg) else {
                continue;
            };
            used_as_call_arg.extend(param_indices.iter().copied());
            // Indirect / unknown callee (`None`): conservatively clears read-only.
            let clears = match callee {
                None => true,
                Some(name) => {
                    borrowed_ro_arg_is_owned_position(name, pos, &builtins, sigs)
                        || !borrowed_ro_arg_forward_safe(name, pos, &builtins, sigs, interner)
                }
            };
            if clears {
                not_read_only.extend(param_indices.iter().copied());
            }
        }
    };

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    func: callee, args, ..
                } => scan(Some(*callee), args),
                ArcInstr::ApplyIndirect { args, .. } => scan(None, args),
                _ => {}
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke {
                func: callee, args, ..
            } => scan(Some(*callee), args),
            ArcTerminator::InvokeIndirect { args, .. } => scan(None, args),
            _ => {}
        }
    }

    // Read-only iff it reaches some call arg AND never an owned/unsafe position.
    used_as_call_arg
        .difference(&not_read_only)
        .copied()
        .collect()
}

/// Does `callee` consume its `pos`-th arg at an OWNED position? Mirrors
/// `compute_arg_ownership`: COW receiver / second-arg, protocol owned positions,
/// or a user-fn param contract `access == Owned`. Unknown non-builtin callees are
/// conservatively Owned. Companion to [`find_borrowed_read_only_params`].
fn borrowed_ro_arg_is_owned_position(
    callee: Name,
    pos: usize,
    builtins: &crate::BuiltinOwnershipSets,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> bool {
    if builtins.consuming_receiver.contains(&callee)
        || builtins.consuming_receiver_only.contains(&callee)
    {
        if pos == 0 {
            return true;
        }
        if pos == 1 && builtins.consuming_second_arg.contains(&callee) {
            return true;
        }
    }
    if let Some(ownership) = builtins.protocol.get(&callee) {
        return matches!(
            ownership.get(pos),
            Some(ori_ir::builtin_constants::protocol::ProtocolArgOwnership::Owned)
        );
    }
    if let Some(contract) = sigs.get(&callee) {
        return contract
            .params
            .get(pos)
            .is_some_and(|p| p.access == AccessClass::Owned);
    }
    // Unknown non-builtin callee: conservative Owned (clears the read-only fact),
    // matching `compute_arg_ownership`'s unknown-owned default.
    true
}

/// Is a BORROWED `pos`-th arg of `callee` read-only-safe to forward? Builtin and
/// `ori_*` borrowed positions are leaf-safe (no deeper user COW). A user-fn
/// borrowed position is safe only when that callee's corresponding param is itself
/// `borrowed_read_only` (SCC: inner contract finalized first per IC-1). Companion
/// to [`find_borrowed_read_only_params`].
fn borrowed_ro_arg_forward_safe(
    callee: Name,
    pos: usize,
    builtins: &crate::BuiltinOwnershipSets,
    sigs: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> bool {
    if builtins.is_builtin(callee) {
        return true;
    }
    if interner
        .try_lookup(callee)
        .is_some_and(|n| n.starts_with("ori_"))
    {
        return true;
    }
    sigs.get(&callee)
        .and_then(|c| c.params.get(pos))
        .is_some_and(|p| p.borrowed_read_only)
}

/// Find Borrowed-eligible params COW-CONSUMED at the lineage's LAST body use —
/// the `ParamContract.borrowed_cow_consumed` fact obligating CALLER funding
/// (one duplication inc per call site, RL-1 `RL1_duplication_balanced`).
///
/// A param qualifies when an alias of it is consumed at a COW-MUTATOR owned
/// position — a builtin consuming receiver (`@push`/`@insert`/`@remove`/... at
/// pos 0, or pos 1 for `consuming_second_arg` callees), or transitively a user
/// callee whose corresponding param carries `borrowed_cow_consumed` (SCC:
/// callee contract finalized first per IC-1) — AND no use of ANY alias of the
/// same param is forward-reachable past the consume site (the consume is the
/// lineage's death; the callee's COW-inc edge release then nets -1 on the
/// caller's allocation). A live-past consume declines (the callee's edge
/// release declines too — net 0, no funding obligation). Aggregate STORES are
/// NOT COW consumes (the borrowed-store dup inc + container drop net 0).
/// Spec: Annex E §AIMS RL-1 + RL-2.
pub(super) fn find_borrowed_cow_consumed_params(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<usize> {
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let param_use_sites = collect_param_alias_use_sites(func, alias_to_param);
    // A COW-MUTATOR owned position: builtin consuming receiver / second arg,
    // or a transitive user-callee `borrowed_cow_consumed` param.
    let cow_consuming_position = |callee: Name, pos: usize| -> bool {
        if builtins.consuming_receiver.contains(&callee)
            || builtins.consuming_receiver_only.contains(&callee)
        {
            return pos == 0 || (pos == 1 && builtins.consuming_second_arg.contains(&callee));
        }
        if builtins.is_builtin(callee) || builtins.protocol.contains_key(&callee) {
            return false;
        }
        sigs.get(&callee)
            .and_then(|c| c.params.get(pos))
            .is_some_and(|p| p.borrowed_cow_consumed)
    };
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut out: FxHashSet<usize> = FxHashSet::default();
    let check = |callee: Name,
                 args: &[ArcVarId],
                 block_idx: usize,
                 site_idx: usize,
                 reachable_cache: &mut FxHashMap<usize, FxHashSet<usize>>,
                 out: &mut FxHashSet<usize>| {
        for (pos, &arg) in args.iter().enumerate() {
            if !cow_consuming_position(callee, pos) {
                continue;
            }
            let Some(params) = alias_to_param.get(&arg) else {
                continue;
            };
            let reachable = reachable_cache
                .entry(block_idx)
                .or_insert_with(|| successor_reachable(func, block_idx));
            for &i in params {
                let Some(uses) = param_use_sites.get(&i) else {
                    continue;
                };
                let used_after = uses
                    .iter()
                    .any(|&(ub, ui)| (ub == block_idx && ui > site_idx) || reachable.contains(&ub));
                if !used_after {
                    out.insert(i);
                }
            }
        }
    };
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                check(
                    *callee,
                    args,
                    block_idx,
                    instr_idx,
                    &mut reachable_cache,
                    &mut out,
                );
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            // The terminator's own use site is `usize::MAX`; pass it as the
            // site index so the consume's own occurrence never counts as a
            // use after itself.
            check(
                *callee,
                args,
                block_idx,
                usize::MAX,
                &mut reachable_cache,
                &mut out,
            );
        }
    }
    out
}

/// Per-param alias use sites (body `(block, instr)`; terminator
/// `(block, usize::MAX)`) — the death-at-consume reachability input of
/// [`find_borrowed_cow_consumed_params`].
fn collect_param_alias_use_sites(
    func: &ArcFunction,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> FxHashMap<usize, Vec<(usize, usize)>> {
    let mut param_use_sites: FxHashMap<usize, Vec<(usize, usize)>> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            for &v in &instr.used_vars() {
                if let Some(params) = alias_to_param.get(&v) {
                    for &i in params {
                        param_use_sites
                            .entry(i)
                            .or_default()
                            .push((block_idx, instr_idx));
                    }
                }
            }
        }
        for v in block.terminator.used_vars() {
            if let Some(params) = alias_to_param.get(&v) {
                for &i in params {
                    param_use_sites
                        .entry(i)
                        .or_default()
                        .push((block_idx, usize::MAX));
                }
            }
        }
    }
    param_use_sites
}

/// Forward-reachable block set from `start`'s SUCCESSORS (`start` itself only
/// when a cycle re-reaches it).
fn successor_reachable(func: &ArcFunction, start: usize) -> FxHashSet<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = func
        .blocks
        .get(start)
        .map(|b| {
            crate::graph::successor_block_ids(&b.terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .collect()
        })
        .unwrap_or_default();
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        if let Some(block) = func.blocks.get(b) {
            for s in crate::graph::successor_block_ids(&block.terminator) {
                stack.push(s.index());
            }
        }
    }
    visited
}
