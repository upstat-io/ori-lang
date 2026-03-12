//! Per-block backward analysis.
//!
//! Computes block entry states from block exit states by walking instructions
//! in reverse and applying transfer functions. Also computes block exit states
//! by joining successor entry states.
//!
//! # Backward direction
//!
//! The analysis is backward: a block's EXIT state represents demand from
//! successors. Walking instructions in reverse through the block, each
//! instruction adds demand on its operands via `seq_add`. The resulting
//! ENTRY state represents the total demand placed on variables flowing
//! into this block.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::graph::successor_block_ids;
use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::super::contract::MemoryContract;
use super::super::lattice::{AimsState, Cardinality};
use super::super::transfer::{backward_demands, backward_terminator_demands, transfer_def};
use super::state_map::AimsStateMap;

/// Compute the EXIT state of a block from its successors' ENTRY states.
///
/// In backward analysis, a block's exit state is the join of all successor
/// blocks' entry states — these represent the demand that successors place
/// on variables flowing out of the current block.
///
/// Uses [`alt_join`](super::super::lattice::AimsState::join) (= `max`) for successor combination: at a branch/switch,
/// only ONE successor executes per dynamic run, so successor demands are
/// alternative. At a Jump (single successor), `alt_join` is trivially the
/// successor's state.
pub(crate) fn compute_block_exit_state(
    func: &ArcFunction,
    block_id: ArcBlockId,
    state_map: &AimsStateMap,
) -> FxHashMap<ArcVarId, AimsState> {
    let block = &func.blocks[block_id.index()];
    let successors = successor_block_ids(&block.terminator);

    if successors.is_empty() {
        // Terminal block (Return, Resume, Unreachable) — no successor demand.
        // For Return, the return value's demand is added by the terminator
        // transfer function during entry state computation.
        return FxHashMap::default();
    }

    let mut exit_state: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();

    for succ_id in successors {
        if let Some(succ_entry) = state_map.block_entry_states(succ_id) {
            for (&var, &succ_state) in succ_entry {
                if state_map.is_excluded(var) {
                    continue;
                }
                let joined = exit_state
                    .get(&var)
                    .map_or(succ_state, |existing| existing.join(&succ_state));
                exit_state.insert(var, joined);
            }
        }
    }

    exit_state
}

/// Compute the ENTRY state of a block by walking backward through its body.
///
/// Starts from the block's exit state (demand from successors), applies
/// terminator transfer, then walks instructions in reverse order applying
/// transfer functions. Each instruction:
/// 1. Adds backward demand on operands via `seq_add`
/// 2. Sets forward state for defined variables via `transfer_def`
///
/// `invoke_defs` maps block IDs to variables defined by Invoke terminators
/// in predecessor blocks. These are defined at this block's entry (normal
/// successor only) and are removed from the entry state, like block params.
pub(crate) fn compute_block_entry_state(
    func: &ArcFunction,
    block_id: ArcBlockId,
    state_map: &AimsStateMap,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    invoke_defs: &FxHashMap<ArcBlockId, Vec<ArcVarId>>,
) -> FxHashMap<ArcVarId, AimsState> {
    let block = &func.blocks[block_id.index()];

    // Start from exit state (demand from successors).
    let mut current = state_map
        .block_exit_states(block_id)
        .cloned()
        .unwrap_or_default();

    // Apply terminator backward demands.
    apply_terminator_demands(&block.terminator, &mut current, state_map, classifier);

    // Walk instructions in reverse order.
    for instr in block.body.iter().rev() {
        // Forward transfer: compute the state for the defined variable.
        // We create the closure inside a limited scope to avoid borrowing
        // `current` across mutation points.
        let def_transfer = {
            let get_state = |v: ArcVarId| -> AimsState {
                if state_map.is_excluded(v) {
                    return AimsState::SCALAR;
                }
                current.get(&v).copied().unwrap_or(AimsState::BOTTOM)
            };
            transfer_def(instr, &get_state)
        };

        if def_transfer.is_some() {
            // If the instruction defines a variable, the defined variable's
            // demand is consumed here (it's defined at this point, so
            // predecessors don't need to provide it). Remove from current state.
            if let Some(dst) = instr.defined_var() {
                current.remove(&dst);
            }

            // Use contract-aware state for Apply/Invoke if available.
            if let crate::ir::ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                apply_callee_contract(*dst, *callee, args, sigs, classifier, &mut current);
                // backward_demands still runs below to add operand demand.
            }

            // For PartialApply: update captured variables' states.
            if let crate::ir::ArcInstr::PartialApply { args, .. } = instr {
                for &arg in args {
                    if !state_map.is_excluded(arg) {
                        let arg_state = current.get(&arg).copied().unwrap_or(AimsState::BOTTOM);
                        let updated = super::super::transfer::capture_state_update(&arg_state);
                        merge_demand(&mut current, arg, updated);
                    }
                }
            }
        }

        // Backward demands: add demand on operands.
        let demands = backward_demands(instr);
        for (var, card) in demands {
            if state_map.is_excluded(var) {
                continue;
            }
            add_backward_demand(&mut current, var, card);
        }
    }

    // Block params are definitions (like phi nodes) — remove from entry state.
    for &(param_var, _ty) in &block.params {
        current.remove(&param_var);
    }

    // Invoke defs: Invoke { dst, normal, .. } defines `dst` at the entry
    // of the `normal` successor only. These act like block params and must
    // be removed from the entry state so demand doesn't propagate backward
    // past the definition point.
    if let Some(vars) = invoke_defs.get(&block_id) {
        for &var in vars {
            current.remove(&var);
        }
    }

    current
}

