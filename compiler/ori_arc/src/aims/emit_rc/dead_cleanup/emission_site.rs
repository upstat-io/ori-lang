//! BUG-04-111 — Emission-disposition predicate SSOT.
//!
//! The `EmissionSite` enum classifies WHERE in a block a variable's `RcDec`
//! would be emitted. The companion `var_emits_dec_in_block` helper returns
//! `Option<EmissionSite>` for each `(ctx, var, is_unwind_block)` triple —
//! the SSOT predicate the PIN-4 gate sites consult.
//!
//! Variants carry `instr_idx` for body-position emission sites so
//! canonical-rep selection picks the LATEST-emitting class
//! member by program order.
//!
//! BUG-04-111: the gate table per variant + the bypass-safe-region carve-out.

/// Where in a block's emission a variable's `RcDec` would actually fire.
///
/// Pure classification — does NOT include sites mutated stateful by
/// `let_reps_dec_emitted` (the Phase A bypass-safe inline-emit path is
/// handled separately by its own bypass-safe-region interaction guard).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[expect(
    dead_code,
    reason = "BUG-04-111 — DefinedDead and MergeEdgeRouted are ordered by emission_site_order but not yet constructed by var_emits_dec_in_block"
)]
pub(crate) enum EmissionSite {
    /// Source 1 — `dead_cleanup/mod.rs::emit_dead_at_entry_decs` dead-at-entry
    /// block-prepended emission. Fires at block entry (EARLIEST emission
    /// position).
    PhaseAEntry,

    /// Source 2 — `dead_cleanup/mod.rs::emit_dead_block_param_decs`
    /// dead-block-param block-prepended emission. Fires at block entry.
    PhaseABlockParam,

    /// Source 3 — `realize/walk_dec.rs::emit_last_use_decs` body last-use
    /// position. `instr_idx` is the per-block instruction position.
    BodyWalkLastUse { instr_idx: usize },

    /// Source 6 — `walk_dec.rs::emit_defined_dead` at the
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
    /// in PIN-4 suppression of other class members — edge-cleanup gates run
    /// on their own; PIN-4 cannot predict whether the per-edge fire will
    /// actually occur without inspecting successor entry states.
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
/// computed via the SSOT `var_emits_dec_in_block` predicate.
/// this pre-compute REPLACES the inline gate sequence at PIN-4 sites; the
/// canonical-rep selection picks ONE member per class to actually
/// emit, suppressing the others.
pub(crate) type Pin4EmitsByClass = FxHashMap<u32, FxHashSet<(ArcVarId, EmissionSite)>>;

/// Pure SSOT predicate: returns `Some(EmissionSite)` when
/// `var` would emit a `RcDec` somewhere in `ctx.blk`'s realized output;
/// `None` otherwise. Consumed by the `pin4_class_emits_dec_set` pre-compute
/// and by the canonical-rep selection logic at PIN-4 gate sites.
///
/// CRITICAL: this helper is the SSOT that PIN-4 *consumes*. It MUST NOT
/// include any PIN-4 class-member gate in its own logic — doing so creates
/// a circular dependency.
///
/// Gate scope EXCLUDES the bypass-safe-entry path (stateful — mutates
/// `let_reps_dec_emitted` across loop iterations + emits inline; cannot
/// be modeled as a pure predicate carve-out). Returns `None` for
/// bypass-safe-entry vars; the inline emission code in
/// `dead_cleanup/mod.rs::emit_dead_at_entry_decs` handles them separately.
///
/// Classifies `PhaseAEntry`, `PhaseABlockParam`, `BodyWalkLastUse`, and
/// `DeferredParent`. `DefinedDead` and `MergeEdgeRouted` are never returned
/// (those emission paths are not modeled by this predicate).
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
    // DefinedDead and MergeEdgeRouted emission paths are not classified here.
    None
}

