//! Unified logical ownership-event realization with inline lifetime-event
//! collection.
//!
//! Phase 1 sub-step B of [`super::realize_rc_reuse()`]. Phase-7 mechanical
//! lowering lives in [`burden_lowering`]; jump-threaded same-allocation rep
//! tracking lives in [`jump_threaded_reps`] — both split out to keep every
//! file under the 500-line hygiene cap.

#[cfg(test)]
mod tests;

mod burden_lowering;
mod jump_threaded_reps;

use ori_ir::Name;
use ori_types::{Pool, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::emit_reuse::{AllocEvent, DeathEvent};
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

use super::metrics;
use burden_lowering::lower_burden_ops_to_rc;

pub use jump_threaded_reps::{push_receiver_lineage_returned, yield_result_for_receiver_lineage};

/// Per-phase snapshot of the current counter-shaped carrier for post-walk
/// adapter debugging.
///
/// Emits one `tracing::trace!` per block summarising every `RcInc`/`RcDec` by
/// `ArcVarId`. These are physical-projection migration metrics, not AIMS facts.
/// Gated behind `tracing::enabled!` — zero overhead when the
/// `ori_arc::aims::realize` target is below trace level.
///
/// `ORI_LOG=ori_arc::aims::realize=trace` activates it; bisects which post-walk
/// pass (Phase-7 burden lowering, `coalesce_block_rc`) rewrote a block's RC
/// ops.
fn trace_phase_snapshot(
    phase: &'static str,
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) {
    if !tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
        return;
    }
    let fn_name = interner.lookup(func.name);
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut incs: Vec<u32> = Vec::new();
        let mut decs: Vec<u32> = Vec::new();
        let mut binc: Vec<u32> = Vec::new();
        let mut bdec: Vec<u32> = Vec::new();
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { var, .. } => incs.push(var.raw()),
                ArcInstr::RcDec { var, .. } => decs.push(var.raw()),
                ArcInstr::BurdenInc { var } => binc.push(var.raw()),
                ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var } => bdec.push(var.raw()),
                _ => {}
            }
        }
        if incs.is_empty() && decs.is_empty() && binc.is_empty() && bdec.is_empty() {
            continue;
        }
        tracing::trace!(
            target: "ori_arc::aims::realize",
            phase = phase,
            fn_name = fn_name,
            block = block_idx,
            inc = ?incs,
            dec = ?decs,
            binc = ?binc,
            bdec = ?bdec,
            "post-walk RC snapshot"
        );
    }
}

/// RC emission for a class-ledger-replaced function.
///
/// Every production shape is class-ledger-replaced (the Step-4b fail-loud
/// gate admits nothing else), so the burden ops in the instruction stream
/// ARE the verified plan: this lowers them mechanically to `RcInc`/`RcDec`
/// (`lower_burden_ops_to_rc`) and finalizes emission. A non-replaced
/// function reaching this point is an internal error (`unreachable!`).
pub(super) fn emit_rc_unified(
    func: &mut ArcFunction,
    _state_map: &AimsStateMap,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    _contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &TypeRegistry,
) -> (
    usize,
    Vec<DeathEvent>,
    Vec<AllocEvent>,
    metrics::SynergyMetrics,
) {
    assert_eq!(
        func.var_metadata_state,
        crate::ir::VariableMetadataState::Realized,
        "variable metadata must be fully realized before RC emission"
    );
    assert_eq!(
        func.var_reprs.len(),
        func.var_types.len(),
        "realized var_reprs must be parallel to var_types before RC emission"
    );
    assert_eq!(
        func.var_rc_strategies.len(),
        func.var_types.len(),
        "realized var_rc_strategies must be parallel to var_types before RC emission"
    );

    // The burden ops in the stream ARE the per-class-verified plan. Lower the
    // plan mechanically in Phase 7 and coalesce the resulting RC ops.
    if func.class_ledger_emission {
        lower_burden_ops_to_rc(func, pool, type_registry, &FxHashSet::default());
        trace_phase_snapshot("after_phase_7_burden_lowering", func, interner);
        finalize_rc_emission(func, interner);
        return (
            count_rc_ops(func),
            Vec::new(),
            Vec::new(),
            metrics::SynergyMetrics::default(),
        );
    }

    // The class-ledger burden carrier is the sole RC-emission input to the
    // current compiled-counter adapter. It is not AIMS's sole physical
    // realization. On the normal (burden-ops-enabled) path the Step-4b `assert!`
    // already ICEs before a
    // non-replaced function reaches here. Under `ORI_DISABLE_BURDEN_OPS=1`
    // the Step-4b assert is vacuously satisfied (its condition is
    // `!burden_ops_enabled || replaced`), so every function declines
    // replacement without tripping it — THIS `unreachable!()` is itself the
    // fail-loud gate for that ablation path, not a redundant backstop.
    unreachable!(
        "realize reached a non-class-ledger function `{}` — the class-ledger \
         plan admits only replaced functions",
        interner.lookup(func.name)
    );
}

