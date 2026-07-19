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
use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::successor_block_ids;
use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};

use super::super::contract::{EffectSummary, FipContract, MemoryContract};
use super::super::demand::RawDemand;
use super::super::lattice::{AccessClass, AimsState, Cardinality, Locality, Uniqueness};
use super::super::transfer::{
    backward_demands, backward_terminator_demands, transfer_def_resolved, BackwardDemand,
};
use super::block_state::BlockState;
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
    /// Pre-strip demand for intra-block instruction-defined dsts (e.g. a fresh
    /// `Construct` consumed within this block). Captured BEFORE
    /// IA-5 step (3) strips the dst, so a downstream
    /// `var_state_at_definition` query recovers the converged backward-demand
    /// (`seqAdd`-accumulated cardinality) the strip erases from `entry_state`.
    /// Maps `dst → AimsState`. Keyed into `AimsStateMap.def_demand` by the
    /// caller as `(this_block, dst)`.
    pub def_demand: FxHashMap<ArcVarId, AimsState>,
    /// L-9-excluded scalar values live at block entry. This binary sidecar
    /// participates in the same backward fixed point without creating scalar
    /// product-lattice states.
    pub scalar_live_at_entry: FxHashSet<ArcVarId>,
}

/// Managed demand and scalar liveness at one block exit.
pub(crate) struct BlockExitState {
    pub demands: FxHashMap<ArcVarId, AimsState>,
    pub live_scalars: FxHashSet<ArcVarId>,
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
) -> BlockExitState {
    let block = &func.blocks[block_id.index()];
    let successors = successor_block_ids(&block.terminator);

    if successors.is_empty() {
        // Terminal block (Return, Resume, Unreachable) — no successor demand.
        // On Return, the return value's demand is added by the terminator
        // transfer function during entry state computation.
        return BlockExitState {
            demands: FxHashMap::default(),
            live_scalars: FxHashSet::default(),
        };
    }

    let mut exit_state: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
    let mut live_scalars = FxHashSet::default();

    for succ_id in successors {
        if let Some(succ_entry) = state_map.block_entry_states(succ_id) {
            for (&var, &succ_state) in succ_entry {
                // BOTTOM keys retain block-local occurrence evidence for edge
                // verification, but carry no demand into a predecessor.
                if state_map.is_excluded(var) || succ_state == AimsState::BOTTOM {
                    continue;
                }
                let joined = exit_state
                    .get(&var)
                    .map_or(succ_state, |existing| existing.join(&succ_state));
                exit_state.insert(var, joined);
            }
        }
        if let Some(succ_live_scalars) = state_map.scalar_live_at_entry(succ_id) {
            let mut edge_live_scalars = succ_live_scalars.clone();
            let successor = &func.blocks[succ_id.index()];
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                if *target == succ_id {
                    for (&arg, &(param, _)) in args.iter().zip(&successor.params) {
                        if edge_live_scalars.remove(&param) {
                            edge_live_scalars.insert(arg);
                        }
                    }
                }
            }
            for &(param, _) in &successor.params {
                edge_live_scalars.remove(&param);
            }
            live_scalars.extend(edge_live_scalars);
        }
    }

    // Cross-block locality widening: variables demanded by successor blocks
    // have crossed a block boundary, so locality widens to at least
    // `FunctionLocal` (converts `BlockLocal` to `FunctionLocal`).
    for state in exit_state.values_mut() {
        if state.locality < Locality::FunctionLocal {
            state.locality = Locality::FunctionLocal;
        }
    }

    BlockExitState {
        demands: exit_state,
        live_scalars,
    }
}