/// `BodyWalkLastUse` + `DeferredParent` case: mirrors
/// `walk_dec.rs::emit_last_use_decs` with the same gate ordering. PIN-4 +
/// PIN-5 class-aware suppression are EXCLUDED — they consume this helper
/// (the body-walk PIN-4 `class_alive_after` queries `pin4_class_emits_dec_set`,
/// which itself depends on this helper modeling body-walk emissions).
///
/// `decide` machinery (consuming-primop, ownership-transfer, owned-call-position
/// detection) is NOT replicated here. The simpler approximation is: var emits
/// a dec via body-walk iff its `LastUse` is `Body(instr_idx)`, var is at a
/// non-owned position in that instruction, and the standard rc-managed +
/// liveness gates pass. Classes whose body-walk emission depends on
/// consuming-primop / ownership-transfer detection may have false-positive
/// emissions in `pin4_emits`, leading to over-suppression. Those edge cases
/// land in subsequent step refinements.
fn check_body_walk_last_use(ctx: &BlockCtx<'_>, var: ArcVarId) -> Option<EmissionSite> {
    // is_rc_managed semantics — same as `walk_dec.rs::is_rc_managed`.
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

    // Align prediction with the actual emission gate:
    // `walk_dec.rs::emit_post_instr_decs_unified` returns early on
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

    // has_live_borrowed_children → DeferredParent variant (mirrors
    // `walk_dec.rs::has_live_borrowed_children`).
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

/// `PhaseAEntry` case: mirrors the `emit_dead_at_entry_decs` Source 1 path
/// with the same gate ordering. Bypass-safe-entry is
/// EXCLUDED carve-out (stateful inline path). PIN-4
/// class-member check is also EXCLUDED to avoid circular dependency on
/// this helper (PIN-4 will *consume* this helper).
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
    // Bypass-safe-entry path is stateful (handled inline in
    // `emit_dead_at_entry_decs`).
    if ctx
        .take_move_facts
        .is_bypass_safe_entry_for_var(var, ctx.blk.index())
    {
        return None;
    }
    // In-class non-bypass-safe vars are handled by the take-project's
    // is_ownership_transfer at the Project site.
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
    // Apply-aliased suppression (BUG-04-090).
    if should_suppress_apply_aliased_dec(ctx.state_map, var, is_unwind_block) {
        return None;
    }
    // Return-transfer suppression (BUG-04-090).
    if should_suppress_return_transfer_dec(ctx, var, ctx.blk, is_unwind_block) {
        return None;
    }
    // Strategy-bound: no strategy means no emission.
    rc_strategy(ctx.func, var, ctx.pool).map(|_| EmissionSite::PhaseAEntry)
}

/// `PhaseABlockParam` case: mirrors the `emit_dead_block_param_decs`
/// per-param gates. Gate ordering matches the live emission code. PIN-4
/// class-member check + PIN-6 same-emission check are EXCLUDED — both consume
/// this helper.
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

/// Build the per-class emission map for this block.
///
/// Iterates over `PhaseAEntry` candidates (vars in `entry_states`),
/// `PhaseABlockParam` candidates (block params not in `entry_states`), and
/// body-walk candidates (vars with `use_info`), querying the SSOT
/// `var_emits_dec_in_block` predicate for each. Members that would emit are
/// grouped by SSA alias class id.
///
/// `DefinedDead` and `MergeEdgeRouted` emission paths are not classified by
/// `var_emits_dec_in_block` — class members emitting via those paths are
/// MISSED in this pre-compute.
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

/// Function-level analogue of [`Pin4EmitsByClass`]: every emitting class member
/// tagged with its block index, so PIN-4 suppression can reason about which
/// member's dec covers `var`'s RC slot ACROSS blocks, not just within one.
/// `usize` = the member's block position (`ArcBlockId::index`).
pub(crate) type GlobalPin4Emits = FxHashMap<u32, FxHashSet<(ArcVarId, usize, EmissionSite)>>;

