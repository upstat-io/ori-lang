//! Tail-recursion-modulo-constructor evidence and context events.

use rustc_hash::FxHashSet;

use crate::aims::contract::ContextRegion;
use crate::aims::lattice::{ShapeClass, Uniqueness};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId, CtorKind};

use super::super::state_map::{AimsEvent, AimsStateMap};

/// Mark uniquely owned constructors fed by recursive calls as context holes.
pub(crate) fn detect_trmc_candidates(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    may_share: bool,
) {
    let recursive_defs: FxHashSet<ArcVarId> =
        crate::aims::normalize::collect_recursive_call_sites(func)
            .into_keys()
            .collect();
    if recursive_defs.is_empty() {
        return;
    }
    if may_share {
        tracing::trace!(
            func = ?func.name,
            "TRMC effect gate: may_share=true (logged, not enforced — no effect handlers in v1)"
        );
    }

    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);
        for instr in &block.body {
            let ArcInstr::Construct {
                dst, ctor, args, ..
            } = instr
            else {
                continue;
            };
            if !matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. })
                || state_map.is_excluded(*dst)
                || !args.iter().any(|arg| recursive_defs.contains(arg))
            {
                continue;
            }
            let state = state_map.var_state_at_block_exit(block_id, *dst);
            if state.uniqueness != Uniqueness::Unique {
                continue;
            }
            state_map.set_var_shape(*dst, ShapeClass::ContextHole);
            tracing::debug!(
                func = ?func.name,
                var = dst.raw(),
                block = block_id.raw(),
                "TRMC candidate detected: ContextHole shape set"
            );
        }
    }
}

/// Record paired context events for unique context-hole regions.
pub(crate) fn populate_context_events(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    context_regions: &[ContextRegion],
    may_share: bool,
) {
    if context_regions.is_empty() {
        return;
    }
    if may_share {
        tracing::trace!(
            func = ?func.name,
            "TRMC context events: may_share=true (logged, not enforced — no effect handlers in v1)"
        );
    }

    for region in context_regions {
        if state_map.is_excluded(region.context_var)
            || !matches!(
                state_map.var_shape(region.context_var),
                ShapeClass::ContextHole
            )
        {
            continue;
        }
        let state = state_map.var_state_at_block_exit(region.open_block, region.context_var);
        if state.uniqueness != Uniqueness::Unique {
            continue;
        }
        state_map.record_event(AimsEvent::ContextOpen {
            block: region.open_block,
            instr: region.open_instr,
            var: region.context_var,
        });
        state_map.record_event(AimsEvent::ContextClose {
            block: region.close_block,
            instr: region.close_instr,
            var: region.hole_var,
        });
        tracing::debug!(
            func = ?func.name,
            context_var = region.context_var.raw(),
            hole_var = region.hole_var.raw(),
            hole_field = region.hole_field,
            "recorded TRMC context events (open + close)"
        );
    }
}
