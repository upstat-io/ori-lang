//! BUG-04-111 §05 Step 1 — Emission-disposition predicate SSOT.
//!
//! The `EmissionSite` enum classifies WHERE in a block a variable's RcDec
//! would be emitted. The companion `var_emits_dec_in_block` helper (added
//! incrementally in subsequent §05 steps) returns `Option<EmissionSite>`
//! for each `(ctx, var, is_unwind_block)` triple — the SSOT predicate that
//! all four PIN-4 gate sites consult.
//!
//! Variants carry `instr_idx` for body-position emission sites so
//! canonical-rep selection (§05 Step 3) picks the LATEST-emitting class
//! member by program order (gemini Round 2 F1).
//!
//! See `bug-tracker/plans/BUG-04-111/section-05-implementation.md` Step 1
//! for the gate table per variant + the bypass-safe-region carve-out.

/// Where in a block's emission a variable's `RcDec` would actually fire.
///
/// Pure classification — does NOT include sites mutated stateful by
/// `let_reps_dec_emitted` (the Phase A bypass-safe inline-emit path is
/// handled separately in §05 Step 1's Bypass-safe-region interaction
/// guard at lines 148-156 of section-05-implementation.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(
    dead_code,
    reason = "BUG-04-111 §05 Step 1 ground-laying — variants consumed by `var_emits_dec_in_block` + canonical_rep_for in §05 Step 1+3 (incremental landing)"
)]
pub(crate) enum EmissionSite {
    /// Source 1 in `dead_cleanup/mod.rs:80-241` — dead-at-entry block-prepended
    /// emission. Fires at block entry (EARLIEST emission position).
    PhaseAEntry,

    /// Source 2 in `dead_cleanup/mod.rs:265-426` — dead-block-param
    /// block-prepended emission. Fires at block entry.
    PhaseABlockParam,

    /// Source 3 — `realize/walk_dec.rs::emit_last_use_decs` body last-use
    /// position. `instr_idx` is the per-block instruction position.
    BodyWalkLastUse { instr_idx: usize },

    /// Source 6 (codex F2) — `walk_dec.rs::emit_defined_dead` at the
    /// defining instruction position.
    DefinedDead { instr_idx: usize },

    /// Source 5 — seeded into `walk_body_unified` via
    /// `child_effective_last_use`. `instr_idx` is the deferred-parent
    /// emission point (where the child's last use dies).
    DeferredParent { instr_idx: usize },

    /// Source 4a — `merge_edge_decs` for `pred_count > 1` paths where the
    /// variable is NOT defined in all predecessors. PIN-4 treats as
    /// out-of-intra-block-order: emission deferred to per-edge cleanup;
    /// classes whose ONLY coverage is `MergeEdgeRouted` DO NOT participate
    /// in PIN-4 suppression of other class members (per codex F1 — edge-
    /// cleanup gates run on their own; PIN-4 cannot predict whether the
    /// per-edge fire will actually occur without inspecting successor
    /// entry states).
    MergeEdgeRouted,
}

use crate::aims::lattice::Cardinality;
use crate::ir::ArcVarId;

use super::super::helpers::{is_live_at_exit, is_owned_at_entry, BlockCtx};
use super::super::{
    rc_strategy, should_suppress_apply_aliased_dec, should_suppress_return_transfer_dec,
};

/// Pure SSOT predicate per §05 Step 1: returns `Some(EmissionSite)` when
/// `var` would emit a `RcDec` somewhere in `ctx.blk`'s realized output;
/// `None` otherwise. Consumed by the (forthcoming, §05 Step 2)
/// `pin4_class_emits_dec_set` pre-compute and by the canonical-rep selection
/// logic that subsequent §05 Steps 3-5 wire into PIN-4 gate sites.
///
/// CRITICAL: this helper is the SSOT that PIN-4 *consumes*. It MUST NOT
/// include any PIN-4 class-member gate in its own logic — doing so creates
/// circular dependency per §05 Step 5 refactor analysis (lines 215+ of
/// section-05-implementation.md).
///
/// Gate scope EXCLUDES the bypass-safe-entry path (stateful — mutates
/// `let_reps_dec_emitted` across loop iterations + emits inline; cannot
/// be modeled as a pure predicate per §05 Step 1 carve-out at lines 75-78
/// of section-05-implementation.md). Returns `None` for bypass-safe-entry
/// vars; the inline emission code at `dead_cleanup/mod.rs:99-164` handles
/// them separately.
///
/// Currently implements ONLY the `PhaseAEntry` variant. `PhaseABlockParam`,
/// `BodyWalkLastUse`, `DefinedDead`, `DeferredParent`, `MergeEdgeRouted`
/// land in subsequent §05 step commits.
#[expect(
    dead_code,
    reason = "BUG-04-111 §05 Step 1 ground-laying — consumed by pin4_class_emits_dec_set + canonical_rep_for in §05 Step 2+3 (incremental landing)"
)]
pub(crate) fn var_emits_dec_in_block(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    is_unwind_block: bool,
) -> Option<EmissionSite> {
    // Check PhaseAEntry first (var in entry_states + Source 1 gates).
    if let Some(site) = check_phase_a_entry(ctx, var, is_unwind_block) {
        return Some(site);
    }
    // Then PhaseABlockParam (var is a block param NOT in entry_states + Source 2 gates).
    if let Some(site) = check_phase_a_block_param(ctx, var, is_unwind_block) {
        return Some(site);
    }
    // Body-position variants (BodyWalkLastUse, DefinedDead, DeferredParent)
    // and MergeEdgeRouted land in subsequent §05 step commits.
    None
}

