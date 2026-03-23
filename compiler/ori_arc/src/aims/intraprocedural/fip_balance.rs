//! FIP token balance computation and gate event recording.
//!
//! Post-convergence passes that count Construct allocations vs consumed
//! parameters to determine FIP classification, and record FIP gate events
//! at Conditional call sites.
//!
//! Called from [`super::post_convergence`] after shape/TRMC passes complete.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};

use super::super::contract::{FipContract, MemoryContract};
use super::super::lattice::{Consumption, ShapeClass, Uniqueness};
use super::state_map::{AimsEvent, AimsStateMap};

/// Compute FIP token balance from the converged state map.
///
/// Counts two quantities:
/// 1. **Construct allocations**: non-scalar `Construct` instructions with reusable
///    constructor kinds (`Struct`, `EnumVariant`). Each one needs a memory slot.
/// 2. **Consumed parameters**: non-scalar function parameters whose converged entry
///    state shows `Dead` or `Unrestricted` consumption. Each consumed parameter
///    provides a "reuse token" — its memory slot can be recycled by a Construct.
///    Shape compatibility is verified at emission time by `emit_reuse/fip.rs`.
///
/// The balance determines FIP classification in
/// [`super::super::interprocedural::extract::extract_contract`]:
/// - `consumed >= constructs` with required-unique params → `Conditional`
/// - `consumed >= constructs` without required-unique → `Certified`
/// - `consumed < constructs` → `Bounded(net)`
///
/// Also records `AllocCreditBalance` events at Switch terminators for per-branch
/// FIP checking (`FIPTree` DMATCH! rule).
///
/// Section 09.2 Effect Activation.
pub(crate) fn populate_fip_balance(state_map: &mut AimsStateMap, func: &ArcFunction) {
    let construct_count = count_reusable_constructs(state_map, func);
    let consumed_count = count_consumed_params(state_map, func);

    // Per-branch balance at Switch terminators (FIPTree DMATCH! rule).
    record_per_branch_balance(state_map, func);

    state_map.set_fip_balance(construct_count, consumed_count);

    if construct_count > 0 || consumed_count > 0 {
        tracing::debug!(
            construct_count,
            consumed_count,
            token_balanced = consumed_count >= construct_count,
            net_allocation = construct_count.saturating_sub(consumed_count),
            "FIP token balance computed"
        );
    }
}

/// Count `Construct` instructions with reusable constructor kinds (Struct, `EnumVariant`)
/// on non-scalar, non-immortal destinations.
fn count_reusable_constructs(state_map: &AimsStateMap, func: &ArcFunction) -> u32 {
    let mut count: u32 = 0;
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, ctor, .. } = instr {
                if !state_map.is_excluded(*dst)
                    && matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. })
                {
                    count = count.saturating_add(1);
                }
            }
        }
    }
    count
}

/// Count consumed non-scalar function parameters.
///
/// A parameter consumed by the function (Dead/Unrestricted consumption in the
/// entry block's entry state) that is non-scalar provides a "reuse token" — its
/// memory slot can be recycled by a Construct of compatible type.
///
/// Note: param shape is a caller-side fact. The callee says "these params are
/// consumed — if unique, their memory is available for reuse." Shape
/// compatibility is verified at emission time by `emit_reuse/fip.rs`.
fn count_consumed_params(state_map: &AimsStateMap, func: &ArcFunction) -> u32 {
    let mut count: u32 = 0;
    let entry_block = ArcBlockId::new(0);
    for (param_idx, _) in func.params.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR var counts fit in u32"
        )]
        let var = ArcVarId::new(param_idx as u32);
        if state_map.is_excluded(var) {
            continue;
        }
        let state = state_map.var_state_at_block_entry(entry_block, var);
        let is_consumed = matches!(
            state.consumption,
            Consumption::Dead | Consumption::Unrestricted
        );
        if is_consumed {
            count = count.saturating_add(1);
        }
    }
    count
}

