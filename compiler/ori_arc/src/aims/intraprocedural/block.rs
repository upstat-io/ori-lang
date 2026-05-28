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

use super::super::contract::{EffectSummary, FipContract, MemoryContract};
use super::super::lattice::{AccessClass, AimsState, Cardinality, Locality, Uniqueness};
use super::super::transfer::{backward_demands, backward_terminator_demands, transfer_def};
use super::effects::{accumulate_instr_effects, accumulate_terminator_effects};
use super::project_aliases::propagate_project_source_demand;
use super::state_map::AimsStateMap;

/// Result of computing a block's entry state.
///
/// Contains the backward-computed entry state (per-variable demand), the
/// block's accumulated effect summary (forward effect aggregation), and
/// any Invoke-defined dst demand captured BEFORE the strip removed it
/// from the entry state.
pub(crate) struct BlockAnalysisResult {
    /// Per-variable demand at block entry.
    pub entry_state: FxHashMap<ArcVarId, AimsState>,
    /// Effects accumulated from instructions in this block.
    /// Effect computation: precise effect computation during analysis.
    pub effects: EffectSummary,
    /// Pre-strip demand for Invoke-defined dsts that flow into this block's
    /// entry state. Captured at the strip site so predecessor lookups can
    /// recover the post-def demand (e.g., Return-widened locality) that the
    /// strip erases from `entry_state`. Maps `dst → AimsState`. Empty when
    /// this block has no Invoke-defined dsts. The owner Invoke block is
    /// determined by the caller via the `invoke_dst_to_owner` inverse map.
    pub invoke_def_demand: FxHashMap<ArcVarId, AimsState>,
}

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
///
/// # Loop convergence at back-edges
///
/// At a loop back-edge (where the successor is the loop header), this
/// function joins the loop header's current entry state with the loop body's
/// exit contribution. Because `alt_join` = `max` on each dimension, and all
/// dimensions are finite-height lattices, the demand on loop-carried variables
/// can only increase or stay the same across worklist iterations. This
/// guarantees monotone convergence without the demand-stabilization tricks
/// that GHC requires for lazy evaluation (`reuseEnv`, weak free variables).
/// (See: Sergey et al., POPL 2014 — GHC Demand Analysis)
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

    // Cross-block locality widening (effect computation): variables demanded by
    // successor blocks have crossed a block boundary, so their locality
    // must be at least `FunctionLocal`. This converts `BlockLocal` values
    // that flow to other blocks into `FunctionLocal`.
    for state in exit_state.values_mut() {
        if state.locality < Locality::FunctionLocal {
            state.locality = Locality::FunctionLocal;
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
/// Also accumulates a block-level [`EffectSummary`] from instruction types
/// (Effect computation: precise effect computation during analysis, not post-hoc).
///
/// `invoke_defs` maps block IDs to variables defined by Invoke terminators
/// in predecessor blocks. These are defined at this block's entry (normal
/// successor only) and are removed from the entry state, like block params.
pub(crate) fn compute_block_entry_state(
    func: &ArcFunction,
    block_id: ArcBlockId,
    state_map: &AimsStateMap,
    sigs: &FxHashMap<Name, MemoryContract>,
    invoke_defs: &FxHashMap<ArcBlockId, Vec<ArcVarId>>,
    project_alias_sources: &FxHashMap<ArcVarId, super::project_aliases::ProjectSources>,
) -> BlockAnalysisResult {
    let block = &func.blocks[block_id.index()];

    // Start from exit state (demand from successors).
    let mut current = state_map
        .block_exit_states(block_id)
        .cloned()
        .unwrap_or_default();

    // Block-level effect accumulator (Effect computation: precise effect computation).
    // Effects are forward-aggregated during the backward walk — monotonically
    // accumulated (OR) from instruction types and callee contracts. There is no
    // "backward direction" for effects.
    let mut block_effects = EffectSummary::default();

    // Apply terminator backward demands.
    apply_terminator_demands(&block.terminator, &mut current, state_map, sigs);

    // Terminator effects (effect computation).
    accumulate_terminator_effects(&block.terminator, sigs, &mut block_effects);

    // Return widening (effect computation): returned values escape the function,
    // so their locality must be `HeapEscaping` and access must be `Owned`
    // (the function transfers ownership to the caller).
    if let ArcTerminator::Return { value } = &block.terminator {
        if !state_map.is_excluded(*value) {
            let entry = current.entry(*value).or_insert(AimsState::BOTTOM);
            // Returned values are owned — the function transfers ownership.
            entry.access = AccessClass::Owned;
            if entry.locality < Locality::HeapEscaping {
                entry.locality = Locality::HeapEscaping;
            }
            entry.canonicalize();
        }
    }

    // Walk instructions in reverse order.
    for instr in block.body.iter().rev() {
        // Forward transfer: compute the state for the defined variable.
        // Create the closure inside a limited scope to avoid borrowing
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
            // Capture destination demand BEFORE removing dst, used for:
            // - Closure-capture-aware locality
            // - Effect accumulation (HeapEscaping → may_share)
            let dst_demand = instr
                .defined_var()
                .map(|dst| current.get(&dst).copied().unwrap_or(AimsState::BOTTOM));

            // Capture the closure's downstream demand BEFORE removing dst.
            let closure_demand = if let crate::ir::ArcInstr::PartialApply { dst, .. } = instr {
                Some(current.get(dst).copied().unwrap_or(AimsState::BOTTOM))
            } else {
                None
            };

            // Effect computation: accumulate per-instruction effects.
            accumulate_instr_effects(instr, dst_demand, state_map, sigs, &mut block_effects);

            // Transparent-alias transfer: for Let { Var(v) }, dst's accumulated
            // demand transfers to v via seq_add (cardinality + consumption)
            // and max (locality) BEFORE dst's state is consumed by the remove
            // below. Without this, accumulated Many cardinality on the alias
            // vanishes when dst is removed, leaving the source under-demanded
            // and a missing RcDec at the alias chain's natural-death site.
            if let crate::ir::ArcInstr::Let {
                value: crate::ir::ArcValue::Var(v),
                ..
            } = instr
            {
                if !state_map.is_excluded(*v) {
                    if let Some(dst_state) = dst_demand {
                        let entry = current.entry(*v).or_insert(AimsState::BOTTOM);
                        entry.cardinality = entry.cardinality.seq_add(dst_state.cardinality);
                        entry.consumption = entry.consumption.seq_add(dst_state.consumption);
                        if entry.locality < dst_state.locality {
                            entry.locality = dst_state.locality;
                        }
                        entry.canonicalize();
                    }
                }
            }

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
                apply_callee_contract(*dst, *callee, args, sigs, &mut current);
                // backward_demands still runs below to add operand demand.
            }

            // For PartialApply: update captured variables' states.
            // Uses closure-aware locality (effect computation): the closure's own
            // demand state determines captured variable locality.
            if let crate::ir::ArcInstr::PartialApply { args, .. } = instr {
                let closure_state = closure_demand.unwrap_or(AimsState::BOTTOM);
                for &arg in args {
                    if !state_map.is_excluded(arg) {
                        let arg_state = current.get(&arg).copied().unwrap_or(AimsState::BOTTOM);
                        let updated = super::super::transfer::capture_state_update(
                            &arg_state,
                            &closure_state,
                        );
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
    propagate_project_source_demand(
        &mut current,
        state_map,
        project_alias_sources,
        &block.params,
    );

    for &(param_var, _ty) in &block.params {
        current.remove(&param_var);
    }

    // Invoke defs: Invoke { dst, normal, .. } defines `dst` at the entry
    // of the `normal` successor only. These act like block params and must
    // be removed from the entry state so demand doesn't propagate backward
    // past the definition point.
    //
    // BUT: the predecessor (the Invoke block) needs the post-def demand on
    // `dst` for `var_state_at_block_exit(invoke_block, dst)` queries from
    // FIP balance + LocalAllocCandidate emission. Without capture, those
    // queries return BOTTOM and miss Return-widened locality / consumed
    // demand from the normal successor's body. Capture here BEFORE removing.
    let mut invoke_def_demand: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
    if let Some(vars) = invoke_defs.get(&block_id) {
        for &var in vars {
            if let Some(state) = current.get(&var).copied() {
                invoke_def_demand.insert(var, state);
            }
            current.remove(&var);
        }
    }

    BlockAnalysisResult {
        entry_state: current,
        effects: block_effects,
        invoke_def_demand,
    }
}

/// Apply backward demands from a terminator.
///
/// In addition to cardinality demand, applies:
/// - Locality widening (effect computation): Jump/Invoke args → `FunctionLocal`
/// - Uniqueness handling (effect computation): Invoke args get contract-aware
///   uniqueness demand (same rule as Apply in `apply_callee_contract`)
fn apply_terminator_demands(
    term: &ArcTerminator,
    current: &mut FxHashMap<ArcVarId, AimsState>,
    state_map: &AimsStateMap,
    sigs: &FxHashMap<Name, MemoryContract>,
) {
    let demands = backward_terminator_demands(term);
    for (var, card) in demands {
        if state_map.is_excluded(var) {
            continue;
        }
        add_backward_demand(current, var, card);
    }

    // Cross-block locality widening for terminator arguments.
    // Jump/Invoke args flow to a different block, so their locality
    // must be at least FunctionLocal.
    match term {
        ArcTerminator::Jump { args, .. } | ArcTerminator::Invoke { args, .. } => {
            for &arg in args {
                if !state_map.is_excluded(arg) {
                    widen_locality(current, arg, Locality::FunctionLocal);
                }
            }
        }
        _ => {}
    }

    // Contract-aware: contract-aware uniqueness for Invoke args.
    // Same rule as apply_callee_contract() for Apply instructions,
    // including FIP Conditional specialization.
    if let ArcTerminator::Invoke {
        func: callee, args, ..
    } = term
    {
        if let Some(contract) = sigs.get(callee) {
            let effective_may_share = compute_effective_may_share(contract, args, current);
            for (arg, param_contract) in args.iter().zip(contract.params.iter()) {
                if state_map.is_excluded(*arg) {
                    continue;
                }
                if effective_may_share && param_contract.access == AccessClass::Borrowed {
                    widen_uniqueness(current, *arg, Uniqueness::MaybeShared);
                }
            }
        }
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

/// Widen a variable's locality to at least `min_locality`.
fn widen_locality(
    current: &mut FxHashMap<ArcVarId, AimsState>,
    var: ArcVarId,
    min_locality: Locality,
) {
    let entry = current.entry(var).or_insert(AimsState::BOTTOM);
    if entry.locality < min_locality {
        entry.locality = min_locality;
    }
    entry.canonicalize();
}

/// Widen a variable's uniqueness to at least `min_uniqueness`.
///
/// Used by the transfer fusion rule: when a callee's
/// `EffectSummary.may_share == true`, borrowed arguments' uniqueness
/// is widened to `MaybeShared` (the callee might create new references).
fn widen_uniqueness(
    current: &mut FxHashMap<ArcVarId, AimsState>,
    var: ArcVarId,
    min_uniqueness: Uniqueness,
) {
    let entry = current.entry(var).or_insert(AimsState::BOTTOM);
    if entry.uniqueness < min_uniqueness {
        entry.uniqueness = min_uniqueness;
    }
    entry.canonicalize();
}

/// Merge a demand state into the current map for a variable.
fn merge_demand(current: &mut FxHashMap<ArcVarId, AimsState>, var: ArcVarId, state: AimsState) {
    let entry = current.entry(var).or_insert(AimsState::BOTTOM);
    *entry = entry.join(&state);
}

/// Compute effective `may_share` for a call site, accounting for FIP Conditional.
///
/// If the callee has `FipContract::Conditional` and all required-unique args
/// have `Uniqueness::Unique` in the caller's current backward state (meaning
/// nothing downstream has required them to be shared), the FIP fast path is
/// viable; `may_share` widening suppressed.
///
/// Effect computation: FIP call-site specialization.
fn compute_effective_may_share(
    contract: &MemoryContract,
    args: &[ArcVarId],
    current: &FxHashMap<ArcVarId, AimsState>,
) -> bool {
    if !contract.effects.may_share {
        return false;
    }
    if let FipContract::Conditional {
        requires_unique_params,
    } = &contract.fip
    {
        // Check if all required-unique args have Unique backward state.
        let preconditions_met =
            args.iter()
                .zip(requires_unique_params.iter())
                .all(|(arg, &required)| {
                    if !required {
                        return true;
                    }
                    let state = current.get(arg).copied().unwrap_or(AimsState::BOTTOM);
                    state.uniqueness == Uniqueness::Unique
                });
        if preconditions_met {
            return false;
        }
    }
    contract.effects.may_share
}

/// Apply callee contract to refine demand at a call site.
///
/// If the callee has a `MemoryContract` in `sigs`, use it to set
/// precise demand on arguments. Otherwise, fall back to conservative
/// (all args Owned/Unrestricted/Many).
///
/// # Uniqueness handling (transfer fusion)
///
/// When the callee's effective `may_share` is true (accounting for FIP
/// Conditional specialization), borrowed arguments might have their refcount
/// incremented by the callee. In the backward direction, this widens the
/// argument's uniqueness to `MaybeShared`.
///
/// When effective `may_share == false` (pure callee, or FIP Conditional with
/// all preconditions met), borrowed arguments preserve uniqueness through the
/// call.
///
/// Soundness (Marshall et al., ESOP 2022): this rule does NOT derive
/// uniqueness from consumption or cardinality alone. It bridges the gap
/// via the callee's `EffectSummary.may_share` — a past-facing fact about
/// whether the callee has created new references.
fn apply_callee_contract(
    _dst: ArcVarId,
    callee: Name,
    args: &[ArcVarId],
    sigs: &FxHashMap<Name, MemoryContract>,
    current: &mut FxHashMap<ArcVarId, AimsState>,
) {
    if let Some(contract) = sigs.get(&callee) {
        // Effect computation: FIP call-site specialization. If the callee is
        // Conditional FIP and all required-unique args are currently Unique,
        // suppress may_share widening.
        let effective_may_share = compute_effective_may_share(contract, args, current);

        // Use per-parameter contracts from interprocedural analysis.
        // Includes locality_bound (effect computation): if the callee may store
        // a parameter into a heap structure, the arg gets HeapEscaping locality.
        for (arg, param_contract) in args.iter().zip(contract.params.iter()) {
            // Contract-aware: uniqueness demand based on callee effects.
            // If effective may_share AND this param is borrowed,
            // the callee might RcInc the argument → widen to MaybeShared.
            // If pure / FIP Conditional met, borrowed args preserve uniqueness.
            let uniqueness_demand =
                if effective_may_share && param_contract.access == AccessClass::Borrowed {
                    Uniqueness::MaybeShared
                } else {
                    Uniqueness::Unique // BOTTOM — no widening
                };
            let demand = AimsState {
                access: param_contract.access,
                consumption: param_contract.consumption,
                cardinality: param_contract.cardinality,
                uniqueness: uniqueness_demand,
                locality: param_contract.locality_bound,
                ..AimsState::BOTTOM
            };
            merge_demand(current, *arg, demand);
        }
    }
    // If no contract found, backward_demands (called after this) already
    // adds Once demand per arg. The conservative Apply transfer function
    // handles the dst state.
}
