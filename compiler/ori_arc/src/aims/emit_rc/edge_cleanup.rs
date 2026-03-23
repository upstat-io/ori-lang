//! Edge cleanup for inter-block RC operations.
//!
//! Handles variables that are live in a predecessor but dead in a particular
//! successor. For single-predecessor successors, prepends `RcDec` at the
//! successor's entry. For multi-predecessor successors, inserts trampoline
//! blocks.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{AccessClass, Cardinality};
use crate::graph::{compute_predecessors, successor_block_ids};
use crate::ir::{
    ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, RcStrategy,
};

use super::trampoline::{compute_defined_at_or_before, insert_trampoline};
use super::{block_id, rc_strategy, DeferredDec, EdgeDec};

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
pub(crate) fn emit_edge_cleanup(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    deferred_parent_decs: &FxHashMap<usize, Vec<DeferredDec>>,
) {
    let predecessors = compute_predecessors(func);

    // Collect edge cleanup operations: (pred_block, succ_block, var, strategy).
    let mut edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)> = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = block_id(block_idx);

        // Handle Invoke separately — use InvokeEdgeState.
        if matches!(block.terminator, ArcTerminator::Invoke { .. }) {
            collect_invoke_edge_decs(
                func,
                block_idx,
                state_map,
                pool,
                all_borrowed_defs,
                &mut edge_decs,
            );
            // Add deferred decs to Invoke edges. target=None → both edges,
            // target=Some(succ) → only the matching edge.
            if let Some(decs) = deferred_parent_decs.get(&block_idx) {
                if let ArcTerminator::Invoke { normal, unwind, .. } = &block.terminator {
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
        collect_branch_edge_decs(
            func,
            block_idx,
            blk,
            &successors,
            state_map,
            pool,
            all_borrowed_defs,
            &mut edge_decs,
        );
    }

    apply_edge_decs(func, &predecessors, edge_decs);
}

/// Collect edge-specific `RcDec` for multi-successor blocks (Branch/Switch).
#[expect(
    clippy::too_many_arguments,
    reason = "edge collection needs full analysis context"
)]
fn collect_branch_edge_decs(
    func: &ArcFunction,
    block_idx: usize,
    blk: ArcBlockId,
    successors: &[ArcBlockId],
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    edge_decs: &mut Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
    let Some(exit_states) = state_map.block_exit_states(blk) else {
        return;
    };

    // Filter out variables defined downstream (from project-source demand
    // propagation at merge points).
    let defined_at_or_before = compute_defined_at_or_before(func, block_idx);

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
        // Skip variables that are only defined downstream (in a successor
        // block). These come from project-source demand propagation at
        // merge points and don't actually exist at this block's exit.
        if !defined_at_or_before.contains(&var) {
            continue;
        }
        for succ_id in successors {
            let succ_entry = state_map.var_state_at_block_entry(*succ_id, var);
            if succ_entry.cardinality == Cardinality::Absent
                || !is_owned_for_rc(
                    state_map,
                    var,
                    succ_entry.access,
                    succ_entry.cardinality,
                    all_borrowed_defs,
                )
            {
                if let Some(strategy) = rc_strategy(func, var, pool) {
                    edge_decs.push((block_idx, succ_id.index(), var, strategy));
                }
            }
        }
    }
}

/// Apply collected edge decs: prepend for single-pred successors, trampoline
/// for multi-pred successors.
fn apply_edge_decs(
    func: &mut ArcFunction,
    predecessors: &[Vec<usize>],
    edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
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
            let dec_instrs: Vec<ArcInstr> = decs
                .iter()
                .map(|&(var, strategy)| ArcInstr::RcDec { var, strategy })
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
        insert_trampoline(func, pred, succ, &decs);
    }
}