/// Apply backward demands from a terminator.
fn apply_terminator_demands(
    term: &ArcTerminator,
    current: &mut FxHashMap<ArcVarId, AimsState>,
    state_map: &AimsStateMap,
    _classifier: &dyn ArcClassification,
) {
    let demands = backward_terminator_demands(term);
    for (var, card) in demands {
        if state_map.is_excluded(var) {
            continue;
        }
        add_backward_demand(current, var, card);
    }
}

/// Add backward demand to a variable in the current state.
///
/// Uses `seq_add` for sequential composition: within a block, each
/// instruction adds demand on its operands sequentially.
fn add_backward_demand(
    current: &mut FxHashMap<ArcVarId, AimsState>,
    var: ArcVarId,
    demand: Cardinality,
) {
    let entry = current.entry(var).or_insert(AimsState::BOTTOM);
    // Bump cardinality via seq_add (sequential within block).
    entry.cardinality = entry.cardinality.seq_add(demand);
    // A variable with demand is at least Affine (may need drop if not used).
    if entry.consumption < crate::aims::lattice::Consumption::Affine {
        entry.consumption = crate::aims::lattice::Consumption::Affine;
    }
    entry.canonicalize();
}

/// Merge a demand state into the current map for a variable.
fn merge_demand(current: &mut FxHashMap<ArcVarId, AimsState>, var: ArcVarId, state: AimsState) {
    let entry = current.entry(var).or_insert(AimsState::BOTTOM);
    *entry = entry.join(&state);
}

/// Apply callee contract to refine demand at a call site.
///
/// If the callee has a `MemoryContract` in `sigs`, use it to set
/// precise demand on arguments. Otherwise, fall back to conservative
/// (all args Owned/Unrestricted/Many).
fn apply_callee_contract(
    _dst: ArcVarId,
    callee: Name,
    args: &[ArcVarId],
    sigs: &FxHashMap<Name, MemoryContract>,
    _classifier: &dyn ArcClassification,
    current: &mut FxHashMap<ArcVarId, AimsState>,
) {
    if let Some(contract) = sigs.get(&callee) {
        // Use per-parameter contracts from interprocedural analysis.
        for (arg, param_contract) in args.iter().zip(contract.params.iter()) {
            let demand = AimsState {
                access: param_contract.access,
                consumption: param_contract.consumption,
                cardinality: param_contract.cardinality,
                ..AimsState::BOTTOM
            };
            merge_demand(current, *arg, demand);
        }
    }
    // If no contract found, backward_demands (called after this) already
    // adds Once demand per arg. The conservative Apply transfer function
    // handles the dst state.
}
