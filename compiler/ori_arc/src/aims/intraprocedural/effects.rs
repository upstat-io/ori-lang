//! Effect accumulation during backward analysis (Section 09.2).
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
        // Construct (non-scalar): may_allocate. If destination demand has
        // locality > BlockLocal and the construct has args, may_share too
        // (Section 09.1: HeapEscaping → may_share).
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

        // PartialApply (non-scalar): may_allocate (closure env allocation).
        crate::ir::ArcInstr::PartialApply { dst, .. } => {
            if !state_map.is_excluded(*dst) {
                effects.may_allocate = true;
            }
        }

        // Apply with known contract: union callee's EffectSummary.
        crate::ir::ArcInstr::Apply { func: callee, .. } => {
            if let Some(contract) = sigs.get(callee) {
                *effects = effects.join(&contract.effects);
            }
        }

        _ => {}
    }
}

/// Accumulate effects from a terminator into the block-level summary.
///
/// `Invoke`: `may_throw` (Invoke exists because the call may unwind).
/// Also unions callee's [`EffectSummary`] if known.
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
