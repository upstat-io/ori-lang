//! Unified RC emission: per-block walk with inline death/alloc event collection.
//!
//! Extracted from `realize/mod.rs` to stay under the 500-line file limit.
//! Called by [`super::realize_rc_reuse()`] as Phase 1 sub-step B.

use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::emit_rc::DeferredDec;
use crate::aims::emit_reuse::{AllocEvent, DeathEvent};
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

use super::metrics;
use super::walk;

/// Per-phase RC-op snapshot for post-walk pass debugging.
///
/// Emits one `tracing::trace!` event per block summarising every
/// `RcInc`/`RcDec` instruction by `ArcVarId`. Gated behind
/// `tracing::enabled!` so the iteration is skipped entirely when
/// `ori_arc::aims::realize` is not at trace level — zero overhead in
/// normal debug runs.
///
/// Activate with `ORI_LOG=ori_arc::aims::realize=trace`. Used to
/// bisect which post-walk pass (`emit_dead_invoke_dsts`,
/// `emit_edge_cleanup`, `emit_project_escape_incs`, `coalesce_block_rc`)
/// modifies a specific block's RC ops without inline `tracing::debug!`
/// insertions. Captured during TPR-07-017 debugging — see
/// `.claude/rules/arc.md` § Debugging.
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
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { var, .. } => incs.push(var.raw()),
                ArcInstr::RcDec { var, .. } => decs.push(var.raw()),
                _ => {}
            }
        }
        if incs.is_empty() && decs.is_empty() {
            continue;
        }
        tracing::trace!(
            target: "ori_arc::aims::realize",
            phase = phase,
            fn_name = fn_name,
            block = block_idx,
            inc = ?incs,
            dec = ?decs,
            "post-walk RC snapshot"
        );
    }
}

/// Unified RC emission: per-block walk with inline death/alloc event collection.
///
/// Replaces `emit_rc_ops()` with a forward walk that routes all decisions
/// through `decide()` and collects reuse events inline, eliminating the
/// separate `collect_death_events()` / `collect_alloc_events()` scans.
///
/// # Phases
///
/// 1. Per-block: dead-at-entry → unified body walk → terminator RC → deferred
/// 2. Dead Invoke cleanup (orphaned Invoke result variables)
/// 3. Inter-block edge cleanup (with deferred parent decs)
/// 4. RC coalescing peephole per block
pub(super) fn emit_rc_unified(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
) -> (
    usize,
    Vec<DeathEvent>,
    Vec<AllocEvent>,
    metrics::SynergyMetrics,
) {
    use crate::aims::emit_rc::{
        coalesce_block_rc, collect_all_borrowed_defs, collect_inline_enum_projected_defs,
        collect_iter_element_defs, collect_project_borrowed_defs, compute_function_project_sources,
        emit_dead_invoke_dsts, emit_edge_cleanup, DeferredDec,
    };

    debug_assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    let all_borrowed_defs = collect_all_borrowed_defs(func, pool);
    let project_borrowed_defs = collect_project_borrowed_defs(func, pool);
    let iter_element_defs = collect_iter_element_defs(func, interner);
    let inline_enum_projected_defs = collect_inline_enum_projected_defs(func, pool);
    let func_project_sources = compute_function_project_sources(func);
    // TPR-07-016: path-sensitive take-project must-move analysis.
    // Precomputed once per function; consumed by dead/edge cleanup
    // to decide whether to suppress the source enum's scope-exit
    // `RcDec` on any given block's entry.
    let take_move_facts = crate::aims::emit_rc::take_project::analyze(func, pool);
    let iter_fn_name = interner.intern("iter");
    let predecessors = crate::graph::compute_predecessors(func);
    let mut all_death_events = Vec::new();
    let mut all_alloc_events = Vec::new();
    // Deferred decs routed to edge cleanup. Each entry:
    // - `None` target: emit on ALL outgoing edges (Phase B deferred parents)
    // - `Some(succ)` target: emit only on edge to `succ` (merge-edge decs)
    let mut block_deferred: FxHashMap<usize, Vec<DeferredDec>> = FxHashMap::default();
    let mut synergy = metrics::SynergyMetrics::default();

    // Phase 1: per-block RC emission via unified forward walk.
    for block_idx in 0..func.blocks.len() {
        let (death_events, alloc_events, walk_metrics) = emit_block_rc(
            func,
            block_idx,
            state_map,
            pool,
            &all_borrowed_defs,
            &project_borrowed_defs,
            &iter_element_defs,
            &inline_enum_projected_defs,
            &func_project_sources,
            &take_move_facts,
            iter_fn_name,
            &predecessors,
            &mut block_deferred,
        );
        synergy.merge(&walk_metrics);
        all_death_events.extend(death_events);
        all_alloc_events.extend(alloc_events);
    }
    trace_phase_snapshot("after_phase_1_walk", func, interner);

    // Phase 1.5: dead Invoke result cleanup.
    emit_dead_invoke_dsts(func, state_map, pool, &all_borrowed_defs);
    trace_phase_snapshot("after_phase_1_5_dead_invoke", func, interner);

    // Phase 2: inter-block edge cleanup (with deferred parent decs).
    emit_edge_cleanup(
        func,
        state_map,
        pool,
        &all_borrowed_defs,
        &take_move_facts,
        &block_deferred,
    );
    trace_phase_snapshot("after_phase_2_edge_cleanup", func, interner);

    // Phase 2.1: insert RcInc for projected children that escape via
    // terminator args, where the parent aggregate was edge-dec'd by
    // Phase 2 above. Edge cleanup may have created trampoline blocks with
    // AggFields dec — these dec ALL fields including projected ones still
    // live in the successor. The RcInc compensates.
    emit_project_escape_incs(
        func,
        state_map,
        pool,
        &func_project_sources,
        &all_borrowed_defs,
    );
    trace_phase_snapshot("after_phase_2_1_escape_incs", func, interner);

    // Phase 3: RC coalescing peephole — merge adjacent RC ops per block.
    for block in &mut func.blocks {
        coalesce_block_rc(&mut block.body);
    }
    trace_phase_snapshot("after_phase_3_coalesce", func, interner);

    let rc_count = count_rc_ops(func);
    (rc_count, all_death_events, all_alloc_events, synergy)
}