/// PhaseAEntry case: mirrors `emit_dead_at_entry_decs` Source 1 path
/// (mod.rs:80-241), with the same gate ordering. Bypass-safe-entry is
/// EXCLUDED per the §05 Step 1 carve-out (stateful inline path). PIN-4
/// class-member check is also EXCLUDED to avoid circular dependency on
/// this helper (PIN-4 will *consume* this helper per §05 Step 5).
fn check_phase_a_entry(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    is_unwind_block: bool,
) -> Option<EmissionSite> {
    let entry_states = ctx.state_map.block_entry_states(ctx.blk)?;
    let &state = entry_states.get(&var)?;

    if state.is_scalar() || state.cardinality == Cardinality::Absent {
        return None;
    }
    if !is_owned_at_entry(
        ctx.state_map,
        ctx.blk,
        var,
        ctx.defined_in_block,
        ctx.borrowed_defs,
        ctx.all_borrowed_defs,
    ) {
        return None;
    }
    // Bypass-safe-entry path is stateful (handled inline at mod.rs:99-164).
    if ctx
        .take_move_facts
        .is_bypass_safe_entry_for_var(var, ctx.blk.index())
    {
        return None;
    }
    // In-class non-bypass-safe vars are handled by the take-project's
    // is_ownership_transfer at the Project site (mod.rs:165-171).
    if ctx.take_move_facts.is_in_class(var) {
        return None;
    }
    // Used in body OR live at exit means a different code path emits the dec.
    if ctx.use_info.contains_key(&var) || is_live_at_exit(ctx.state_map, ctx.blk, var) {
        return None;
    }
    // Merge-block route check: when not all predecessors define this var,
    // emission is routed to per-edge cleanup (MergeEdgeRouted variant).
    let predecessors = crate::graph::compute_predecessors(ctx.func);
    let pred_count = predecessors.get(ctx.blk.index()).map_or(0, Vec::len);
    let is_block_param = ctx.func.blocks[ctx.blk.index()]
        .params
        .iter()
        .any(|&(p, _)| p == var);
    if pred_count > 1 && !is_block_param && !ctx.defined_in_block.contains(&var) {
        let all_preds_define_it = predecessors[ctx.blk.index()]
            .iter()
            .all(|&pred_idx| ctx.func.blocks[pred_idx].defines_var(var));
        if !all_preds_define_it {
            // Routed to merge_edge_decs — not PhaseAEntry.
            return None;
        }
    }
    // Deferred-parent variables emit later (DeferredParent variant).
    if ctx.child_effective_last_use.contains_key(&var) {
        return None;
    }
    // Apply-aliased suppression (BUG-04-090 §05 Hypothesis D #4).
    if should_suppress_apply_aliased_dec(ctx.state_map, var, is_unwind_block) {
        return None;
    }
    // Return-transfer suppression (BUG-04-090 §05 Step 8).
    if should_suppress_return_transfer_dec(ctx, var, ctx.blk, is_unwind_block) {
        return None;
    }
    // Strategy-bound: no strategy means no emission.
    rc_strategy(ctx.func, var, ctx.pool).map(|_| EmissionSite::PhaseAEntry)
}

/// PhaseABlockParam case: mirrors `emit_dead_block_param_decs` (mod.rs:265-426)
/// per-param gates. Gate ordering matches the live emission code. PIN-4
/// class-member check + PIN-6 same-emission check are EXCLUDED — both consume
/// this helper per §05 Steps 4 + 5.
fn check_phase_a_block_param(
    ctx: &BlockCtx<'_>,
    var: ArcVarId,
    is_unwind_block: bool,
) -> Option<EmissionSite> {
    let block = &ctx.func.blocks[ctx.blk.index()];
    // Must be a block param.
    if !block.params.iter().any(|&(p, _)| p == var) {
        return None;
    }
    // Skip if already handled by Source 1 (PhaseAEntry).
    let entry_states = ctx.state_map.block_entry_states(ctx.blk);
    if entry_states.is_some_and(|es| es.contains_key(&var)) {
        return None;
    }
    if ctx.state_map.is_excluded(var) {
        return None;
    }
    if ctx.use_info.contains_key(&var) {
        return None;
    }
    if is_live_at_exit(ctx.state_map, ctx.blk, var) {
        return None;
    }
    if ctx.iter_element_defs.contains(&var) {
        return None;
    }
    if ctx.take_move_facts.is_in_class(var) {
        return None;
    }
    if should_suppress_apply_aliased_dec(ctx.state_map, var, is_unwind_block) {
        return None;
    }
    rc_strategy(ctx.func, var, ctx.pool).map(|_| EmissionSite::PhaseABlockParam)
}
