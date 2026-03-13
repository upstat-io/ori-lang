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
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, RcStrategy,
};

use super::{block_id, rc_strategy, EdgeDec};

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
/// Also handles deferred parent `RcDec` operations from Phase B. These are
/// parent aggregates whose `RcDec` was deferred because a borrowed child
/// (from Project) is used in the block terminator. The parent's `RcDec`
/// must go on ALL successor edges so it executes after the terminator.
pub(crate) fn emit_edge_cleanup(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    deferred_parent_decs: &FxHashMap<usize, Vec<(ArcVarId, RcStrategy)>>,
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
            // Add deferred parent decs to both Invoke edges.
            if let Some(decs) = deferred_parent_decs.get(&block_idx) {
                if let ArcTerminator::Invoke { normal, unwind, .. } = &block.terminator {
                    for &(var, strategy) in decs {
                        edge_decs.push((block_idx, normal.index(), var, strategy));
                        edge_decs.push((block_idx, unwind.index(), var, strategy));
                    }
                }
            }
            continue;
        }

        let successors = successor_block_ids(&block.terminator);

        // Add deferred parent decs to all successor edges.
        if let Some(decs) = deferred_parent_decs.get(&block_idx) {
            for succ_id in &successors {
                for &(var, strategy) in decs {
                    edge_decs.push((block_idx, succ_id.index(), var, strategy));
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
/// Two categories of variables need cleanup:
/// 1. Variables live at block exit (in `exit_states`) that die on specific
///    edges — handled by the `exit_states` loop.
/// 2. Borrowed Invoke args NOT in `exit_states` — these are used only by
///    the Invoke terminator, so the backward analysis never propagates them
///    to the exit boundary. The caller must still `RcDec` after the call.
///
/// Both categories exclude variables whose ownership was transferred to the
/// callee via `ArgOwnership::Owned`. Once ownership transfers, the caller
/// must not emit cleanup — the callee (or its iterator/drop) handles it.
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
            // Skip variables whose ownership was transferred to the callee.
            // If any Invoke arg position transfers this variable as Owned,
            // the caller must not also dec — the callee handles cleanup.
            if invoke_transfers_ownership(var, args, arg_ownership) {
                continue;
            }

            // Check unwind path.
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

    // Category 2: borrowed Invoke args not in exit_states.
    //
    // When a variable is passed as `Borrowed` to an Invoke, the caller retains
    // ownership and must `RcDec` after the call. If the variable is not live in
    // any successor (not in exit_states), the backward analysis never propagates
    // it to the exit boundary, so the exit_states loop above misses it.
    //
    // Emit `RcDec` on both normal and unwind edges for these variables.
    // (Owned args are excluded by the filter — ownership was transferred.)
    for (i, &arg) in args.iter().enumerate() {
        if arg_ownership.get(i).copied() != Some(ArgOwnership::Borrowed) {
            continue;
        }
        if state_map.is_excluded(arg) {
            continue;
        }
        // Skip Project-defined (borrowed) variables. These don't have
        // independent RC management — the source aggregate's RC covers them.
        if all_borrowed_defs.contains(&arg) {
            continue;
        }
        // Already handled by exit_states loop above.
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

/// Insert a trampoline block to carry edge-specific `RcDec` operations.
fn insert_trampoline(func: &mut ArcFunction, pred_idx: usize, succ_idx: usize, decs: &[EdgeDec]) {
    let trampoline_id = block_id(func.blocks.len());
    let succ_id = block_id(succ_idx);

    let body: Vec<ArcInstr> = decs
        .iter()
        .map(|&(var, strategy)| ArcInstr::RcDec { var, strategy })
        .collect();

    let jump_args = extract_jump_args_for_succ(&func.blocks[pred_idx].terminator, succ_id);

    let trampoline_block = ArcBlock {
        id: trampoline_id,
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Jump {
            target: succ_id,
            args: jump_args,
        },
    };

    func.blocks.push(trampoline_block);
    retarget_terminator(
        &mut func.blocks[pred_idx].terminator,
        succ_id,
        trampoline_id,
    );
}

/// Extract the jump arguments that would be passed to a specific successor.
fn extract_jump_args_for_succ(terminator: &ArcTerminator, succ_id: ArcBlockId) -> Vec<ArcVarId> {
    match terminator {
        ArcTerminator::Jump { args, target } if *target == succ_id => args.clone(),
        _ => Vec::new(),
    }
}

/// Retarget a terminator: replace references to `old_target` with `new_target`.
fn retarget_terminator(
    terminator: &mut ArcTerminator,
    old_target: ArcBlockId,
    new_target: ArcBlockId,
) {
    match terminator {
        ArcTerminator::Jump { target, .. } => {
            if *target == old_target {
                *target = new_target;
            }
        }
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            if *then_block == old_target {
                *then_block = new_target;
            }
            if *else_block == old_target {
                *else_block = new_target;
            }
        }
        ArcTerminator::Switch { cases, default, .. } => {
            for (_, target) in cases.iter_mut() {
                if *target == old_target {
                    *target = new_target;
                }
            }
            if *default == old_target {
                *default = new_target;
            }
        }
        ArcTerminator::Invoke { normal, unwind, .. } => {
            if *normal == old_target {
                *normal = new_target;
            }
            if *unwind == old_target {
                *unwind = new_target;
            }
        }
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
}