/// Emit RC operations for a single block via the unified forward walk.
///
/// Returns `(death_events, alloc_events, walk_metrics)`.
#[expect(
    clippy::too_many_arguments,
    reason = "block-level RC emission needs full context"
)]
fn emit_block_rc(
    func: &mut ArcFunction,
    block_idx: usize,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    project_borrowed_defs: &FxHashSet<ArcVarId>,
    iter_element_defs: &FxHashSet<ArcVarId>,
    inline_enum_projected_defs: &FxHashSet<ArcVarId>,
    func_project_sources: &FxHashMap<ArcVarId, ArcVarId>,
    take_move_facts: &crate::aims::emit_rc::take_project::TakeMoveFacts,
    iter_fn_name: ori_ir::Name,
    predecessors: &[Vec<usize>],
    block_deferred: &mut FxHashMap<usize, Vec<DeferredDec>>,
) -> (Vec<DeathEvent>, Vec<AllocEvent>, metrics::SynergyMetrics) {
    use crate::aims::emit_rc::{
        block_id, collect_borrowed_defs, collect_defined_vars, compute_child_effective_last_use,
        emit_dead_at_entry_decs, emit_terminator_rc, precompute_block_uses, BlockCtx,
    };

    let blk = block_id(block_idx);
    let use_info = precompute_block_uses(&func.blocks[block_idx]);
    let defined_in_block = collect_defined_vars(&func.blocks[block_idx]);
    let borrowed_defs = collect_borrowed_defs(&func.blocks[block_idx], func, pool);
    let child_elu =
        compute_child_effective_last_use(&func.blocks[block_idx], &use_info, func_project_sources);

    let old_body = std::mem::take(&mut func.blocks[block_idx].body);
    let mut new_body: Vec<ArcInstr> = Vec::with_capacity(old_body.len() * 2);

    let ctx = BlockCtx {
        func,
        blk,
        state_map,
        defined_in_block: &defined_in_block,
        borrowed_defs: &borrowed_defs,
        all_borrowed_defs,
        project_borrowed_defs,
        iter_element_defs,
        inline_enum_projected_defs,
        use_info: &use_info,
        pool,
        child_effective_last_use: &child_elu,
        take_move_facts,
    };

    let (deferred_parents, merge_edge_decs) = emit_dead_at_entry_decs(&ctx, &mut new_body);

    let walk::BodyWalkResult {
        uses_so_far,
        terminator_deferred,
        death_events,
        alloc_events,
        walk_metrics,
    } = walk::walk_body_unified(
        &ctx,
        &old_body,
        &mut new_body,
        iter_fn_name,
        deferred_parents,
    );

    emit_terminator_rc(&ctx, block_idx, uses_so_far, &mut new_body);

    let edge_deferred = match &func.blocks[block_idx].terminator {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {
            for &(var, strategy) in &terminator_deferred {
                new_body.push(ArcInstr::RcDec { var, strategy });
            }
            Vec::new()
        }
        _ => terminator_deferred,
    };

    func.blocks[block_idx].body = new_body;
    if !edge_deferred.is_empty() {
        let tagged: Vec<_> = edge_deferred
            .into_iter()
            .map(|(var, strat)| (None, var, strat))
            .collect();
        block_deferred.insert(block_idx, tagged);
    }
    route_merge_edge_decs(
        func,
        block_idx,
        &merge_edge_decs,
        predecessors,
        block_deferred,
    );

    (death_events, alloc_events, walk_metrics)
}

