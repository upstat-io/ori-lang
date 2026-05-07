//! BUG-04-111 §05 Step 1 — Emission-disposition predicate SSOT.
//!
//! The `EmissionSite` enum classifies WHERE in a block a variable's `RcDec`
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
    reason = "BUG-04-111 §05 Step 1 incremental landing — BodyWalkLastUse/DefinedDead/DeferredParent/MergeEdgeRouted variants land in subsequent §05 step commits when var_emits_dec_in_block grows their gate logic"
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

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::lattice::Cardinality;
use crate::ir::ArcVarId;

use super::super::helpers::{is_live_at_exit, is_owned_at_entry, BlockCtx, LastUse};
use super::super::{
    is_consuming_primop, is_ownership_transfer, rc_strategy, should_suppress_apply_aliased_dec,
    should_suppress_return_transfer_dec,
};

/// Canonical map from SSA alias class id → set of (var, `EmissionSite`) pairs
/// representing every class member that would emit a dec in this block,
/// computed via the SSOT `var_emits_dec_in_block` predicate. Per §05 Step 2,
/// this pre-compute REPLACES the inline gate sequence at PIN-4 sites; the
/// canonical-rep selection (§05 Step 3) picks ONE member per class to actually
/// emit, suppressing the others.
pub(crate) type Pin4EmitsByClass = FxHashMap<u32, FxHashSet<(ArcVarId, EmissionSite)>>;

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
/// Currently implements `PhaseAEntry` and `PhaseABlockParam` variants.
/// `BodyWalkLastUse`, `DefinedDead`, `DeferredParent`, `MergeEdgeRouted`
/// land in subsequent §05 step commits.
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
    // Then BodyWalkLastUse / DeferredParent (body-walk last-use emissions).
    if let Some(site) = check_body_walk_last_use(ctx, var) {
        return Some(site);
    }
    // DefinedDead and MergeEdgeRouted variants land in subsequent §05 step commits.
    None
}

/// `BodyWalkLastUse` + `DeferredParent` case: mirrors `walk_dec.rs::emit_last_use_decs`
/// (lines 180-246), with the same gate ordering. PIN-4 + PIN-5 class-aware
/// suppression are EXCLUDED — they consume this helper. Per §05 Step 4.5,
/// the body-walk PIN-4 (`class_alive_after`) is replaced with a query against
/// `pin4_class_emits_dec_set` (which itself depends on this helper modeling
/// body-walk emissions).
///
/// `decide()` machinery (consuming-primop, ownership-transfer, owned-call-position
/// detection) is NOT replicated here. The simpler approximation is: var emits
/// a dec via body-walk iff its `LastUse` is `Body(instr_idx)`, var is at a
/// non-owned position in that instruction, and the standard rc-managed +
/// liveness gates pass. Classes whose body-walk emission depends on
/// consuming-primop / ownership-transfer detection may have false-positive
/// emissions in `pin4_emits`, leading to over-suppression. Those edge cases
/// land in subsequent §05 step refinements.
fn check_body_walk_last_use(ctx: &BlockCtx<'_>, var: ArcVarId) -> Option<EmissionSite> {
    // is_rc_managed semantics — same as is_owned_at_entry per walk_dec.rs:402-411.
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
    if ctx.iter_element_defs.contains(&var) {
        return None;
    }

    let &(_total, last_use) = ctx.use_info.get(&var)?;
    let instr_idx = match last_use {
        LastUse::Body(idx) => idx,
        LastUse::Terminator => return None,
    };

    if is_live_at_exit(ctx.state_map, ctx.blk, var) {
        return None;
    }

    // Validate: var must be at a non-owned position in the instr at instr_idx.
    let block = &ctx.func.blocks[ctx.blk.index()];
    let instr = block.body.get(instr_idx)?;
    let var_position = instr.used_vars().iter().position(|&v| v == var)?;
    if instr.is_owned_position(var_position) {
        return None;
    }

    // BUG-04-111 §05 Step 4.5b: align prediction with actual emission gate.
    // `emit_post_instr_decs_unified` (`walk_dec.rs:91-93`) returns early on
    // `is_consuming_primop` or `is_ownership_transfer` instructions, skipping
    // BOTH `emit_last_use_decs` (BodyWalkLastUse) AND deferral (DeferredParent
    // — `apply_last_use_decision`'s `Defer` branch is reached only via
    // `emit_last_use_decs`). Without this gate, `pin4_class_emits_dec_set`
    // records false-positive entries for vars whose last use is a transparent
    // Let alias / Construct arg / take-Project source — the var is predicted
    // as canonical-rep, suppressing the OTHER class member that actually
    // emits. Per AIMS §3 RL-2 + AIMS Invariant #5 (Unified Model): the
    // SSOT predicate consults the same gates the emitter consults; no
    // parallel paths.
    if is_consuming_primop(instr, ctx.func) || is_ownership_transfer(instr, ctx.func, ctx.pool) {
        return None;
    }

    // has_live_borrowed_children → DeferredParent variant (mirrors walk_dec.rs:414-423).
    if let Some(&child_last) = ctx.child_effective_last_use.get(&var) {
        let has_live_children = match child_last {
            LastUse::Body(c) => c > instr_idx,
            LastUse::Terminator => true,
        };
        if has_live_children {
            return Some(EmissionSite::DeferredParent { instr_idx });
        }
    }

    Some(EmissionSite::BodyWalkLastUse { instr_idx })
}

