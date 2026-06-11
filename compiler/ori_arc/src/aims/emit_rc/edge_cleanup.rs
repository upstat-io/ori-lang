//! Edge cleanup for inter-block RC operations — dispatch hub.
//!
//! Handles variables that are live in a predecessor but dead in a particular
//! successor. For single-predecessor successors, prepends `RcDec` at the
//! successor's entry. For multi-predecessor successors, inserts trampoline
//! blocks. The per-terminator dead-set analysis lives in two sibling category
//! modules: `branch` (Branch/Switch/Jump edges) and `invoke`
//! (Invoke/InvokeIndirect edges + the Phase-6.98 unwind pair-net release). This
//! file owns the shared `EdgeCleanupEnv` + same-alloc reps, the top-level
//! `emit_edge_cleanup` dispatch, and the `apply_edge_decs` trampoline/prepend
//! emission.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::{AimsStateMap, ApplyAliasSource};
use crate::aims::lattice::{AccessClass, Cardinality};
use crate::graph::{compute_predecessors, successor_block_ids};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

use super::trampoline::insert_trampoline;
use super::{block_id, DeferredDec, EdgeDec};

mod branch;
mod invoke;
mod invoke_unwind;

#[cfg(test)]
mod tests;

use branch::collect_branch_edge_decs;
use invoke::collect_invoke_edge_decs;

// Re-export the Invoke Phase-6.98 unwind pair-net release so consumers continue
// to resolve it through `edge_cleanup::` (consumed by `emit_rc::mod`).
pub(crate) use invoke_unwind::emit_invoke_unwind_pair_net_releases;

/// Whether the successor block at `succ_idx` is an unwind block (terminator
/// = Resume).
///
/// For Branch/Switch successors (no explicit unwind/normal distinction in
/// the terminator shape), this is the available signal. For Invoke
/// successors, prefer the explicit `normal`/`unwind` field distinction —
/// `is_unwind_succ_block` is a fallback when the explicit flag is unknown.
///
/// Intermediate blocks reachable via the Invoke unwind-successor cluster may
/// not have Resume terminators themselves, so this shallow check under-detects
/// unwind blocks for the cluster case. It is sound for non-unwind control flow;
/// the unwind-edge (try-block) case requires the deeper authoritative
/// `ArcFunction::unwind_blocks` accessor.
#[inline]
fn is_unwind_succ_block(func: &ArcFunction, succ_idx: usize) -> bool {
    matches!(func.blocks[succ_idx].terminator, ArcTerminator::Resume)
}

/// Whether a variable should be treated as owned for RC purposes.
///
/// Same logic as `is_owned_at_entry` but works with raw state and the
/// function-level borrowed defs set. Used in edge cleanup where we don't
/// have the per-block `defined_in_block` context.
#[inline]
fn is_owned_for_rc(
    state_map: &AimsStateMap,
    var: ArcVarId,
    access: AccessClass,
    cardinality: Cardinality,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) -> bool {
    if state_map.is_excluded(var) {
        return false;
    }
    if access == AccessClass::Owned {
        return true;
    }
    // Cross-block variable with access stuck at BOTTOM (Borrowed).
    // Owned unless it was defined by Project.
    if cardinality != Cardinality::Absent {
        return !all_borrowed_defs.contains(&var);
    }
    false
}