/// Route merge-edge decs to per-predecessor edge cleanup.
///
/// Each predecessor that DEFINES the variable gets the dec on its edge
/// to the merge block ONLY (not all outgoing edges). This preserves
/// successor identity so edge cleanup doesn't fire on unrelated edges.
///
/// Take-project alias-class members never reach this routing: the
/// `dead_cleanup.rs` `is_in_class` checks skip them entirely (their
/// natural scope-exit drops in non-projecting predecessors handle the
/// cleanup, and `is_ownership_transfer` at the take-project `Project`
/// site suppresses the source's last-use drop).
fn route_merge_edge_decs(
    func: &ArcFunction,
    block_idx: usize,
    merge_edge_decs: &[(ArcVarId, RcStrategy)],
    predecessors: &[Vec<usize>],
    block_deferred: &mut FxHashMap<usize, Vec<DeferredDec>>,
) {
    if merge_edge_decs.is_empty() {
        return;
    }
    let preds = &predecessors[block_idx];
    for &(var, strategy) in merge_edge_decs {
        for &pred_idx in preds {
            if func.blocks[pred_idx].defines_var(var) {
                block_deferred
                    .entry(pred_idx)
                    .or_default()
                    .push((Some(block_idx), var, strategy));
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

/// Emit `RcInc` for projected children of deferred parents that escape the
/// block via terminator arguments (Jump/Branch/Switch).
///
/// When a parent aggregate's `RcDec [AggFields]` is deferred to edge cleanup
/// (emitted AFTER the terminator), it decrements ALL struct fields. If a
/// projected child is passed to a successor via the terminator args, the child
/// must survive the parent's dec. Without `RcInc`, the field's refcount drops
/// Insert `RcInc` for projected children that escape via terminator args
/// when the parent aggregate will be cleaned up by edge cleanup.
///
/// Edge cleanup emits `RcDec [AggFields]` for parent aggregates that are
/// live at block exit but dead in a successor. This decs ALL struct fields,
/// including projected ones that were passed to the successor via Jump args.
/// Without a compensating `RcInc`, the projected field is freed while the
/// successor still holds a reference → use-after-free.
///
/// This function scans each block: if a parent aggregate is live at exit
/// and has a projected child that escapes via the terminator, it inserts
/// `RcInc` for the projected child at the end of the block body (before
/// the terminator).
fn emit_project_escape_incs(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    func_project_sources: &FxHashMap<ArcVarId, ArcVarId>,
    _all_borrowed_defs: &FxHashSet<ArcVarId>,
) {
    use crate::aims::emit_rc::{block_id, rc_strategy};

    let mut incs_to_insert: Vec<(usize, Vec<ArcInstr>)> = Vec::new();
    let mut succ_decs: Vec<(usize, ArcVarId, RcStrategy)> = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = block_id(block_idx);
        let ArcTerminator::Jump { args, target, .. } = &block.terminator else {
            continue;
        };
        if args.is_empty() {
            continue;
        }
        let target_idx = target.index();

        let doomed_parents =
            find_edge_decced_project_parents(block, blk, args, state_map, func_project_sources);
        if doomed_parents.is_empty() {
            continue;
        }

        // For each arg tracing to a doomed parent: RcInc on the arg itself
        // (the specific projected child that is escaping), plus a successor
        // RcDec on the corresponding merge-block parameter.
        //
        // Previously this used a single `parent -> project_dst` map which
        // collapsed multiple escaping children from the same parent into one
        // slot. Now each arg gets its own RcInc keyed to its own identity.
        let var_to_parent = build_var_to_parent(block, args, func_project_sources);
        let mut block_incs = Vec::new();
        for (arg_pos, &arg) in args.iter().enumerate() {
            let Some(&parent) = var_to_parent.get(&arg) else {
                continue;
            };
            if !doomed_parents.contains(&parent) {
                continue;
            }
            let Some(strategy) = rc_strategy(func, arg, pool) else {
                continue;
            };

            block_incs.push(ArcInstr::RcInc {
                var: arg,
                count: 1,
                strategy,
            });

            let final_target = follow_jump_chain(func, target_idx);
            if let Some(&(param_var, _)) = func
                .blocks
                .get(final_target)
                .and_then(|b| b.params.get(arg_pos))
            {
                let ps = rc_strategy(func, param_var, pool).unwrap_or(strategy);
                succ_decs.push((final_target, param_var, ps));
            }
        }
        if !block_incs.is_empty() {
            incs_to_insert.push((block_idx, block_incs));
        }
    }

    for (block_idx, incs) in incs_to_insert {
        func.blocks[block_idx].body.extend(incs);
    }
    let mut seen: FxHashSet<(usize, u32)> = FxHashSet::default();
    for (succ_idx, var, strategy) in succ_decs {
        if succ_idx < func.blocks.len() && seen.insert((succ_idx, var.raw())) {
            func.blocks[succ_idx]
                .body
                .push(ArcInstr::RcDec { var, strategy });
        }
    }
}

/// Build a mapping from variables to their Project source parent.
///
/// Combines block-local Projects, function-level Project sources for
/// terminator args, and Let aliases in the block body.
fn build_var_to_parent(
    block: &crate::ir::ArcBlock,
    args: &[ArcVarId],
    func_project_sources: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut map: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for instr in &block.body {
        if let ArcInstr::Project { dst, value, .. } = instr {
            map.insert(*dst, *value);
        }
    }
    for &arg in args {
        if let Some(&parent) = func_project_sources.get(&arg) {
            map.entry(arg).or_insert(parent);
        }
    }
    for instr in &block.body {
        if let ArcInstr::Let {
            dst,
            value: crate::ir::ArcValue::Var(src),
            ..
        } = instr
        {
            let parent = map
                .get(src)
                .copied()
                .or_else(|| func_project_sources.get(src).copied());
            if let Some(p) = parent {
                map.insert(*dst, p);
            }
        }
    }
    map
}

/// Find parent aggregates that will be edge-dec'd and have projected children
/// escaping via terminator args.
///
/// Returns the set of doomed parent variable IDs. Each arg that traces to a
/// doomed parent needs its own compensating `RcInc` — the caller resolves
/// per-arg identity, not this function.
fn find_edge_decced_project_parents(
    block: &crate::ir::ArcBlock,
    blk: ArcBlockId,
    args: &[ArcVarId],
    state_map: &AimsStateMap,
    func_project_sources: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    use crate::aims::emit_rc::is_live_at_exit;
    use crate::aims::lattice::Cardinality;
    use crate::graph::successor_block_ids;

    let var_to_parent = build_var_to_parent(block, args, func_project_sources);
    let successors = successor_block_ids(&block.terminator);

    let mut doomed: FxHashSet<ArcVarId> = FxHashSet::default();
    for &parent in var_to_parent.values() {
        if !is_live_at_exit(state_map, blk, parent) {
            continue;
        }
        for succ_id in &successors {
            if state_map
                .var_state_at_block_entry(*succ_id, parent)
                .cardinality
                == Cardinality::Absent
            {
                doomed.insert(parent);
                break;
            }
        }
    }

    doomed
}

/// Follow a chain of Jump terminators through trampoline blocks to find
/// the final merge block.
///
/// Edge cleanup creates trampoline blocks between predecessors and merge
/// blocks. Trampolines have params (they receive Jump args) but are
/// single-use intermediaries. This function skips trampolines (blocks
/// whose body is only `RcDec` instructions) and returns the first block
/// with non-`RcDec` body content or the end of the chain.
fn follow_jump_chain(func: &ArcFunction, mut idx: usize) -> usize {
    let mut visited = 0;
    while visited < func.blocks.len() {
        let Some(block) = func.blocks.get(idx) else {
            break;
        };
        // A trampoline block has body containing only RcDec instructions.
        let is_trampoline = !block.body.is_empty()
            && block
                .body
                .iter()
                .all(|i| matches!(i, ArcInstr::RcDec { .. }));
        if !is_trampoline {
            return idx;
        }
        if let ArcTerminator::Jump { target, .. } = &block.terminator {
            idx = target.index();
            visited += 1;
        } else {
            break;
        }
    }
    idx
}
