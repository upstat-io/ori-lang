//! Branch/Switch/Jump edge cleanup — the per-edge dead-set
//! (`compute_branch_edge_dead_set`) for owned non-scalar vars live at a block's
//! exit but dead at a successor's entry, after take-move-class / apply-aliased /
//! same-alloc-member-live / per-edge-class-dedup suppression. SSOT consumed by
//! both `edge_cleanup` (`RcDec`) and burden-op emission (`BurdenDec`). Spec:
//! Annex E §AIMS RL-4.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::lattice::Cardinality;
use crate::ir::{ArcBlockId, ArcVarId, RcStrategy};

use super::super::trampoline::compute_defined_at_or_before;
use super::super::{rc_strategy, should_suppress_apply_aliased_dec};
use super::{is_owned_for_rc, is_unwind_succ_block, same_alloc, EdgeCleanupEnv};

/// Collect branch/switch/jump edge `RcDec`s by mapping the shared edge-dead-set
/// to `(pred, succ, var, RcStrategy)`. The dead-set itself is the SSOT consumed
/// by both this predicate-stack emitter and burden-op emission.
pub(crate) fn collect_branch_edge_decs(
    env: EdgeCleanupEnv<'_>,
    block_idx: usize,
    blk: ArcBlockId,
    successors: &[ArcBlockId],
    edge_decs: &mut Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
    for (pred, succ, var) in compute_branch_edge_dead_set(env, block_idx, blk, successors) {
        if let Some(strategy) = rc_strategy(env.func, var, env.pool) {
            edge_decs.push((pred, succ, var, strategy));
        }
    }
}