/// Emit `RcDec` on edges where a variable is live in the predecessor but
/// dead in a particular successor.
///
/// Also handles deferred `RcDec` operations from two sources:
/// - **Phase B deferred parents** (`target: None`): parent aggregates whose
///   `RcDec` was deferred because a borrowed child (from Project) is used in
///   the block terminator. Emitted on ALL successor edges.
/// - **Merge-edge decs** (`target: Some(succ)`): branch-local variables at
///   merge blocks. Emitted ONLY on the edge to the specific merge successor.
///
/// Union-find representative over the SAME-ALLOCATION subset of the SSA-alias
/// graph: every union edge `compute_ssa_alias_classes` uses EXCEPT edge type 2
/// (Jump-arg → successor block-param). The Jump-phi edge merges DIFFERENT
/// runtime allocations into one class when a block param has predecessors
/// passing distinct values (e.g. `if c then x else y` unions x and y via the
/// merge param), so it is NOT a same-allocation relation. Edges retained:
/// Let{Var} aliases, apply-result Direct + Conditional (Project/Wrapped already
/// excluded by PIN-2). Used by the PIN-4 class-liveness suppression in
/// `collect_branch_edge_decs` so only a TRUE same-allocation alias being live
/// at a successor suppresses `var`'s edge dec — phi-merged alternatives must
/// not (RL-4 P1 + §10 under-elimination-leaks per-path-net-0 invariant).
///
/// Thin projection over the §1.9 unified alias-table construction
/// (`project_aliases::compute_genuine_same_alloc_reps`) — ONE builder behind
/// both this query and the table's over-approximation classification; no
/// parallel tracker. Spec: Annex E §AIMS.
pub(crate) fn compute_same_alloc_reps(
    func: &ArcFunction,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    crate::aims::intraprocedural::project_aliases::compute_genuine_same_alloc_reps(
        func,
        apply_result_aliases,
    )
}

/// Whether `a` and `b` denote the same runtime allocation (same
/// `compute_same_alloc_reps` rep). A var with no entry is its own rep.
pub(crate) fn same_alloc(reps: &FxHashMap<ArcVarId, ArcVarId>, a: ArcVarId, b: ArcVarId) -> bool {
    reps.get(&a).copied().unwrap_or(a) == reps.get(&b).copied().unwrap_or(b)
}

/// Function-wide analysis context shared by the edge-dead-set collectors.
///
/// Bundles the `ArcFunction`, state map, type pool, borrowed-def set,
/// take-move facts, and same-allocation reps. Every field is a shared borrow
/// read together by `collect_branch_edge_decs` / `compute_branch_edge_dead_set`
/// / `collect_invoke_edge_decs` / `compute_invoke_edge_dead_set`.
#[derive(Clone, Copy)]
pub(crate) struct EdgeCleanupEnv<'a> {
    func: &'a ArcFunction,
    state_map: &'a AimsStateMap,
    pool: &'a Pool,
    all_borrowed_defs: &'a FxHashSet<ArcVarId>,
    take_move_facts: &'a super::take_project::TakeMoveFacts,
    same_alloc_reps: &'a FxHashMap<ArcVarId, ArcVarId>,
}

pub(crate) fn emit_edge_cleanup(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    take_move_facts: &super::take_project::TakeMoveFacts,
    deferred_parent_decs: &FxHashMap<usize, Vec<DeferredDec>>,
    // Probe path (`ORI_DISABLE_PREDICATE_STACK_RC=1`): emit dying-edge releases
    // as `BurdenDec` only (no predicate-stack `RcDec`) so the burden path is the
    // sole RC emitter; Phase 7 lowers the `BurdenDec` to a real `RcDec`. Default
    // (`false`): emit the predicate-stack `RcDec` + adjacent `BurdenDec` ledger
    // marker as before. Spec: Annex E §AIMS RL-4.
    burden_only: bool,
) {
    let predecessors = compute_predecessors(func);
    let same_alloc_reps = compute_same_alloc_reps(func, state_map.apply_result_aliases());

    // Collect edge cleanup operations: (pred_block, succ_block, var, strategy).
    let mut edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)> = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = block_id(block_idx);
        let env = EdgeCleanupEnv {
            func,
            state_map,
            pool,
            all_borrowed_defs,
            take_move_facts,
            same_alloc_reps: &same_alloc_reps,
        };

        // Handle Invoke/InvokeIndirect separately — use InvokeEdgeState.
        if matches!(
            block.terminator,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        ) {
            collect_invoke_edge_decs(env, block_idx, &mut edge_decs);
            // Add deferred decs to Invoke edges. target=None → both edges,
            // target=Some(succ) → only the matching edge.
            if let Some(decs) = deferred_parent_decs.get(&block_idx) {
                if let ArcTerminator::Invoke { normal, unwind, .. }
                | ArcTerminator::InvokeIndirect { normal, unwind, .. } = &block.terminator
                {
                    for &(target, var, strategy) in decs {
                        match target {
                            None => {
                                edge_decs.push((block_idx, normal.index(), var, strategy));
                                edge_decs.push((block_idx, unwind.index(), var, strategy));
                            }
                            Some(succ) if succ == normal.index() => {
                                edge_decs.push((block_idx, normal.index(), var, strategy));
                            }
                            Some(succ) if succ == unwind.index() => {
                                edge_decs.push((block_idx, unwind.index(), var, strategy));
                            }
                            Some(succ) => {
                                debug_assert!(
                                    false,
                                    "merge-edge dec targets block {succ} which is neither \
                                     normal ({}) nor unwind ({}) of Invoke in block {block_idx}",
                                    normal.index(),
                                    unwind.index(),
                                );
                            }
                        }
                    }
                }
            }
            continue;
        }

        let successors = successor_block_ids(&block.terminator);

        // Add deferred decs to successor edges. target=None → all edges,
        // target=Some(succ) → only the matching edge.
        if let Some(decs) = deferred_parent_decs.get(&block_idx) {
            for &(target, var, strategy) in decs {
                match target {
                    None => {
                        for succ_id in &successors {
                            edge_decs.push((block_idx, succ_id.index(), var, strategy));
                        }
                    }
                    Some(succ) => {
                        debug_assert!(
                            successors.iter().any(|s| s.index() == succ),
                            "merge-edge dec targets block {succ} which is not a successor of block {block_idx}",
                        );
                        if successors.iter().any(|s| s.index() == succ) {
                            edge_decs.push((block_idx, succ, var, strategy));
                        }
                    }
                }
            }
        }

        if successors.len() <= 1 {
            continue; // Single successor: no edge-specific cleanup needed.
        }

        // Multiple successors: check each edge for dead variables.
        collect_branch_edge_decs(env, block_idx, blk, &successors, &mut edge_decs);
    }

    apply_edge_decs(func, &predecessors, edge_decs, burden_only);
}

