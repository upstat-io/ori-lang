//! RL-4 per-edge release for branch-exclusive terminal-move lineages: a FRESH
//! local `Construct` whose path-insensitive funding inc is consumed at an owned
//! position on a strict subset of branch paths owes exactly one release on each
//! non-consuming branch edge. Spec: Annex E §AIMS RL-4 + RL-1 + RL-2.

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::forwarder_release::ForwarderReleasePos;
use super::store_dup::compute_funded_store_dup_aliases;
use super::{
    collect_def_blocks, instr_transfer_vars, live_out::transferred_to_block,
    successor_reachable_blocks,
};

/// `ORI_DISABLE_BRANCH_EXCLUSIVE_EDGE_RELEASE=1` skips the Phase-5 RL-4
/// per-edge release for a FRESH local `Construct` lineage consumed at an owned
/// position on a strict subset of branch paths
/// ([`compute_branch_exclusive_edge_releases`]). With the toggle set, the
/// non-consuming branch path keeps the pre-branch funding inc with no matched
/// release (the pre-cure +1 leak per non-consuming call). Bisection surface:
/// isolates a branch-exclusive terminal-move leak to this emission vs the rest
/// of the Phase-5 walk. Default (unset): the per-edge release is emitted.
/// Spec: Annex E §AIMS RL-4 (`RL4_edge_dec_decision` +
/// `RL4_edge_release_balanced`) + RL-1 (`RL1_duplication_balanced`) + RL-2
/// (`RL2_release_exactly_once`).
static BRANCH_EXCLUSIVE_EDGE_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_BRANCH_EXCLUSIVE_EDGE_RELEASE").as_deref() == Ok("1")
});

/// Per-root lineage facts consumed by the per-edge admission: the same-alloc
/// member set (root + transitive `Let { Var }` aliases), the borrow-view set
/// (transitive `Project` dsts), every member/view USE site, and the consume
/// sites partitioned by funding.
struct LineageFacts {
    /// `(block, pos)` per use of a lineage member or view; `pos = usize::MAX`
    /// for terminator uses (any body index precedes them).
    use_sites: Vec<(usize, usize)>,
    /// Blocks holding ANY owned-position consume of a lineage member or view
    /// (funded or unfunded).
    consume_blocks: FxHashSet<usize>,
    /// Blocks holding an UNFUNDED consume — a consume priced against the
    /// root's FRESH-site funding inc (the consumed var carries no kept
    /// alias-site inc of its own).
    unfunded_consume_blocks: FxHashSet<usize>,
}