/// Compute the ENTRY state of a block by walking backward through its body.
///
/// Starts from the block's exit state, applies terminator transfer, then
/// walks instructions in reverse: each instruction adds backward demand via
/// `seq_add` and sets forward state via `transfer_def`. Also accumulates a
/// block-level [`EffectSummary`] from instruction types.
///
/// `invoke_defs` maps block IDs to variables defined by predecessor Invoke
/// terminators; they are removed from the entry state, like block params.
pub(crate) fn compute_block_entry_state(
    func: &ArcFunction,
    block_id: ArcBlockId,
    state_map: &AimsStateMap,
    sigs: &FxHashMap<Name, MemoryContract>,
    invoke_defs: &FxHashMap<ArcBlockId, Vec<ArcVarId>>,
    demand_sources: &FxHashMap<ArcVarId, super::project_aliases::ProjectSources>,
    select_alias_dsts: &FxHashSet<ArcVarId>,
) -> BlockAnalysisResult {
    let block = &func.blocks[block_id.index()];

    // Start from exit state (demand from successors).
    // INVARIANT: AimsStateMap pre-sizes block_exit_states to the function's
    // block count at construction, so every in-range block has an entry.
    let current = state_map
        .block_exit_states(block_id)
        .cloned()
        .unwrap_or_else(|| {
            unreachable!("block {block_id:?} out of range for pre-sized AimsStateMap")
        });
    let live_scalars = state_map
        .scalar_live_at_exit(block_id)
        .cloned()
        .unwrap_or_default();
    let mut current = BlockState::from_observed(current, live_scalars);

    // Block-level effect accumulator: effects are forward-aggregated during
    // the backward walk, monotonically accumulated (OR) from instruction
    // types and callee contracts — there is no "backward direction" for effects.
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
            current.mark_returned(*value);
        }
    }

    // Converged BACKWARD-demand at each intra-block instruction definition,
    // captured at the strip site so `var_state_at_definition` recovers it for
    // a var defined+consumed within this block (where block-exit is BOTTOM).
    let mut def_demand: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();

    // Walk instructions in reverse order.
    for instr in block.body.iter().rev() {
        // Forward transfer: compute the state for the defined variable.
        // Create the closure inside a limited scope to avoid borrowing
        // `current` across mutation points.
        let defines_value = {
            let get_state = |v: ArcVarId| -> AimsState {
                if state_map.is_excluded(v) {
                    return AimsState::SCALAR;
                }
                current.observe_or_bottom(v)
            };
            transfer_def_resolved(func, instr, &get_state).is_some()
        };

        if defines_value {
            // Capture the converged demand on the defined var BEFORE
            // IA-5 step (3) strips it — the `seqAdd`-accumulated
            // cardinality (TF-11) that is DP-3's proven input.
            if let Some(dst) = instr.defined_var() {
                if !state_map.is_excluded(dst) {
                    if let Some(state) = current.observe(dst) {
                        def_demand.insert(dst, state);
                    }
                }
            }
        }

        apply_instr_step_one(
            func,
            instr,
            &mut current,
            state_map,
            sigs,
            &mut block_effects,
        );

        // Backward demands: add demand on operands.
        let demands = backward_demands(instr);
        for demand in demands {
            if state_map.is_scalar(demand.var) {
                current.mark_scalar_live(demand.var);
                continue;
            }
            if state_map.is_immortal(demand.var) {
                continue;
            }
            add_backward_demand(&mut current, demand);
        }

        // IA-5 step (3): the destination is defined here, so predecessor
        // demand stops only after alias transfer and direct TF-11 demand.
        if defines_value {
            if let Some(dst) = instr.defined_var() {
                current.remove(dst);
            }
        }
    }

    // Exclude block params (which are definitions like phi nodes) from entry state.
    propagate_project_source_demand(
        &mut current,
        state_map,
        demand_sources,
        select_alias_dsts,
        &block.params,
    );

    let mut scalar_definition_demand = FxHashSet::default();
    for &(param_var, _ty) in &block.params {
        if state_map.is_scalar(param_var) && current.is_scalar_live(param_var) {
            scalar_definition_demand.insert(param_var);
        }
        current.remove(param_var);
    }

    // INVARIANT: Invoke-defined dsts act like block params (removed from
    // entry state so demand stops at the def point), but captured FIRST —
    // predecessor FIP queries need the post-def demand this removal erases.
    let mut invoke_def_demand: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
    if let Some(vars) = invoke_defs.get(&block_id) {
        for &var in vars {
            if state_map.is_scalar(var) && current.is_scalar_live(var) {
                scalar_definition_demand.insert(var);
            }
            if let Some(state) = current.observe(var) {
                invoke_def_demand.insert(var, state);
            }
            current.remove(var);
        }
    }

    let (entry_state, mut scalar_live_at_entry) = current.into_observed();
    scalar_live_at_entry.extend(scalar_definition_demand);
    BlockAnalysisResult {
        entry_state,
        effects: block_effects,
        invoke_def_demand,
        def_demand,
        scalar_live_at_entry,
    }
}