/// `PhaseAEntry` case: mirrors `emit_dead_at_entry_decs` Source 1 path
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

/// `PhaseABlockParam` case: mirrors `emit_dead_block_param_decs` (mod.rs:265-426)
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

/// §05 Step 2: build the per-class emission map for this block.
///
/// Iterates over all `PhaseAEntry` candidates (vars in `entry_states`) and
/// `PhaseABlockParam` candidates (block params not in `entry_states`),
/// querying the SSOT `var_emits_dec_in_block` predicate for each. Members
/// that would emit are grouped by SSA alias class id.
///
/// Body-walk variants (`BodyWalkLastUse`, `DefinedDead`, `DeferredParent`) and
/// `MergeEdgeRouted` are not yet covered by `var_emits_dec_in_block` per §05
/// Step 1's incremental landing — class members emitting via those paths
/// will be MISSED in this pre-compute. For BUG-04-111's (B-1)/(B-3) RED
/// tests, the relevant emissions are PhaseAEntry/PhaseABlockParam so the
/// pre-compute is sufficient to drive the canonical-rep selection that
/// turns the leak tests GREEN.
pub(crate) fn pin4_class_emits_dec_set(
    ctx: &BlockCtx<'_>,
    is_unwind_block: bool,
) -> Pin4EmitsByClass {
    let mut result: Pin4EmitsByClass = FxHashMap::default();
    let mut record = |var: ArcVarId| {
        if let Some(site) = var_emits_dec_in_block(ctx, var, is_unwind_block) {
            if let Some(class_id) = ctx.state_map.ssa_alias_class_of(var) {
                result.entry(class_id).or_default().insert((var, site));
            }
        }
    };

    // `PhaseAEntry` candidates: vars in entry_states.
    if let Some(entry_states) = ctx.state_map.block_entry_states(ctx.blk) {
        for &var in entry_states.keys() {
            record(var);
        }
    }

    // PhaseABlockParam candidates: block params (var_emits_dec_in_block
    // internally checks "is in entry_states" and skips if so to avoid
    // double-counting with PhaseAEntry).
    let block = &ctx.func.blocks[ctx.blk.index()];
    for &(var, _) in &block.params {
        record(var);
    }

    // BodyWalkLastUse / DeferredParent candidates: any var with use_info
    // (covers all body-walk-emission-eligible vars). Includes vars defined
    // in this block (Construct/Apply results) AND vars from entry_states
    // that have body uses (var_emits_dec_in_block internally selects the
    // first matching variant).
    for &var in ctx.use_info.keys() {
        record(var);
    }

    result
}

/// §05 Step 3: select the canonical rep for `class_id` from its emitting
/// members per gemini Round 2 F1's LATEST-emission-site rule.
///
/// `PhaseAEntry`/`PhaseABlockParam` fire at block entry (EARLIEST). `BodyWalk`
/// variants fire at `instr_idx` (later). `MergeEdgeRouted` fires post-exit
/// (LATEST). Picking the latest preserves correct RC semantics — earlier
/// class member uses are read-only borrows; the actual decrement fires at
/// the latest emission point.
pub(crate) fn canonical_rep_for(class_id: u32, pin4_emits: &Pin4EmitsByClass) -> Option<ArcVarId> {
    let members = pin4_emits.get(&class_id)?;
    members
        .iter()
        .max_by_key(|&&(var, site)| (emission_site_order(site), var.raw()))
        .map(|&(var, _)| var)
}

/// Total order over `EmissionSite` for canonical-rep selection.
/// Higher = LATER in program order = preferred for canonical-rep.
fn emission_site_order(site: EmissionSite) -> usize {
    match site {
        // Block-entry positions tie at 0; `canonical_rep_for` breaks ties via var id.
        EmissionSite::PhaseAEntry | EmissionSite::PhaseABlockParam => 0,
        // Body positions ordered by `instr_idx` (offset by 1 to keep above entry).
        EmissionSite::BodyWalkLastUse { instr_idx }
        | EmissionSite::DefinedDead { instr_idx }
        | EmissionSite::DeferredParent { instr_idx } => 1 + instr_idx,
        // Merge-edge fires at block boundaries — strictly latest.
        EmissionSite::MergeEdgeRouted => usize::MAX,
    }
}