/// Per-edge RL-4 releases for branch-exclusive terminal-move lineages.
///
/// Shape (the `@store_one` family): a FRESH local `Construct` root whose
/// FRESH-site `BurdenInc` is KEPT (path-insensitive funding emitted before a
/// `Branch` / `Switch`) is consumed at an owned position (aggregate store /
/// owned call-arg / map value) on a strict subset of branch paths. The
/// consuming path transfers the funded duplicate (RL-2); the non-consuming
/// sibling path nets +1 — the funded duplicate is live at the branch exit and
/// dead in the sibling's entire forward region with no release
/// (`RL4_edge_dec_decision`: exactly one edge release is owed).
///
/// The release is ADDITIVE — one `BurdenDec(root)` placed on the non-consuming
/// branch-target edge, AFTER the path's final lineage read (no existing op
/// moves; the consuming path emits nothing extra). Placement within the
/// admitted target `B`:
///
/// - no lineage use in `B` → `B`'s entry (`ForwarderReleasePos::BlockEntry`);
/// - final use is a body instruction → after it
///   (`ForwarderReleasePos::AfterInstr`) — the duplicate is released only once
///   every borrow-read of the allocation on this path has completed;
/// - final use is a BORROWED terminator-`Invoke` arg → the `normal`
///   successor's entry (single-pred-gated) — the borrowed call completes on
///   the terminator, so an in-block dec would let the path's existing last-use
///   dec drive the count to zero BEFORE the call reads the buffer.
///
/// Admission gates (ALL hold; a declined edge keeps the current under-release
/// — a leak, never a double-free):
///
/// - root: a `Construct` dst in `owned_vars_needing_rc` whose FRESH-site inc
///   is kept (not inc-suppressed / full-moved) and whose defining block is not
///   in a CFG cycle (a per-iteration root re-defines the binding — out of this
///   scan's per-edge model);
/// - per-edge consume-count partition from the Phase-5 SSOT
///   ([`super::dup_inc::collect_move_edges_and_store_consumes`] consume kinds
///   via [`instr_transfer_vars`] + terminator owned positions) — NEVER
///   var-deadness: the root is live-in at the merge via borrows; the dead
///   entity is the funded DUPLICATE;
/// - ≥1 UNFUNDED consume is forward-reachable from the branch block `D` (the
///   duplicate's pending job — also the once-per-path guard: an admitted
///   target's region has no unfunded consume, so no nested branch inside it
///   re-admits);
/// - the candidate target's forward region (`{B} ∪ reach(B)`,
///   [`successor_reachable_blocks`] — the per-block reachability idiom shared
///   with the duplication scans) holds NO consume of the lineage;
/// - NO consume block reaches the candidate target (mutual exclusion: a
///   loop-exit or post-consume edge is on the SAME runtime path as the
///   consume, so a release there double-frees);
/// - every lineage use in the candidate region lies in the target block
///   itself — a use in a deeper or SHARED (post-merge) block declines: the
///   post-merge borrow-read shape is a genuine funded duplication whose
///   surplus the Phase-7 lineage net already elides (the GREEN clamp);
/// - single-predecessor target (block-entry placement == edge release; merge
///   targets decline per the dead-owned-param precedent);
/// - RL-4 transfer exemption: the root is not handed off on the `D → B` edge
///   ([`transferred_to_block`]).
///
/// FUNDED consumes ([`compute_funded_store_dup_aliases`] +
/// [`super::call_arg_dup::compute_funded_call_arg_dup_aliases`] members) carry
/// their own kept alias-site inc with the consumer as its matched release —
/// they do NOT consume the FRESH-site funding, so they neither satisfy the
/// sibling-consume requirement nor block an edge by reachability.
pub(in crate::lower::burden_lower) fn compute_branch_exclusive_edge_releases(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    inc_suppressed_vars: &FxHashSet<ArcVarId>,
    full_move_vars: &FxHashSet<ArcVarId>,
    funded_call_arg_aliases: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>> {
    if *BRANCH_EXCLUSIVE_EDGE_RELEASE_DISABLED {
        return FxHashMap::default();
    }
    let roots = collect_candidate_roots(
        func,
        owned_vars_needing_rc,
        inc_suppressed_vars,
        full_move_vars,
    );
    if roots.is_empty() {
        return FxHashMap::default();
    }
    let funded_store = compute_funded_store_dup_aliases(func, contracts);
    let def_block = collect_def_blocks(func);
    let n = func.blocks.len();
    let mut pred_count: Vec<u32> = vec![0; n];
    for block in &func.blocks {
        for succ in crate::graph::successor_block_ids(&block.terminator) {
            if succ.index() < n {
                pred_count[succ.index()] += 1;
            }
        }
    }
    let mut reach_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let reach = |b: usize, cache: &mut FxHashMap<usize, FxHashSet<usize>>| -> FxHashSet<usize> {
        cache
            .entry(b)
            .or_insert_with(|| successor_reachable_blocks(func, b))
            .clone()
    };

    let mut out: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>> = FxHashMap::default();
    for root in roots {
        // Per-iteration roots are out of the per-edge model: re-reaching the
        // definition re-defines the binding (the duplication scans' CUT).
        let Some(&def) = def_block.get(&root) else {
            continue;
        };
        if reach(def, &mut reach_cache).contains(&def) {
            continue;
        }
        let facts = collect_lineage_facts(func, root, &funded_store, funded_call_arg_aliases);
        if facts.unfunded_consume_blocks.is_empty() {
            continue;
        }
        for (d_idx, block) in func.blocks.iter().enumerate() {
            if !matches!(
                block.terminator,
                ArcTerminator::Branch { .. } | ArcTerminator::Switch { .. }
            ) {
                continue;
            }
            let d_reach = reach(d_idx, &mut reach_cache);
            if !facts
                .unfunded_consume_blocks
                .iter()
                .any(|c| d_reach.contains(c))
            {
                continue;
            }
            let mut seen_targets: FxHashSet<usize> = FxHashSet::default();
            for succ in crate::graph::successor_block_ids(&block.terminator) {
                let target = succ.index();
                if target >= n || !seen_targets.insert(target) || pred_count[target] != 1 {
                    continue;
                }
                let Some(pos) =
                    admit_edge(func, root, &facts, d_idx, target, &pred_count, &mut |b| {
                        reach(b, &mut reach_cache)
                    })
                else {
                    continue;
                };
                let entry = out.entry(pos).or_default();
                if !entry.contains(&root) {
                    entry.push(root);
                }
            }
        }
    }
    out
}

/// FRESH local `Construct` dsts whose FRESH-site funding inc is KEPT — the
/// only roots whose pre-branch duplicate can be stranded on a non-consuming
/// branch path.
fn collect_candidate_roots(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    inc_suppressed_vars: &FxHashSet<ArcVarId>,
    full_move_vars: &FxHashSet<ArcVarId>,
) -> Vec<ArcVarId> {
    let mut roots = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, .. } = instr {
                if owned_vars_needing_rc.contains(dst)
                    && !inc_suppressed_vars.contains(dst)
                    && !full_move_vars.contains(dst)
                {
                    roots.push(*dst);
                }
            }
        }
    }
    roots
}

