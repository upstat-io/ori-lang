//! RL-2 move-alias transfer-suppression analysis feeding `emit_burden_ops`:
//! the function-wide fixpoint that propagates ownership-transfer through
//! `Let { Var }` move-alias chains so the move source's last-use `BurdenDec`
//! is suppressed.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::emit_rc::block_id;
use crate::graph::DominatorTree;
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::{instr_transfer_vars, successor_reachable_blocks};

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
    // used in >= 2 blocks has >= 2 entries (one per block) — its terminal use
    // is proven instead by the successor-reachability final-use proof
    // (`is_cross_block_final_use`), which admits the FIXPOINT-ONLY hand-off
    // edges below.
    let mut last_use_entry_count: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut last_use_pos: FxHashMap<ArcVarId, (usize, usize)> = FxHashMap::default();
    let mut block_last_use: FxHashMap<(ArcVarId, usize), usize> = FxHashMap::default();
    let mut use_blocks: FxHashMap<ArcVarId, Vec<usize>> = FxHashMap::default();
    for &(var, b, i) in last_use_points {
        *last_use_entry_count.entry(var).or_default() += 1;
        last_use_pos.insert(var, (b, i));
        block_last_use.insert((var, b), i);
        use_blocks.entry(var).or_default().push(b);
    }
    let is_single_block_last_use = |src: &ArcVarId, b: usize, i: usize| -> bool {
        last_use_entry_count.get(src).copied().unwrap_or(0) == 1
            && last_use_pos.get(src) == Some(&(b, i))
    };
    let src_has_dead_alias = collect_dead_alias_sources(func, use_counts);

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
    // case. A dup'd source qualifies at its TERMINAL `Let { Var }` use AND
    // only when it has NO dead duplicate alias (a dead alias's reference is
    // discharged by the kept terminal dec, not by a downstream consumer).
    // Terminality is proven two ways:
    // - single-block source: its sole `last_use_points` entry is this site
    //   (`move_edges` — the legacy admitted set, seed + fixpoint);
    // - cross-block LOCAL source: the successor-reachability final-use proof
    //   (`handoff_edges` — FIXPOINT-ONLY conditional edges: the source is
    //   cancelled iff its terminal alias dst GENUINELY transfers out at an
    //   owned position per AIMS RL-2, AND every NON-terminal use of the source
    //   also discharges a duplicate reference — an owned-position consume, or
    //   a `Let { Var }` alias whose own lineage transfers — from a block that
    //   DOMINATES the terminal block (an alternative-arm use never runs on
    //   the terminal's path, so it discharges nothing there). A non-consuming
    //   non-terminal use (borrow read, slice view) leaves a duplicate
    //   reference whose only release IS the terminal dec, so cancellation
    //   declines. Function PARAMS are excluded: a param's last-use dec marker
    //   is load-bearing on the default coexistence path (`emit_last_use_decs`
    //   keeps it so `populate_class_covered` suppresses the predicate stack's
    //   own real dec); the param-transfers-through-return case is owned by the
    //   contract-driven `transfer_through_return_param_vars` strip instead.
    let param_vars: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let mut move_edges: Vec<(ArcVarId, ArcVarId)> = Vec::new();
    let mut handoff_edges: Vec<HandoffEdge> = Vec::new();
    let mut reach_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    // Built lazily — only a function with at least one cross-block final-use
    // candidate pays for the dominator tree.
    let mut dom_tree: Option<DominatorTree> = None;
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                let count = use_counts.get(src).copied().unwrap_or(0);
                let dup_no_dead_alias = count >= 2 && !src_has_dead_alias.contains(src);
                let use_once = count == 1;
                let dup_terminal_move =
                    dup_no_dead_alias && is_single_block_last_use(src, block_idx, instr_idx);
                if use_once || dup_terminal_move {
                    move_edges.push((*dst, *src));
                } else if dup_no_dead_alias
                    && !param_vars.contains(src)
                    && !super::super::cross_block_final_use_cancel_disabled()
                    && is_cross_block_final_use(
                        func,
                        &block_last_use,
                        &use_blocks,
                        &mut reach_cache,
                        *src,
                        block_idx,
                        instr_idx,
                    )
                {
                    let dom = dom_tree.get_or_insert_with(|| DominatorTree::build(func));
                    if let Some(required) =
                        non_terminal_uses_all_discharge(func, dom, *src, block_idx, instr_idx)
                    {
                        handoff_edges.push(HandoffEdge {
                            dst: *dst,
                            src: *src,
                            required,
                        });
                    }
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
    // Fixpoint: a move source transfers out when its dst transfers out. The
    // cross-block hand-off edges join here (and ONLY here): the relaxed gate's
    // cancellation fires solely on a proven downstream owned-position transfer
    // of the terminal alias AND of every non-terminal alias lineage — never on
    // the owned-RC-dst seed above. Both edge kinds are monotone over
    // `transferred`, so the combined fixpoint terminates.
    let mut changed = true;
    while changed {
        changed = false;
        for &(dst, src) in &move_edges {
            if transferred.contains(&dst) && transferred.insert(src) {
                changed = true;
            }
        }
        for edge in &handoff_edges {
            if transferred.contains(&edge.dst)
                && edge.required.iter().all(|d| transferred.contains(d))
                && transferred.insert(edge.src)
            {
                changed = true;
            }
        }
    }
    transferred
}