/// Apply IA-5 step (1) and update call/capture state.
///
/// Side-effecting instructions also pass through this function because `Set`
/// has an IA-5 value-promotion obligation despite defining no destination.
fn apply_instr_step_one(
    func: &ArcFunction,
    instr: &crate::ir::ArcInstr,
    current: &mut BlockState,
    state_map: &AimsStateMap,
    sigs: &FxHashMap<Name, MemoryContract>,
    block_effects: &mut EffectSummary,
) {
    let scalar_destination_live = instr
        .defined_var()
        .is_some_and(|dst| state_map.is_scalar(dst) && current.is_scalar_live(dst));
    // Capture destination demand BEFORE removing dst, used for:
    // - Closure-capture-aware locality
    // - Effect accumulation (HeapEscaping → may_share)
    let dst_demand = instr
        .defined_var()
        .map(|dst| current.observe_or_bottom(dst));
    // Capture the closure's downstream demand BEFORE removing dst.
    let closure_demand = if let crate::ir::ArcInstr::PartialApply { dst, .. } = instr {
        Some(current.observe_or_bottom(*dst))
    } else {
        None
    };

    // Effect computation: accumulate per-instruction effects.
    accumulate_instr_effects(func, instr, dst_demand, state_map, sigs, block_effects);

    transfer_instruction_value_flow(
        instr,
        current,
        state_map,
        dst_demand,
        scalar_destination_live,
        block_effects,
    );
    apply_call_and_capture_state(instr, current, state_map, sigs, closure_demand);
}

/// Transfer destination demand through aliases and ownership-taking edges.
fn transfer_instruction_value_flow(
    instr: &crate::ir::ArcInstr,
    current: &mut BlockState,
    state_map: &AimsStateMap,
    dst_demand: Option<AimsState>,
    scalar_destination_live: bool,
    block_effects: &mut EffectSummary,
) {
    match instr {
        crate::ir::ArcInstr::Let {
            value: crate::ir::ArcValue::Var(source),
            ..
        } if state_map.is_scalar(*source) => {
            if scalar_destination_live {
                current.mark_scalar_live(*source);
            }
        }
        crate::ir::ArcInstr::Let {
            value: crate::ir::ArcValue::Var(source),
            ..
        } if !state_map.is_excluded(*source) => {
            if let Some(dst_state) = dst_demand {
                let source_remains_live = current.has_raw_demand(*source);
                if source_remains_live && dst_state.cardinality != Cardinality::Absent {
                    block_effects.may_share = true;
                }
                transfer_full_alias_demand(current, *source, dst_state);
            }
        }
        crate::ir::ArcInstr::Project {
            dst, value: source, ..
        } => {
            if state_map.is_scalar(*dst) {
                if state_map.is_scalar(*source) {
                    if scalar_destination_live {
                        current.mark_scalar_live(*source);
                    }
                } else if !state_map.is_excluded(*source) {
                    current.transfer_scalar_project(*source, scalar_destination_live);
                }
            } else if !state_map.is_excluded(*source) {
                if let Some(dst_state) = dst_demand {
                    transfer_project_demand(current, *source, dst_state);
                }
            }
        }
        crate::ir::ArcInstr::Select {
            true_val,
            false_val,
            ..
        } => {
            if let Some(dst_state) = dst_demand {
                for source in [*true_val, *false_val] {
                    if state_map.is_scalar(source) {
                        if scalar_destination_live {
                            current.mark_scalar_live(source);
                        }
                    } else if !state_map.is_excluded(source) {
                        transfer_full_alias_demand(current, source, dst_state);
                    }
                }
            }
        }
        crate::ir::ArcInstr::Construct { args, .. }
        | crate::ir::ArcInstr::Reuse { args, .. }
        | crate::ir::ArcInstr::CollectionReuse { args, .. } => {
            let locality = dst_demand.unwrap_or(AimsState::BOTTOM).locality;
            for &arg in args {
                if !state_map.is_excluded(arg) {
                    promote_owned_with_locality(current, arg, locality);
                }
            }
        }
        crate::ir::ArcInstr::Set { base, value, .. } => {
            if !state_map.is_excluded(*value) {
                let base_locality = current.observe_or_bottom(*base).locality;
                promote_owned_with_locality(current, *value, base_locality);
            }
        }
        _ => {}
    }
}