/// Walk `func` once for `root`'s lineage: members (root + transitive
/// `Let { Var }` aliases), views (transitive `Project` dsts), every use site,
/// and the consume partition. View consumes are conservatively UNFUNDED (a
/// stored borrow-view is outside the funded families).
fn collect_lineage_facts(
    func: &ArcFunction,
    root: ArcVarId,
    funded_store: &FxHashSet<ArcVarId>,
    funded_call_arg: &FxHashSet<ArcVarId>,
) -> LineageFacts {
    // Fixed-point member/view closure over the two structural edge kinds.
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
    let mut views: FxHashSet<ArcVarId> = FxHashSet::default();
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if members.contains(src) && members.insert(*dst) => {
                        grew = true;
                    }
                    ArcInstr::Project { dst, value, .. }
                        if (members.contains(value) || views.contains(value))
                            && views.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
                }
            }
        }
        if !grew {
            break;
        }
    }

    let is_funded = |var: ArcVarId| funded_store.contains(&var) || funded_call_arg.contains(&var);
    let mut use_sites: Vec<(usize, usize)> = Vec::new();
    let mut consume_blocks: FxHashSet<usize> = FxHashSet::default();
    let mut unfunded_consume_blocks: FxHashSet<usize> = FxHashSet::default();
    let mut record_consume = |block_idx: usize, var: ArcVarId, is_view: bool| {
        consume_blocks.insert(block_idx);
        if is_view || !is_funded(var) {
            unfunded_consume_blocks.insert(block_idx);
        }
    };
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            // Burden-op markers are TF-N/A metadata, not uses — skipped so the
            // scan stays in lock-step pre- and post-emission (the wiring point
            // is pre-emission; unit fixtures may carry ops).
            if super::dup_inc::is_burden_op(instr) {
                continue;
            }
            let transfers = instr_transfer_vars(instr, func);
            for &var in &instr.used_vars() {
                let is_member = members.contains(&var);
                let is_view = views.contains(&var);
                if !is_member && !is_view {
                    continue;
                }
                use_sites.push((block_idx, instr_idx));
                if transfers.contains(&var) || instr_consumes_in_place(instr, var) {
                    record_consume(block_idx, var, is_view);
                }
            }
        }
        for (pos, &var) in block.terminator.used_vars().iter().enumerate() {
            let is_member = members.contains(&var);
            let is_view = views.contains(&var);
            if !is_member && !is_view {
                continue;
            }
            use_sites.push((block_idx, usize::MAX));
            if terminator_consumes(&block.terminator, pos) {
                record_consume(block_idx, var, is_view);
            }
        }
    }
    LineageFacts {
        use_sites,
        consume_blocks,
        unfunded_consume_blocks,
    }
}