/// Sources with at least one DEAD `Let { Var }` duplicate alias (`%d = %src`,
/// `%d` never used). The dead alias's reference is released by the source's
/// terminal dec (RL-2 `ScopeExit`); suppressing it would leak.
fn collect_dead_alias_sources(
    func: &ArcFunction,
    use_counts: &FxHashMap<ArcVarId, u32>,
) -> FxHashSet<ArcVarId> {
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
    src_has_dead_alias
}

/// Conditional cross-block hand-off edge: `src`'s terminal `Let { Var }` alias
/// is `dst`; cancellation fires in the fixpoint only when `dst` AND every
/// `required` non-terminal alias dst transfer out.
struct HandoffEdge {
    dst: ArcVarId,
    src: ArcVarId,
    required: Vec<ArcVarId>,
}

/// Classify every NON-terminal use of `src` (every use site except the
/// terminal `Let { Var }` at `(terminal_block, terminal_idx)`) per AIMS RL-2:
///
/// - owned-position consume (`instr_transfer_vars`: Construct / Reuse /
///   `CollectionReuse` / owned Apply arg / `Set.value` / list-concat operand,
///   or an owned terminator position) — discharges its duplicate reference at
///   the consume; nothing further required.
/// - `Let { Var }` alias — discharges only when the alias's own lineage
///   transfers out; the alias dst joins the returned `required` set and is
///   resolved in the fixpoint.
/// - anything else (borrow read, `Project`, `IsShared`, slice view, `Set`
///   base, borrowed terminator arg) — does NOT consume a reference; the
///   terminal dec is that duplicate's only release, so cancellation DECLINES
///   (`None`).
///
/// Every non-terminal use's block MUST additionally DOMINATE the terminal
/// block (same-block uses are earlier-in-block by the caller's finality
/// proof). A discharge counts only when it executes on EVERY path reaching
/// the terminal; an ALTERNATIVE-arm use (sibling `Branch`/`Switch` arm — the
/// per-arm sum-rebuild shape `if c then Left(v, next: t) else
/// Right(v, next: t)`) never runs on this arm's path, so the kept per-arm dec
/// is that path's only release for the path-insensitive duplication inc —
/// cancellation DECLINES (`None`).
fn non_terminal_uses_all_discharge(
    func: &ArcFunction,
    dom: &DominatorTree,
    src: ArcVarId,
    terminal_block: usize,
    terminal_idx: usize,
) -> Option<Vec<ArcVarId>> {
    let terminal_block_id = block_id(terminal_block);
    let mut required: Vec<ArcVarId> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if block_idx == terminal_block && instr_idx == terminal_idx {
                continue;
            }
            if !instr.used_vars().contains(&src) {
                continue;
            }
            if block_idx != terminal_block && !dom.dominates(block_id(block_idx), terminal_block_id)
            {
                return None;
            }
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(s),
                ..
            } = instr
            {
                if *s == src {
                    required.push(*dst);
                    continue;
                }
            }
            if let ArcInstr::Set { base, .. } = instr {
                if *base == src {
                    return None;
                }
            }
            if instr_transfer_vars(instr, func).contains(&src) {
                continue;
            }
            return None;
        }
        let term_uses = block.terminator.used_vars();
        for (pos, &var) in term_uses.iter().enumerate() {
            if var != src {
                continue;
            }
            if !block.terminator.is_owned_position(pos)
                || !dom.dominates(block_id(block_idx), terminal_block_id)
            {
                return None;
            }
        }
    }
    Some(required)
}

/// Successor-reachability final-use proof for a dup'd CROSS-BLOCK move source
/// per AIMS RL-2: the `Let { Var(src) }` at `(b, i)` is `src`'s global final
/// use iff (a) it is `src`'s last use within block `b` (a terminator use of
/// `src` registers at the sentinel index and fails this), and (b) no block
/// reachable from `b`'s successors uses `src`. The walk follows EVERY CFG
/// edge, so a block re-reached through a loop back-edge — including `b`
/// itself — is in the set: a back-edge re-use is a later use of the same
/// lineage and DECLINES the proof (the next iteration still consumes the
/// reference). Reachability sets are memoized per block in `reach_cache`.
fn is_cross_block_final_use(
    func: &ArcFunction,
    block_last_use: &FxHashMap<(ArcVarId, usize), usize>,
    use_blocks: &FxHashMap<ArcVarId, Vec<usize>>,
    reach_cache: &mut FxHashMap<usize, FxHashSet<usize>>,
    src: ArcVarId,
    b: usize,
    i: usize,
) -> bool {
    if block_last_use.get(&(src, b)) != Some(&i) {
        return false;
    }
    let reach = reach_cache
        .entry(b)
        .or_insert_with(|| successor_reachable_blocks(func, b));
    use_blocks
        .get(&src)
        .is_none_or(|blocks| blocks.iter().all(|ub| !reach.contains(ub)))
}