/// Apply exact callee contracts and closure-capture demand.
fn apply_call_and_capture_state(
    instr: &crate::ir::ArcInstr,
    current: &mut BlockState,
    state_map: &AimsStateMap,
    sigs: &FxHashMap<Name, MemoryContract>,
    closure_demand: Option<AimsState>,
) {
    if let crate::ir::ArcInstr::Apply {
        dst,
        func: callee,
        args,
        ..
    } = instr
    {
        apply_callee_contract(*dst, *callee, args, sigs, current, state_map);
    }

    if let crate::ir::ArcInstr::PartialApply { args, .. } = instr {
        let closure_state = closure_demand.unwrap_or(AimsState::BOTTOM);
        for &arg in args {
            if state_map.is_scalar(arg) {
                current.mark_scalar_live(arg);
            } else if !state_map.is_excluded(arg) {
                let arg_state = current.observe_or_bottom(arg);
                let updated =
                    super::super::transfer::capture_state_update(&arg_state, &closure_state);
                merge_demand(current, arg, updated);
            }
        }
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
    current: &mut BlockState,
    state_map: &AimsStateMap,
    sigs: &FxHashMap<Name, MemoryContract>,
) {
    let demands = backward_terminator_demands(term);
    let jump_uses_edge_mapping = matches!(term, ArcTerminator::Jump { .. });
    for demand in demands {
        if state_map.is_scalar(demand.var) {
            if !jump_uses_edge_mapping {
                current.mark_scalar_live(demand.var);
            }
            continue;
        }
        if state_map.is_immortal(demand.var) {
            continue;
        }
        add_backward_demand(current, demand);
    }

    if let ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } = term {
        if state_map.is_scalar(*dst) {
            current.remove(*dst);
        }
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
fn add_backward_demand(current: &mut BlockState, demand: BackwardDemand) {
    current.seq_add(
        demand.var,
        RawDemand::new(demand.cardinality, demand.consumption),
    );
}

/// IA-5 transparent or conditional alias transfer.
fn transfer_full_alias_demand(current: &mut BlockState, source: ArcVarId, destination: AimsState) {
    current.transfer_alias(source, destination);
}

/// TF-14 borrowed projection transfer.
fn transfer_project_demand(current: &mut BlockState, source: ArcVarId, destination: AimsState) {
    current.transfer_project(source, destination);
}

/// IA-5 ownership-taking aggregate-edge transfer.
fn promote_owned_with_locality(current: &mut BlockState, value: ArcVarId, locality: Locality) {
    current.promote_owned(value, locality);
}

/// Widen a variable's locality to at least `min_locality`.
fn widen_locality(current: &mut BlockState, var: ArcVarId, min_locality: Locality) {
    current.widen_locality(var, min_locality);
}

/// Widen a variable's uniqueness to at least `min_uniqueness`.
///
/// Used by the transfer fusion rule: when a callee's
/// `EffectSummary.may_share == true`, borrowed arguments' uniqueness
/// is widened to `MaybeShared` (the callee might create new references).
fn widen_uniqueness(current: &mut BlockState, var: ArcVarId, min_uniqueness: Uniqueness) {
    current.widen_uniqueness(var, min_uniqueness);
}

/// Merge a demand state into the current map for a variable.
fn merge_demand(current: &mut BlockState, var: ArcVarId, state: AimsState) {
    current.alt_join_state(var, state);
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
    current: &BlockState,
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
                    let state = current.observe_or_bottom(*arg);
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
/// Conditional specialization), a callee may mint a sharing credit for a
/// borrowed argument's storage identity. In the backward direction, this
/// widens the argument's uniqueness to `MaybeShared`.
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
    current: &mut BlockState,
    state_map: &AimsStateMap,
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
            if state_map.is_excluded(*arg) {
                continue;
            }
            // Contract-aware: uniqueness demand from callee effects — if
            // effective may_share AND borrowed, the callee might RcInc the
            // arg (widen to MaybeShared); else borrowed args preserve uniqueness.
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
    // If no contract is found, backward_demands will add Once demand per arg.
}
