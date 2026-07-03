//! Effect accumulation during backward analysis.
//!
//! Computes per-block [`EffectSummary`] by accumulating effects from
//! instructions and terminators during the backward walk. Effects are
//! forward-aggregated (monotonically OR'd) — there is no "backward
//! direction" for effects.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::ArcTerminator;

use super::super::contract::{EffectSummary, MemoryContract};
use super::super::lattice::{AimsState, Locality};
use super::state_map::AimsStateMap;

/// Accumulate effects from an instruction into the block-level summary.
///
/// Called during the backward walk. `dst_demand` is the destination variable's
/// demand state BEFORE it is removed from the current state (captures the
/// downstream demand including locality, needed for `HeapEscaping` → `may_share`).
pub(super) fn accumulate_instr_effects(
    instr: &crate::ir::ArcInstr,
    dst_demand: Option<AimsState>,
    state_map: &AimsStateMap,
    sigs: &FxHashMap<Name, MemoryContract>,
    effects: &mut EffectSummary,
) {
    match instr {
        // Why: HeapEscaping locality propagates may_share to non-scalar constructor arguments.
        crate::ir::ArcInstr::Construct { dst, args, .. } => {
            if !state_map.is_excluded(*dst) {
                effects.may_allocate = true;
                if !args.is_empty() {
                    if let Some(demand) = dst_demand {
                        if demand.locality > Locality::BlockLocal {
                            effects.may_share = true;
                        }
                    }
                }
            }
        }

        // Why: Partial application allocates a closure environment on the heap.
        crate::ir::ArcInstr::PartialApply { dst, .. } => {
            if !state_map.is_excluded(*dst) {
                effects.may_allocate = true;
            }
        }

        crate::ir::ArcInstr::Apply { func: callee, .. } => {
            if let Some(contract) = sigs.get(callee) {
                *effects = effects.join(&contract.effects);
            }
        }

        _ => {}
    }
}

/// Accumulates callee contract and unwind effects for a terminator.
pub(super) fn accumulate_terminator_effects(
    term: &ArcTerminator,
    sigs: &FxHashMap<Name, MemoryContract>,
    effects: &mut EffectSummary,
) {
    if let ArcTerminator::Invoke { func: callee, .. } = term {
        effects.may_throw = true;
        if let Some(contract) = sigs.get(callee) {
            *effects = effects.join(&contract.effects);
        }
    }
}
