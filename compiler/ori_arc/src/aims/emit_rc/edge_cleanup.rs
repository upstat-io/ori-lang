//! Edge cleanup for inter-block RC operations.
//!
//! Handles variables that are live in a predecessor but dead in a particular
//! successor. For single-predecessor successors, prepends `RcDec` at the
//! successor's entry. For multi-predecessor successors, inserts trampoline
//! blocks.

use rustc_hash::FxHashMap;

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{AccessClass, Cardinality};
use crate::graph::{compute_predecessors, successor_block_ids};
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

use super::{block_id, rc_strategy, EdgeDec};

/// Emit `RcDec` on edges where a variable is live in the predecessor but
/// dead in a particular successor.
pub(super) fn emit_edge_cleanup(func: &mut ArcFunction, state_map: &AimsStateMap, pool: &Pool) {
    let predecessors = compute_predecessors(func);

    // Collect edge cleanup operations: (pred_block, succ_block, var, strategy).
    let mut edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)> = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = block_id(block_idx);

        // Handle Invoke separately — use InvokeEdgeState.
        if matches!(block.terminator, ArcTerminator::Invoke { .. }) {
            collect_invoke_edge_decs(func, block_idx, state_map, pool, &mut edge_decs);
            continue;
        }

        let successors = successor_block_ids(&block.terminator);
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
            &mut edge_decs,
        );
    }

    apply_edge_decs(func, &predecessors, edge_decs);
}

/// Collect edge-specific `RcDec` for multi-successor blocks (Branch/Switch).
fn collect_branch_edge_decs(
    func: &ArcFunction,
    block_idx: usize,
    blk: ArcBlockId,
    successors: &[ArcBlockId],
    state_map: &AimsStateMap,
    pool: &Pool,
    edge_decs: &mut Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
    let Some(exit_states) = state_map.block_exit_states(blk) else {
        return;
    };
    for (&var, &state) in exit_states {
        if state.is_scalar() || state.access != AccessClass::Owned {
            continue;
        }
        if state.cardinality == Cardinality::Absent {
            continue;
        }
        for succ_id in successors {
            let succ_entry = state_map.var_state_at_block_entry(*succ_id, var);
            if succ_entry.cardinality == Cardinality::Absent
                || succ_entry.access != AccessClass::Owned
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
fn collect_invoke_edge_decs(
    func: &ArcFunction,
    block_idx: usize,
    state_map: &AimsStateMap,
    pool: &Pool,
    edge_decs: &mut Vec<(usize, usize, ArcVarId, RcStrategy)>,
) {
    let blk = block_id(block_idx);
    let ArcTerminator::Invoke { normal, unwind, .. } = &func.blocks[block_idx].terminator else {
        return;
    };
    let (normal, unwind) = (*normal, *unwind);

    let Some(edge_state) = state_map.invoke_edge_state(blk) else {
        return;
    };
    let Some(exit_states) = state_map.block_exit_states(blk) else {
        return;
    };

    for (&var, &state) in exit_states {
        if state.is_scalar() || state.access != AccessClass::Owned {
            continue;
        }
        if state.cardinality == Cardinality::Absent {
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