/// Shared RC-emission tail: Phase 3 coalescing peephole (merge adjacent RC
/// ops per block) followed by RL-2 scope-exit drop-order correction on
/// `Return` blocks ([`order_return_block_scope_exit_decs`]).
///
/// BOTH `emit_rc_unified` exit paths — the class-ledger replacement early
/// return and the default burden-path walk — emit real `RcInc`/`RcDec`
/// instructions subject to the SAME user-`@drop`-observable ordering hazard
/// on `Return` blocks (Spec: Annex E §AIMS RL-2 + RL-DROP); routing both
/// through one finalize step keeps them from drifting out of sync the way a
/// duplicated inline tail would.
fn finalize_rc_emission(func: &mut ArcFunction, interner: &ori_ir::StringInterner) {
    use crate::aims::emit_rc::coalesce_block_rc;

    for block in &mut func.blocks {
        coalesce_block_rc(&mut block.body);
    }
    trace_phase_snapshot("after_phase_3_coalesce", func, interner);

    order_return_block_scope_exit_decs(func);
}

/// The released var of a scope-exit release op (whole-var or field-grain),
/// `None` for every non-release instruction.
fn release_var(instr: &ArcInstr) -> Option<ArcVarId> {
    match instr {
        ArcInstr::RcDec { var, .. }
        | ArcInstr::BurdenDec { var }
        | ArcInstr::RcDecPartial { var, .. }
        | ArcInstr::BurdenDecPartial { var, .. }
        | ArcInstr::RcDecVariant { var }
        | ArcInstr::BurdenDecVariant { var } => Some(*var),
        ArcInstr::RcDecField { base, .. } | ArcInstr::BurdenDecField { base, .. } => Some(*base),
        _ => None,
    }
}

/// RL-2 scope-exit drop ordering on `Return` blocks: sort each Return block's
/// trailing release run into REVERSE DECLARATION ORDER (descending `ArcVarId`),
/// the value-semantics teardown order (a later-declared container drops before
/// the earlier locals its teardown may observe — the two-channel map teardown
/// fires before the caller's own key/value copies release). Releases within
/// one trailing run are a per-path permutation (RC-net neutral); only the
/// user-`@drop`-observable order changes. Spec: Annex E §AIMS RL-2 + RL-DROP.
fn order_return_block_scope_exit_decs(func: &mut ArcFunction) {
    for block_idx in 0..func.blocks.len() {
        if !matches!(
            func.blocks[block_idx].terminator,
            crate::ir::ArcTerminator::Return { .. }
        ) {
            continue;
        }
        let body_len = func.blocks[block_idx].body.len();
        // The maximal trailing run of release ops — whole-var AND field-grain
        // (a partial/field/variant dec walks field payloads whose drop glue
        // may fire transitively, so it is order-bearing and must not truncate
        // the run).
        let mut start = body_len;
        while start > 0 && release_var(&func.blocks[block_idx].body[start - 1]).is_some() {
            start -= 1;
        }
        if body_len - start < 2 {
            continue;
        }
        // One unit per release op; the sort is stable, so same-var release
        // sequences keep their relative order. `func.spans` is indexed
        // `[block_index][instr_index]` in lockstep with `body` (per every
        // other body-reordering pass in this crate — `block_merge::select`,
        // `aims::emit_reuse::dynamic`, `tail_call::rewrite`); split + resort
        // the span tail alongside the instruction tail so a reordered
        // release's provenance stays attached to the reordered instruction
        // instead of silently describing whichever instruction ends up at
        // its old position.
        let tail: Vec<ArcInstr> = func.blocks[block_idx].body.split_off(start);
        let span_tail: Vec<Option<ori_ir::Span>> = func
            .spans
            .get_mut(block_idx)
            .map(|spans| {
                let at = start.min(spans.len());
                spans.split_off(at)
            })
            .unwrap_or_default();
        let mut units: Vec<(usize, ArcInstr, Option<ori_ir::Span>)> = tail
            .into_iter()
            .enumerate()
            .map(|(i, instr)| {
                let Some(var) = release_var(&instr) else {
                    unreachable!("trailing run contains only release ops")
                };
                let span = span_tail.get(i).copied().flatten();
                (var.index(), instr, span)
            })
            .collect();
        units.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, instr, span) in units {
            func.blocks[block_idx].body.push(instr);
            if let Some(spans) = func.spans.get_mut(block_idx) {
                spans.push(span);
            }
        }
    }
}

/// Count RC operations (`RcInc` + `RcDec`) in a function.
fn count_rc_ops(func: &ArcFunction) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .count()
}