/// Collect edge-specific `RcDec` for Invoke terminators using
/// `InvokeEdgeState` to determine normal vs unwind cleanup.
///
/// Three cleanup categories: (1) `exit_states` vars dying on specific edges,
/// (2) borrowed `Invoke` args absent from `exit_states`, (3) unwind cleanup for
/// ownership-transferred args. Categories 1-2 exclude `Owned` normal-path
/// transfers; category 3 handles the unwind path for those same variables.
#[expect(
    clippy::too_many_lines,
    reason = "3 cleanup categories in one function — extracting would fragment edge logic"
)]
fn collect_invoke_edge_decs(
    func: &ArcFunction,
    block_idx: usize,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    edge_decs: &mut Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
    let blk = block_id(block_idx);
    let ArcTerminator::Invoke {
        normal,
        unwind,
        args,
        arg_ownership,
        ..
    } = &func.blocks[block_idx].terminator
    else {
        return;
    };
    let (normal, unwind) = (*normal, *unwind);

    let edge_state = state_map.invoke_edge_state(blk);
    let exit_states = state_map.block_exit_states(blk);

    // Cat 3 vars — so Cat 1 can skip to avoid double-dec.
    let mut cat3_unwind_vars: FxHashSet<ArcVarId> = FxHashSet::default();

    // Category 3: unwind cleanup for Owned args (callee may not have consumed).
    for (i, &arg) in args.iter().enumerate() {
        let is_owned = arg_ownership
            .get(i)
            .is_none_or(|o| *o == ArgOwnership::Owned);
        if !is_owned {
            continue;
        }
        if state_map.is_excluded(arg) {
            continue;
        }
        if all_borrowed_defs.contains(&arg) {
            continue;
        }
        if let Some(strategy) = rc_strategy(func, arg, pool) {
            edge_decs.push((block_idx, unwind.index(), arg, strategy));
            cat3_unwind_vars.insert(arg);
        }
    }

    // Precompute which variables are defined at or before this block.
    let defined_at_or_before = compute_defined_at_or_before(func, block_idx);

    // Category 1: variables in exit_states that die on specific edges.
    if let (Some(edge_state), Some(exit_states)) = (edge_state, exit_states) {
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
            // Skip variables defined downstream (from project-source
            // demand propagation). Same filter as collect_branch_edge_decs.
            if !defined_at_or_before.contains(&var) {
                continue;
            }
            // Ownership transferred → callee handles normal cleanup.
            if invoke_transfers_ownership(var, args, arg_ownership) {
                continue;
            }

            // Check unwind path — skip if already handled by Category 3.
            if !cat3_unwind_vars.contains(&var) {
                let unwind_state = edge_state
                    .unwind
                    .get(&var)
                    .copied()
                    .unwrap_or(crate::aims::lattice::AimsState::BOTTOM);
                if unwind_state.cardinality == Cardinality::Absent {
                    if let Some(strategy) = rc_strategy(func, var, pool) {
                        edge_decs.push((block_idx, unwind.index(), var, strategy));
                    }
                }
            }

            // Check normal path.
            let normal_state = edge_state
                .normal
                .get(&var)
                .copied()
                .unwrap_or(crate::aims::lattice::AimsState::BOTTOM);
            if normal_state.cardinality == Cardinality::Absent {
                if let Some(strategy) = rc_strategy(func, var, pool) {
                    edge_decs.push((block_idx, normal.index(), var, strategy));
                }
            }
        }
    }

    // Category 2: borrowed Invoke args absent from exit_states — caller must
    // still RcDec. Emit on both edges. Owned args excluded (transferred).
    for (i, &arg) in args.iter().enumerate() {
        if arg_ownership.get(i).copied() != Some(ArgOwnership::Borrowed) {
            continue;
        }
        if state_map.is_excluded(arg) {
            continue;
        }
        // Skip Project-defined (borrowed) vars — source aggregate handles RC.
        if all_borrowed_defs.contains(&arg) {
            continue;
        }
        let in_exit_states = exit_states.is_some_and(|states| {
            states.get(&arg).is_some_and(|s| {
                is_owned_for_rc(state_map, arg, s.access, s.cardinality, all_borrowed_defs)
            })
        });
        if in_exit_states {
            continue;
        }
        if let Some(strategy) = rc_strategy(func, arg, pool) {
            edge_decs.push((block_idx, normal.index(), arg, strategy));
            edge_decs.push((block_idx, unwind.index(), arg, strategy));
        }
    }
}

/// Check whether an Invoke transfers ownership of `var` at any argument position.
///
/// If the variable appears at any `Owned` position, the callee takes ownership
/// and the caller must not emit cleanup for that variable. Conservative: if the
/// same variable appears at multiple positions and ANY is Owned, treat as
/// transferred (the callee received at least one owned reference).
#[inline]
fn invoke_transfers_ownership(
    var: ArcVarId,
    args: &[ArcVarId],
    arg_ownership: &[ArgOwnership],
) -> bool {
    args.iter().enumerate().any(|(i, &arg)| {
        arg == var
            && arg_ownership
                .get(i)
                .is_none_or(|o| *o == ArgOwnership::Owned)
    })
}