/// Cross-block PIN-4 suppression with post-dominance (BUG-04-123 cluster).
///
/// Returns `None` when `class_id` has no recorded emitters (caller falls
/// through to side-table dedup), `Some(true)` to suppress `var`'s dec, else
/// `Some(false)`. A class member suppresses `var` when it covers `var`'s RC slot
/// on EVERY path from `var`'s block to exit: same-block members dedup by latest
/// emission site; cross-block members must POST-DOMINATE `var`'s block.
///
/// Post-dominance — not raw block order — is load-bearing: a class's emitters
/// are leaf decs (RL-4 Jump-arg exemption transfers ownership at merges), so on
/// a linear chain the later leaf post-dominates the earlier (suppress earlier),
/// but on a branch neither post-dominates the other (both emit, one per path).
/// This preserves the per-path RC-balance invariant (RL-2/RL-4/RL-5): every
/// concrete CFG path nets exactly one dec PER OWNED REFERENCE.
///
/// `lineage_roots` is the per-var retained-lineage map (var → retained-copy
/// root; absent = alloc-reference lineage). A member `m` may suppress `var` ONLY
/// when they share a lineage (`lineage_roots.get(m) == lineage_roots.get(var)`):
/// a distinct retained copy holds its OWN owned reference of the heap value, so
/// its dec must NOT be collapsed against another lineage's. This partitions the
/// class into `1 + N` lineages (N retained + 1 alloc-ref), each deduped to one
/// dec per path → `1 + N` total (04B.2-under-elim.lean `rc_per_path_invariant`).
/// When there are no within-class retained copies the map is empty → every
/// member shares the alloc-ref lineage → identical to per-class dedup (no-op
/// baseline). Within a lineage the existing same-block-latest / cross-block
/// post-dominance dedup is unchanged.
pub(crate) fn class_member_suppresses(
    class_id: u32,
    var: ArcVarId,
    var_block: usize,
    post_doms: &crate::graph::PostDominatorTree,
    global: &GlobalPin4Emits,
    lineage_roots: &FxHashMap<ArcVarId, ArcVarId>,
) -> Option<bool> {
    let members = global.get(&class_id)?;
    let var_lineage = lineage_roots.get(&var);
    let var_site = members
        .iter()
        .find(|&&(m, b, _)| m == var && b == var_block)
        .map(|&(_, _, s)| emission_site_order(s));
    let suppressor = members.iter().find(|&&(m, m_block, m_site)| {
        if m == var && m_block == var_block {
            return false;
        }
        // Lineage-equality gate: a member only dedups `var` when it is the SAME
        // owned reference (same retained lineage, or both alloc-ref). Distinct
        // retained copies each keep their own dec.
        if lineage_roots.get(&m) != var_lineage {
            return false;
        }
        if m_block == var_block {
            match var_site {
                Some(vs) => (emission_site_order(m_site), m.raw()) > (vs, var.raw()),
                None => false,
            }
        } else {
            post_doms.post_dominates(
                crate::aims::emit_rc::block_id(m_block),
                crate::aims::emit_rc::block_id(var_block),
            )
        }
    });
    if tracing::enabled!(target: "ori_arc::aims::realize::class_dec", tracing::Level::TRACE) {
        if let Some(&(m, m_block, _)) = suppressor {
            tracing::trace!(
                target: "ori_arc::aims::realize::class_dec",
                class_id,
                var = var.raw(),
                var_block,
                covered_by = m.raw(),
                cover_block = m_block,
                relation = if m_block == var_block { "same-block-latest" } else { "post-dominator" },
                "PIN-4 cross-block dec SUPPRESSED (covering member fires on every path)"
            );
        } else {
            tracing::trace!(
                target: "ori_arc::aims::realize::class_dec",
                class_id,
                var = var.raw(),
                var_block,
                emitters = members.len(),
                "PIN-4 cross-block dec EMITS (no member post-dominates this block)"
            );
        }
    }
    Some(suppressor.is_some())
}