/// Pure branch/switch/jump edge-dead-set: which `(pred_block, succ_block, var)`
/// triples have `var` owned-non-scalar live at `pred` exit but dead (Absent) at
/// `succ` entry, after take-move-class / apply-aliased / same-alloc-member-live
/// / per-edge-class-dedup suppression. SSOT consumed by both `edge_cleanup`
/// (`RcDec`) and burden-op emission (`BurdenDec`); strategy re-derived per var.
#[expect(
    clippy::too_many_lines,
    reason = "single edge-dead-set analysis pass — extracting would fragment the per-edge suppression logic"
)]
pub(crate) fn compute_branch_edge_dead_set(
    env: EdgeCleanupEnv<'_>,
    block_idx: usize,
    blk: ArcBlockId,
    successors: &[ArcBlockId],
) -> Vec<(usize, usize, ArcVarId)> {
    let EdgeCleanupEnv {
        func,
        state_map,
        pool,
        all_borrowed_defs,
        take_move_facts,
        same_alloc_reps,
    } = env;
    let mut dead_set: Vec<(usize, usize, ArcVarId)> = Vec::new();
    let Some(exit_states) = state_map.block_exit_states(blk) else {
        return dead_set;
    };

    // Filter out variables defined downstream (from project-source demand
    // propagation at merge points).
    let defined_at_or_before = compute_defined_at_or_before(func, block_idx);

    // PIN-5: per-edge class-id tracking for same-edge batching.
    let mut classes_inserted_per_edge: FxHashMap<(usize, usize), FxHashSet<u32>> =
        FxHashMap::default();

    for (&var, &state) in exit_states {
        if state.is_scalar() {
            continue;
        }
        if !is_owned_for_rc(
            state_map,
            var,
            state.access,
            state.cardinality,
            all_borrowed_defs,
        ) {
            continue;
        }
        // skip vars that participate in a
        // take-project alias class. Their scope-exit drops are
        // emitted by `dead_cleanup` source 1's in-class branch on
        // bypass-safe blocks (per-class), with class-deduped
        // semantics. Letting edge cleanup also emit a dec for an
        // alias sibling (e.g., `%19 = %5` Let alias) would
        // double-free the shared underlying value.
        if take_move_facts.is_in_class(var) {
            tracing::debug!(
                target: "ori_arc::aims::realize::edge_cleanup",
                func = ?func.name, var = var.raw(), block = block_idx,
                reason = "in-take-move-class",
                "RL-4 branch-edge-dec SUPPRESSED"
            );
            continue;
        }
        // Skip variables that are only defined downstream (in a successor
        // block). These come from project-source demand propagation at
        // merge points and don't actually exist at this block's exit.
        if !defined_at_or_before.contains(&var) {
            tracing::debug!(
                target: "ori_arc::aims::realize::edge_cleanup",
                func = ?func.name, var = var.raw(), block = block_idx,
                reason = "not-defined-at-or-before",
                "RL-4 branch-edge-dec SUPPRESSED"
            );
            continue;
        }
        for succ_id in successors {
            let succ_entry = state_map.var_state_at_block_entry(*succ_id, var);
            if (succ_entry.cardinality == Cardinality::Absent
                || !is_owned_for_rc(
                    state_map,
                    var,
                    succ_entry.access,
                    succ_entry.cardinality,
                    all_borrowed_defs,
                ))
                && rc_strategy(func, var, pool).is_some()
            {
                // Suppress the edge dec when `var` was consumed by an
                // Apply/Invoke whose dst aliases it.
                let is_unwind_succ = is_unwind_succ_block(func, succ_id.index());
                if should_suppress_apply_aliased_dec(state_map, var, is_unwind_succ) {
                    tracing::debug!(
                        target: "ori_arc::aims::realize::edge_cleanup",
                        func = ?func.name, var = var.raw(),
                        block = block_idx, succ = succ_id.index(),
                        reason = "apply-aliased-dst",
                        "RL-4 branch-edge-dec SUPPRESSED"
                    );
                    continue;
                }
                // PIN-4 + PIN-5: class-aware skip + per-edge
                // batching. Skip when any class member is live at the
                // successor's entry (PIN-4), or when the same class
                // already emitted a dec for this (pred, succ) edge in
                // this collection pass (PIN-5).
                if let Some(class_id) = state_map.ssa_alias_class_of(var) {
                    if let Some(members) = state_map.class_members(class_id) {
                        // Only a TRUE same-allocation alias being live at the
                        // successor may suppress var's edge dec. Phi-merged
                        // alternatives (distinct allocations unioned via a
                        // Jump-arg→block-param merge param, e.g. `if c then x
                        // else y`) must NOT suppress — each alternative needs
                        // its own edge dec on the branch where it dies
                        // (RL-4 P1 + §10 under-elimination per-path-net-0).
                        // Project-merge guard: a same-alloc member `m` may
                        // only carry `var`'s drop across the THIS-block→succ
                        // edge if it actually EXISTS at this block
                        // (defined-at-or-before `block_idx`, mirroring the
                        // `var` guard above). A member defined only on a
                        // SIBLING branch (e.g. `%15 = %p2` in the else arm)
                        // is reported `Once`-live at the then-successor entry
                        // by `var_state_at_block_entry` (backward alias-demand
                        // bleed) but is NOT reachable on this edge — counting
                        // it phantom-suppresses the not-taken parent's edge
                        // dec → leak. Spec: Annex E §AIMS RL-4.
                        if let Some(&m_live) = members.iter().find(|&&m| {
                            defined_at_or_before.contains(&m)
                                && same_alloc(same_alloc_reps, m, var)
                                && state_map.var_state_at_block_entry(*succ_id, m).cardinality
                                    != Cardinality::Absent
                        }) {
                            tracing::debug!(
                                target: "ori_arc::aims::realize::edge_cleanup",
                                func = ?func.name, var = var.raw(),
                                block = block_idx, succ = succ_id.index(),
                                member = m_live.raw(),
                                member_card = ?state_map
                                    .var_state_at_block_entry(*succ_id, m_live)
                                    .cardinality,
                                reason = "pin4-same-alloc-member-live",
                                "RL-4 branch-edge-dec SUPPRESSED"
                            );
                            continue;
                        }
                    }
                    let edge_key = (block_idx, succ_id.index());
                    let edge_classes = classes_inserted_per_edge.entry(edge_key).or_default();
                    if !edge_classes.insert(class_id) {
                        continue;
                    }
                }
                dead_set.push((block_idx, succ_id.index(), var));
            }
        }
    }
    dead_set
}
