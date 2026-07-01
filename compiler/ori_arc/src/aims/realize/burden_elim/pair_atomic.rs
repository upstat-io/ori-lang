//! Pair-atomic alias-lineage collection (RL-1) for `eliminate_whole_function`.
//!
//! Identifies `Let { Var }` alias dsts whose birth reference belongs to a
//! shared root (a function param, or a local `Construct` whose lineage
//! terminally moves into an aggregate store) so their Inc/Dec pair is
//! eliminated as a unit rather than split.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::forward_reachable;
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};
use crate::lower::burden_lower::{
    collect_move_edges_and_store_consumes, local_construct_pair_coupling_disabled,
};

/// Read-only structural scan: every `Let { Var }` alias dst whose alias-chain
/// ROOT is a function PARAM or a LOCAL fresh `Construct`.
///
/// An inc-carrying alias dst with such a root is an RL-1 duplication-alias
/// pair on the root's allocation: its `BurdenInc` IS the alias's own `+1` (an
/// alias definition is never a birth site — the var is a same-allocation view
/// of the root's reference), paired with either its own last-use `BurdenDec`
/// or a cross-var release (the consumer's, when the alias was
/// transfer-suppressed at Phase 5). The pair is ATOMIC per
/// `AimsProof.Realization::RL1_duplication_balanced` —
/// `mark_whole_var_removals` consults this set to ban the decoupled inc-only
/// split for these vars (splitting nets -1 on the still-live root lineage —
/// the `@stash_and_return` double-free for param roots; the local
/// terminal-move-store double-free for Construct roots).
///
/// Aliases rooted at an `Apply` / `Invoke` RESULT stay on the decoupled path:
/// their splits are load-bearing compensation for pre-existing
/// under-emissions, coupled only WITH the matching under-emission cure (each
/// its own cycle). Construct-rooted lineages WITHOUT a store consume likewise
/// stay decoupled — their splits compensate borrowed-call-arg lineage
/// arrangements the same way. The Construct-rooted admission is gated by
/// `ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING=1` independently of the
/// param-rooted admission (master toggle:
/// `ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING=1` gates the whole pair-atomic
/// guard in `mark_whole_var_removals`).
pub(super) fn collect_pair_atomic_alias_dsts(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    // Pass 1: resolve every `Let { Var }` alias chain to its root (vars are
    // defined before use within the function walk).
    let mut root: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                let r = root.get(src).copied().unwrap_or(*src);
                root.insert(*dst, r);
            }
        }
    }
    let root_of = |v: ArcVarId| root.get(&v).copied().unwrap_or(v);
    // Pass 2: admissible roots — every param; plus local `Construct` dsts
    // whose lineage TERMINALLY moves into an aggregate-STORE (the local
    // terminal-move-store family, `Holder { kept: w }` with no use of `w`
    // after the store): the store transfers the root's reference into the
    // container, so the pre-elim per-lineage ledger is exact and any split
    // nets -1. A lineage with a use AFTER the store (the local genuine-dup
    // shape — store + source read after) stays DECOUPLED: its FRESH-site inc
    // is balanced by a read-alias split, the compensation arrangement the
    // decoupled path provides until that over-emission gets its own cycle.
    // Store-consume membership shares the Phase-5 SSOT
    // (`collect_move_edges_and_store_consumes`).
    let mut root_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    if !local_construct_pair_coupling_disabled() {
        root_vars.extend(terminal_store_construct_roots(func, &root));
    }
    // Pass 3: collect alias dsts whose chain root is admissible.
    let mut pair_atomic: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if root_vars.contains(&root_of(*src)) || root_vars.contains(src) {
                    pair_atomic.insert(*dst);
                }
            }
        }
    }
    // Pass 4: loop-carried pure-borrow-view aliases (RL-1 pair atomicity) —
    // see `super::loop_carried::extend_with_loop_carried_borrow_view_aliases`.
    super::loop_carried::extend_with_loop_carried_borrow_view_aliases(func, &root, &mut pair_atomic);
    pair_atomic
}

/// Local `Construct` roots whose alias lineage TERMINALLY moves into an
/// aggregate-STORE: the lineage has at least one store-consumed member
/// (`Construct` / `Reuse` / `CollectionReuse` arg, `Set.value` — per the
/// Phase-5 SSOT `collect_move_edges_and_store_consumes`) and NO lineage-member
/// use strictly after any store site ("after" = later in the store's block,
/// OR in a block forward-reachable from the store block's successors — the
/// loop back-edge re-reach counts, mirroring the Phase-5 reachability
/// discriminator). A lineage used past the store is the local genuine-dup
/// shape and stays decoupled (its FRESH-site inc is compensated by a
/// read-alias split until that over-emission's own cycle).
fn terminal_store_construct_roots(
    func: &ArcFunction,
    root: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let root_of = |v: ArcVarId| root.get(&v).copied().unwrap_or(v);
    let mut construct_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, .. } = instr {
                construct_dsts.insert(*dst);
            }
        }
    }
    if construct_dsts.is_empty() {
        return FxHashSet::default();
    }
    let (_, store_consumed) = collect_move_edges_and_store_consumes(func);
    // Per-root lineage use sites + store-consume sites. Terminator uses at
    // `usize::MAX` so any body index in the block precedes them.
    let mut lineage_uses: FxHashMap<ArcVarId, Vec<(usize, usize)>> = FxHashMap::default();
    let mut lineage_store_sites: FxHashMap<ArcVarId, Vec<(usize, usize)>> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            for &v in &instr.used_vars() {
                let r = root_of(v);
                if construct_dsts.contains(&r) {
                    lineage_uses
                        .entry(r)
                        .or_default()
                        .push((block_idx, instr_idx));
                }
            }
            let store_args: &[ArcVarId] = match instr {
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. } => args,
                ArcInstr::Set { value, .. } => std::slice::from_ref(value),
                _ => &[],
            };
            for &v in store_args {
                if !store_consumed.contains(&v) {
                    continue;
                }
                let r = root_of(v);
                if construct_dsts.contains(&r) {
                    lineage_store_sites
                        .entry(r)
                        .or_default()
                        .push((block_idx, instr_idx));
                }
            }
        }
        for v in block.terminator.used_vars() {
            let r = root_of(v);
            if construct_dsts.contains(&r) {
                lineage_uses
                    .entry(r)
                    .or_default()
                    .push((block_idx, usize::MAX));
            }
        }
    }
    // Forward-reachable-from-successors per store block, memoized.
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    'roots: for (r, stores) in &lineage_store_sites {
        let Some(uses) = lineage_uses.get(r) else {
            continue;
        };
        for &(sb, si) in stores {
            let reachable = reachable_cache.entry(sb).or_insert_with(|| {
                let succs: Vec<usize> = func
                    .blocks
                    .get(sb)
                    .map(|b| {
                        crate::graph::successor_block_ids(&b.terminator)
                            .into_iter()
                            .map(crate::ir::ArcBlockId::index)
                            .collect()
                    })
                    .unwrap_or_default();
                forward_reachable(func, &succs)
            });
            let used_after = uses
                .iter()
                .any(|&(ub, ui)| (ub == sb && ui > si) || reachable.contains(&ub));
            if used_after {
                continue 'roots;
            }
        }
        out.insert(*r);
    }
    out
}