/// Apply collected edge decs: prepend for single-pred successors, trampoline
/// for multi-pred successors.
fn apply_edge_decs(
    func: &mut ArcFunction,
    predecessors: &[Vec<usize>],
    edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)>,
    burden_only: bool,
) {
    if tracing::enabled!(tracing::Level::DEBUG) {
        let burden_true = func.burden_emitted.iter().filter(|b| **b).count();
        let edge_vars: Vec<String> = edge_decs
            .iter()
            .map(|(p, s, v, _)| {
                let be = func.burden_emitted.get(v.index()).copied().unwrap_or(false);
                format!("bb{p}->bb{s}:%{}(burden={be})", v.index())
            })
            .collect();
        tracing::debug!(
            target: "ori_arc::aims::realize::edge_cleanup",
            func = ?func.name,
            burden_emitted_true = burden_true,
            burden_emitted_len = func.burden_emitted.len(),
            edge_decs = %edge_vars.join(" "),
            "apply_edge_decs entry",
        );
    }
    let mut edge_groups: FxHashMap<(usize, usize), Vec<EdgeDec>> = FxHashMap::default();
    for (pred, succ, var, strategy) in edge_decs {
        edge_groups
            .entry((pred, succ))
            .or_default()
            .push((var, strategy));
    }

    let mut trampolines: Vec<(usize, usize, Vec<EdgeDec>)> = Vec::new();

    for ((pred, succ), decs) in &edge_groups {
        if predecessors[*succ].len() == 1 {
            // Faithful release: `BurdenDec` paired adjacent to each edge
            // `RcDec` whose var carries burden ops — per-value burden ledger
            // nets 0 across this CFG edge (RL-4). The edge variant suppresses
            // the burden dec for an owned-transfer arg of `pred`'s terminator
            // (already balanced at the transfer point).
            let dec_instrs: Vec<ArcInstr> = decs
                .iter()
                .flat_map(|&(var, strategy)| {
                    if burden_only {
                        super::release_burden_only_edge(func, *pred, *succ, var)
                    } else {
                        super::release_with_burden_edge(func, *pred, var, strategy)
                    }
                })
                .collect();
            let body = &mut func.blocks[*succ].body;
            let mut new_body = dec_instrs;
            new_body.append(body);
            *body = new_body;
        } else {
            trampolines.push((*pred, *succ, decs.clone()));
        }
    }

    for (pred, succ, decs) in trampolines {
        insert_trampoline(func, pred, succ, &decs, burden_only);
    }
}