/// In-place / token positions outside `instr_transfer_vars`'s owned-position
/// set that still end the duplicate's read-only life — conservatively a
/// consume (decline-driving): COW mutation receivers, refcount inspection,
/// reuse tokens.
fn instr_consumes_in_place(instr: &ArcInstr, var: ArcVarId) -> bool {
    match instr {
        ArcInstr::Set { base, .. } | ArcInstr::SetTag { base, .. } => *base == var,
        ArcInstr::IsShared { var: v, .. } | ArcInstr::Reset { var: v, .. } => *v == var,
        ArcInstr::Reuse { token, .. } => *token == var,
        _ => false,
    }
}

/// Terminator positions that hand the value off the current path: `Return`
/// values, `Jump` args (block-param handoff — the lineage continues under a
/// name this scan does not model), owned `Invoke` / `InvokeIndirect` args, and
/// `Resume` (conservative). Borrowed `Invoke` args and `Branch` / `Switch`
/// operands are reads, not consumes.
fn terminator_consumes(term: &ArcTerminator, pos: usize) -> bool {
    match term {
        ArcTerminator::Return { .. } | ArcTerminator::Jump { .. } | ArcTerminator::Resume => true,
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
            term.is_owned_position(pos)
        }
        ArcTerminator::Branch { .. }
        | ArcTerminator::Switch { .. }
        | ArcTerminator::Unreachable => false,
    }
}

/// Evaluate the per-edge admission gates for `D → target` and return the
/// release position, or `None` to decline (the leak persists — never a
/// double-free).
fn admit_edge(
    func: &ArcFunction,
    root: ArcVarId,
    facts: &LineageFacts,
    d_idx: usize,
    target: usize,
    pred_count: &[u32],
    reach: &mut dyn FnMut(usize) -> FxHashSet<usize>,
) -> Option<(usize, ForwarderReleasePos)> {
    // A target that cannot complete normally (the impossible `Switch` default
    // arm's `Unreachable`, an unwind handler's `Resume`) never executes a
    // release — the dying unwind / unreachable edges are owned by the edge
    // cleanup, not this scan.
    if matches!(
        func.blocks[target].terminator,
        ArcTerminator::Unreachable | ArcTerminator::Resume
    ) {
        return None;
    }
    let mut region = reach(target);
    region.insert(target);
    // No consume of the lineage anywhere forward of this edge.
    if facts.consume_blocks.iter().any(|c| region.contains(c)) {
        return None;
    }
    // Mutual exclusion: no consume block reaches the target (a loop-exit /
    // post-consume edge shares its runtime path with the consume).
    for &c in &facts.consume_blocks {
        if c == target || reach(c).contains(&target) {
            return None;
        }
    }
    // Every forward lineage use is confined to the target block itself — a
    // deeper or shared (post-merge) use is the funded-duplication clamp shape.
    if facts
        .use_sites
        .iter()
        .any(|&(b, _)| b != target && region.contains(&b))
    {
        return None;
    }
    // RL-4 transfer exemption on the edge itself.
    if transferred_to_block(&func.blocks[d_idx].terminator, func.blocks[target].id).contains(&root)
    {
        return None;
    }
    // Placement: after the path's final lineage read.
    let last_use_in_target = facts
        .use_sites
        .iter()
        .filter(|&&(b, _)| b == target)
        .map(|&(_, pos)| pos)
        .max();
    match last_use_in_target {
        None => Some((target, ForwarderReleasePos::BlockEntry)),
        Some(usize::MAX) => {
            // Final read is a BORROWED terminator-`Invoke` arg (owned args are
            // consumes — already declined). Release at the single-pred normal
            // successor entry so the call completes before the count can reach
            // zero.
            let (ArcTerminator::Invoke { normal, .. }
            | ArcTerminator::InvokeIndirect { normal, .. }) = &func.blocks[target].terminator
            else {
                return None;
            };
            let succ = normal.index();
            if succ >= pred_count.len() || pred_count[succ] != 1 {
                return None;
            }
            Some((succ, ForwarderReleasePos::BlockEntry))
        }
        Some(idx) => Some((target, ForwarderReleasePos::AfterInstr(idx))),
    }
}
