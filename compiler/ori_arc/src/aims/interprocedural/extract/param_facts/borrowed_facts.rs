//! Borrowed-param read-only and COW-consumed-at-death fact detection.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::super::super::super::contract::MemoryContract;
use super::super::super::super::lattice::AccessClass;

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
pub(in crate::aims::interprocedural::extract) fn find_borrowed_read_only_params(
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

    let mut classify_call_args = |callee: Option<Name>, args: &[ArcVarId]| {
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
                } => classify_call_args(Some(*callee), args),
                ArcInstr::ApplyIndirect { args, .. } => classify_call_args(None, args),
                _ => {}
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke {
                func: callee, args, ..
            } => classify_call_args(Some(*callee), args),
            ArcTerminator::InvokeIndirect { args, .. } => classify_call_args(None, args),
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
    if builtins.contains(callee) {
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

/// Which owned-position class [`find_borrowed_cow_consumed_params`] scans for.
/// The builtin `iter` sits in the consuming-receiver set (the iterator takes
/// ownership of the data buffer) but is NOT a COW mutator, so `MutatorOnly`
/// excludes it — the two scopes feed two distinct contract fields
/// (`borrowed_cow_consumed` vs `borrowed_cow_mutated`), never one boolean mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CowConsumeScope {
    /// Any COW-consuming position, including the builtin `iter` — feeds
    /// `ParamContract.borrowed_cow_consumed`.
    AnyConsume,
    /// Genuine COW mutators only (`push`/`insert`/`set`/...), excluding the
    /// builtin `iter` — feeds `ParamContract.borrowed_cow_mutated`.
    MutatorOnly,
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
pub(crate) fn find_borrowed_cow_consumed_params(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    interner: &ori_ir::StringInterner,
    scope: CowConsumeScope,
) -> FxHashSet<usize> {
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let param_use_sites = collect_param_alias_use_sites(func, alias_to_param);
    let iter_name = interner.intern("iter");
    // A COW-consuming owned position: builtin consuming receiver / second arg,
    // or a transitive user-callee param carrying the matching fact
    // (`borrowed_cow_mutated` on `MutatorOnly`, else `borrowed_cow_consumed`).
    let cow_consuming_position = |callee: Name, pos: usize| -> bool {
        if builtins.consuming_receiver.contains(&callee)
            || builtins.consuming_receiver_only.contains(&callee)
        {
            if scope == CowConsumeScope::MutatorOnly && callee == iter_name {
                return false;
            }
            return pos == 0 || (pos == 1 && builtins.consuming_second_arg.contains(&callee));
        }
        if builtins.contains(callee) {
            return false;
        }
        sigs.get(&callee)
            .and_then(|c| c.params.get(pos))
            .is_some_and(|p| match scope {
                CowConsumeScope::MutatorOnly => p.borrowed_cow_mutated,
                CowConsumeScope::AnyConsume => p.borrowed_cow_consumed,
            })
    };
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut consumed_params: FxHashSet<usize> = FxHashSet::default();
    let record_consumed_params =
        |callee: Name,
         args: &[ArcVarId],
         block_idx: usize,
         site_idx: usize,
         reachable_cache: &mut FxHashMap<usize, FxHashSet<usize>>,
         consumed_params: &mut FxHashSet<usize>| {
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
                    let used_after = uses.iter().any(|&(ub, ui)| {
                        (ub == block_idx && ui > site_idx) || reachable.contains(&ub)
                    });
                    if !used_after {
                        consumed_params.insert(i);
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
                record_consumed_params(
                    *callee,
                    args,
                    block_idx,
                    instr_idx,
                    &mut reachable_cache,
                    &mut consumed_params,
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
            record_consumed_params(
                *callee,
                args,
                block_idx,
                usize::MAX,
                &mut reachable_cache,
                &mut consumed_params,
            );
        }
    }
    consumed_params
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