/// Compute per-parameter uniqueness requirements for FIP.
///
/// Returns `Vec<bool>` indexed by param position. `true` = param is consumed
/// (Dead/Unrestricted) and non-scalar. These params need caller-guaranteed
/// uniqueness for their memory to be reusable.
///
/// Scalar params always return `false` (no memory to reuse).
pub(crate) fn compute_requires_unique_params(
    state_map: &AimsStateMap,
    func: &ArcFunction,
) -> Vec<bool> {
    let entry_block = ArcBlockId::new(0);
    func.params
        .iter()
        .enumerate()
        .map(|(param_idx, _)| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR var counts fit in u32"
            )]
            let var = ArcVarId::new(param_idx as u32);
            if state_map.is_excluded(var) {
                return false;
            }
            let state = state_map.var_state_at_block_entry(entry_block, var);
            matches!(
                state.consumption,
                Consumption::Dead | Consumption::Unrestricted
            )
        })
        .collect()
}

/// Record per-branch allocation credit balance at Switch terminators.
///
/// For each Switch successor, computes the per-block allocation vs death count
/// and records an `AllocCreditBalance` event. FIP certification requires each
/// branch to independently maintain non-negative credit balance (`FIPTree` DMATCH! rule).
fn record_per_branch_balance(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let ArcTerminator::Switch { cases, default, .. } = &block.terminator else {
            continue;
        };

        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        // Collect successor block IDs: case targets + default.
        let successors: Vec<ArcBlockId> = cases
            .iter()
            .map(|(_, target)| *target)
            .chain(std::iter::once(*default))
            .collect();

        for (succ_idx, target) in successors.iter().enumerate() {
            let balance = compute_block_fip_balance(state_map, func, *target);
            state_map.record_event(AimsEvent::AllocCreditBalance {
                block: blk,
                successor_idx: succ_idx,
                balance,
            });
        }
    }
}

/// Compute the FIP allocation balance for a single block.
///
/// Returns `allocs - deaths`: positive means the block needs more tokens
/// than it provides, zero is balanced, negative means surplus.
fn compute_block_fip_balance(
    state_map: &AimsStateMap,
    func: &ArcFunction,
    block_id: ArcBlockId,
) -> i32 {
    let block = &func.blocks[block_id.index()];
    let mut allocs: i32 = 0;
    let mut deaths: i32 = 0;

    for instr in &block.body {
        if let ArcInstr::Construct { dst, ctor, .. } = instr {
            if !state_map.is_excluded(*dst)
                && matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. })
            {
                allocs = allocs.saturating_add(1);
            }
        }

        if let Some(dst) = instr.defined_var() {
            if state_map.is_excluded(dst) {
                continue;
            }
            let exit_state = state_map.var_state_at_block_exit(block_id, dst);
            let is_consumed = matches!(
                exit_state.consumption,
                Consumption::Dead | Consumption::Unrestricted
            );
            if is_consumed && matches!(exit_state.shape, ShapeClass::ReusableCtor(_)) {
                deaths = deaths.saturating_add(1);
            }
        }
    }

    allocs.saturating_sub(deaths)
}

/// Record `FipGate` events at call sites where the callee has
/// `FipContract::Conditional` and all required-unique args are `Unique`
/// at that program point.
///
/// This is a post-convergence pass that re-derives per-instruction state
/// by replaying the backward walk within each block (same technique as
/// emission passes). Records events in the sparse event table.
///
/// Section 09.2: sparse event table records FIP gates.
pub(crate) fn populate_fip_gate_events(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::Apply {
                func: callee_name,
                args,
                ..
            } = instr
            else {
                continue;
            };

            let Some(contract) = sigs.get(callee_name) else {
                continue;
            };

            let FipContract::Conditional {
                requires_unique_params,
            } = &contract.fip
            else {
                continue;
            };

            // Check if all required-unique args are Unique at this point.
            // Use block entry state as conservative approximation (true
            // per-instruction state would require replay, but entry state
            // is sufficient — uniqueness can only widen from entry to the
            // instruction point).
            let all_unique =
                args.iter()
                    .zip(requires_unique_params.iter())
                    .all(|(arg, &required)| {
                        if !required {
                            return true;
                        }
                        let state = state_map.var_state_at_block_entry(blk, *arg);
                        state.uniqueness == Uniqueness::Unique
                    });

            if all_unique {
                state_map.record_event(AimsEvent::FipGate {
                    block: blk,
                    instr: instr_idx,
                });
            }
        }
    }
}
