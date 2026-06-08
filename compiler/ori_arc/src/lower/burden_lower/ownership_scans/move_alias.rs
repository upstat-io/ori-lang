//! RL-2 move-alias transfer-suppression analysis feeding `emit_burden_ops`:
//! the function-wide fixpoint that propagates ownership-transfer through
//! `Let { Var }` move-alias chains so the move source's last-use `BurdenDec`
//! is suppressed.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::instr_transfer_vars;

/// Compute the move-alias transfer-suppression set per AIMS RL-2.
///
/// Seed = every var transferred out at a terminator (`terminator_transfer`)
/// plus every var consumed at an owned instruction position. A value reaching
/// one of those transfer points THROUGH a `Let { Var }` move-alias chain
/// (`%dst = %src` where the alias is `%src`'s LAST use) also transfers out: that
/// terminal use forwards `%src`'s remaining ownership to `%dst`. Backward-
/// propagate: for every move-alias whose `dst` is in the set, add `src`. Iterate
/// to a fixpoint so multi-hop chains (`%2 = %1; %3 = %2; Return %3`) propagate.
/// The returned set suppresses the last-use `BurdenDec` of every move source in
/// a transfer chain.
///
/// Use-once vs dup'd-terminal-move gate. A use-once source (`use_counts == 1`)
/// is the unchanged pure-move case: its sole use forwards its only reference, so
/// its last-use dec is suppressed (the consumer discharges the release). A DUP'd
/// source (`%s` used >= 2 — earlier uses each consume a duplicate reference)
/// ALSO forwards its ORIGINAL allocation reference at its TERMINAL `Let { Var }`
/// use; emitting that terminal `BurdenDec` releases a reference the move hands to
/// `%dst`'s consuming lineage (RL-2 net=-1, an early over-release that collapses
/// a COW receiver's RC below the LIVE alias count — witness:
/// `let a; let b = a; let c = a.updated(..)` with `b` LIVE decs `a` before the
/// consuming `updated`, so `is_unique` takes the in-place path on a still-aliased
/// buffer). BUT the terminal dec is suppressed ONLY when every NON-terminal
/// duplicate use is LIVE — each LIVE alias releases its own reference at its own
/// last-use (RL-2 `LastReadBeforeScopeExit`), so the source's terminal dec is the
/// redundant double-release. If a duplicate alias is DEAD (`let b = a` where `b`
/// is never read), the source's terminal dec is that dead alias's
/// `ScopeExit` release (RL-2 non-transfer -> dec) — it is KEPT, not suppressed
/// (else the `updated_list_dead_alias_no_leak` case leaks the orphaned buffer).
/// Only the terminal-move source's last-use DEC is governed here; its FRESH inc
/// is KEPT for the dup case (mod.rs gates the symmetric inc-suppression on
/// `use_counts <= 1`), since that inc supplies the duplicate references the
/// non-terminal uses consume. Per AIMS RL-2 `TerminalUse`: a move IS an
/// ownership-transferring terminal use, a dead alias's `ScopeExit` is not
/// (`AimsProof.Realization::RL2_dec_at_last_use`).
pub(in crate::lower::burden_lower) fn compute_transfer_via_move_alias(
    func: &ArcFunction,
    terminator_transfer_per_block: &[FxHashSet<ArcVarId>],
    use_counts: &FxHashMap<ArcVarId, u32>,
    last_use_points: &[(ArcVarId, usize, usize)],
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    // `Invoke @callee(%arg [own]) -> %result` move-edges where `@callee`'s
    // `MemoryContract` transfers an owned param THROUGH its return: `%result` IS
    // the forwarded `%arg` (a move across the call), so the chain
    // `... -> %arg -> %result -> ...` collapses like a `Let { Var }` move. Without
    // this edge the move-chain breaks at the `Invoke` (the result is a fresh SSA
    // var, not a Let-alias of the arg), leaving the value-copy aggregate's
    // intermediate decs/incs un-suppressed (double-free on the forwarder lineage).
    invoke_ttr_edges: &[(ArcVarId, ArcVarId)],
) -> FxHashSet<ArcVarId> {
    // Global-last-use lookup: a var with exactly ONE `last_use_points` entry is
    // used in exactly one block, and that entry is its global last use. A var
    // used in >= 2 blocks has >= 2 entries (one per block) — its terminal use is
    // not statically pin-pointed here, so it is NOT eligible for terminal-move
    // suppression (conservative: keep its dec, never over-suppress cross-block).
    let mut last_use_entry_count: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut last_use_pos: FxHashMap<ArcVarId, (usize, usize)> = FxHashMap::default();
    for &(var, b, i) in last_use_points {
        *last_use_entry_count.entry(var).or_default() += 1;
        last_use_pos.insert(var, (b, i));
    }
    let is_global_last_use = |src: &ArcVarId, b: usize, i: usize| -> bool {
        last_use_entry_count.get(src).copied().unwrap_or(0) == 1
            && last_use_pos.get(src) == Some(&(b, i))
    };
    // Sources with at least one DEAD `Let { Var }` duplicate alias (`%d = %src`,
    // `%d` never used). The dead alias's reference is released by the source's
    // terminal dec (RL-2 `ScopeExit`); suppressing it would leak.
    let mut src_has_dead_alias: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if use_counts.get(dst).copied().unwrap_or(0) == 0 {
                    src_has_dead_alias.insert(*src);
                }
            }
        }
    }

    let mut transferred: FxHashSet<ArcVarId> = FxHashSet::default();
    // Seed: terminator-transferred vars.
    for set in terminator_transfer_per_block {
        transferred.extend(set.iter().copied());
    }
    // Seed: instruction owned-position transfers (Construct/Apply/Set/etc.).
    for block in &func.blocks {
        for instr in &block.body {
            transferred.extend(instr_transfer_vars(instr, func).iter().copied());
        }
    }
    // Move-alias edges `dst -> src`. A use-once source is the unchanged pure-move
    // case. A dup'd source qualifies ONLY at its TERMINAL `Let { Var }` use AND
    // only when it has NO dead duplicate alias (a dead alias's reference is
    // discharged by the kept terminal dec, not by a downstream consumer).
    let mut move_edges: Vec<(ArcVarId, ArcVarId)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                let use_once = use_counts.get(src).copied().unwrap_or(0) == 1;
                let dup_terminal_move = use_counts.get(src).copied().unwrap_or(0) >= 2
                    && is_global_last_use(src, block_idx, instr_idx)
                    && !src_has_dead_alias.contains(src);
                if use_once || dup_terminal_move {
                    move_edges.push((*dst, *src));
                }
            }
        }
    }
    // Invoke transfer-through-return edges: `%result` is the moved `%arg`.
    move_edges.extend(invoke_ttr_edges.iter().copied());
    // Seed: a move-alias source `%s` (`%d = %s`, `%s` used once) whose dst `%d`
    // is owned-RC has its SINGLE release discharged BY `%d` — either `%d`
    // transfers out (seeded above) OR `%d` gets its own last-use dec. The move
    // hands `%s`'s one allocation to `%d`; emitting `%s`'s own last-use dec
    // would double-release the shared buffer, and emitting `%s`'s FRESH-site inc
    // would orphan (the move is not a duplication — `%d` does NOT get a paired
    // inc, so the lineage carries exactly one inc+dec at the `%d` end). Both
    // halves suppressed via this set (dec here, inc via `inc_suppressed_vars`),
    // matching the transfer-out case. Witness: `coll_list_cow_concat_shared`
    // `%14 = %8` (fresh concat result moved to a borrow-used alias) — `%8`'s
    // scope-exit dec double-frees the buffer `%14` decs. Per AIMS RL-2
    // (move = ownership transfer, single release at the lineage's terminal owner).
    for &(dst, src) in &move_edges {
        if owned_vars_needing_rc.contains(&dst) {
            transferred.insert(src);
        }
    }
    // Fixpoint: a move source transfers out when its dst transfers out.
    let mut changed = true;
    while changed {
        changed = false;
        for &(dst, src) in &move_edges {
            if transferred.contains(&dst) && transferred.insert(src) {
                changed = true;
            }
        }
    }
    transferred
}
