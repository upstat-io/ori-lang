//! Unified RC emission: per-block walk with inline death/alloc event collection.
//!
//! Extracted from `realize/mod.rs` to stay under the 500-line file limit.
//! Called by [`super::realize_rc_reuse()`] as Phase 1 sub-step B.

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{MemoryContract, ReturnAliasShape};
use crate::aims::emit_rc::DeferredDec;
use crate::aims::emit_reuse::{AllocEvent, DeathEvent};
use crate::aims::intraprocedural::apply_aliases::build_let_alias_map;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

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
/// insertions. § Debugging.
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
    contracts: &FxHashMap<Name, MemoryContract>,
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

    // BUG-04-090 §05 Step 6: pre-compute the set of parameter ArcVarIds
    // whose `ParamContract.transfers_through_return` is true. Threaded
    // into BlockCtx for consumption by `should_suppress_return_transfer_dec`
    // in the realize walk. Empty set when no contract is available
    // (FFI / external) — equivalent to the pre-fix behavior.
    let return_transfer_params: FxHashSet<ArcVarId> = contracts
        .get(&func.name)
        .map(|c| {
            func.params
                .iter()
                .enumerate()
                .filter(|(i, _)| c.params.get(*i).is_some_and(|p| p.transfers_through_return))
                .map(|(_, param)| param.var)
                .collect()
        })
        .unwrap_or_default();

    // BUG-04-090 §05 Step 6: alias map carrier — variable → set of param
    // indices it aliases. Reuses `build_alias_to_param_map` from
    // interprocedural extraction (single canonical alias-tracing source per
    // `LEAK:algorithmic-duplication`). Consumed by `traces_to_var` to
    // resolve whether a Return terminator's value aliases a return-transfer
    // param. Empty map suffices when `return_transfer_params` is empty —
    // the suppression helper short-circuits before consulting the map.
    let alias_to_param: FxHashMap<ArcVarId, FxHashSet<usize>> = if return_transfer_params.is_empty()
    {
        FxHashMap::default()
    } else {
        let param_vars: FxHashMap<ArcVarId, usize> = func
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.var, i))
            .collect();
        crate::aims::interprocedural::build_alias_to_param_map(func, &param_vars, Some(contracts))
    };

    // BUG-04-090 §05 F-prj: per-Project compensating-Inc targets. See the
    // doc comment on `BlockCtx::return_project_inc_targets` and the helper
    // `build_return_project_inc_targets` below for the full rationale.
    // Empty when no contract is available OR no params carry
    // `return_alias = Some(Project { _ })`.
    let return_project_inc_targets: FxHashMap<ArcVarId, RcStrategy> = contracts
        .get(&func.name)
        .map(|c| build_return_project_inc_targets(func, c, pool))
        .unwrap_or_default();
    // Per-class take-project facts via union-find +
    // CFG reachability. Precomputed once per function. Each
    // take-project source seeds its own connected-component class
    // (Let-alias + Jump-arg → block-param edges); each class has
    // its own bypass-safe blocks and bypass-safe entries. Consumers
    // (`dead_cleanup` source 1, `dead_cleanup` source 2,
    // `edge_cleanup`) query via `is_in_class`, `class_of`, and
    // `is_bypass_safe_entry_for_var` to coordinate exactly-one drop
    // per CFG path without double-free or leak.
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
            &return_transfer_params,
            &alias_to_param,
            &return_project_inc_targets,
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
    super::project_escape::emit_project_escape_incs(
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
    return_transfer_params: &FxHashSet<ArcVarId>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    return_project_inc_targets: &FxHashMap<ArcVarId, RcStrategy>,
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
        return_transfer_params,
        alias_to_param,
        return_project_inc_targets,
    };

    let (deferred_parents, merge_edge_decs) = emit_dead_at_entry_decs(&ctx, &mut new_body);

    let walk::BodyWalkResult {
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

    emit_terminator_rc(&ctx, block_idx, &mut new_body);

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

/// BUG-04-090 §05 F-prj: precompute the per-Project compensating-Inc target map.
///
/// Identifies every `Project { dst, value, field }` instruction whose `dst`
/// flows to a `Return` terminator AND whose `value` resolves (via Let-alias
/// chain) to a function param `p` whose `ParamContract.return_alias` is
/// `Some(Project { field: F })` with `F == field`. Fires regardless of `p`'s
/// own access class — the Inc compensates for the `AggFields` walk that fires
/// at whichever scope holds the parent allocation when the call returns,
/// callee-side (Owned-callee scope-exit drop) or caller-side (Owned-caller
/// arg drop after the Apply).
///
/// Each such `dst` maps to its `RcStrategy`. The realize walk consumes this
/// map to emit `RcInc dst [strategy]` immediately after the Project. Without
/// this Inc, the field-walk inside `[AggFields]` decrements the projected
/// allocation to 0 BEFORE the consumer of `dst` (Return → caller's xs) reads
/// it — use-after-free.
///
/// Returns an empty map when the contract has no Project `return_alias` entries
/// — bypasses Project-instruction iteration entirely in the common case.
fn build_return_project_inc_targets(
    func: &ArcFunction,
    contract: &MemoryContract,
    pool: &Pool,
) -> FxHashMap<ArcVarId, RcStrategy> {
    use crate::aims::emit_rc::rc_strategy;

    let project_return_params: FxHashMap<ArcVarId, u32> = func
        .params
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let pc = contract.params.get(i)?;
            match pc.return_alias? {
                ReturnAliasShape::Project { field } => Some((p.var, field)),
                ReturnAliasShape::Direct => None,
            }
        })
        .collect();
    if project_return_params.is_empty() {
        return FxHashMap::default();
    }

    let let_alias_map = build_let_alias_map(func);
    let resolve_root = |var: ArcVarId| -> ArcVarId {
        let mut current = var;
        for _ in 0..64 {
            match let_alias_map.get(&current) {
                Some(&src) => current = src,
                None => break,
            }
        }
        current
    };

    let return_values: FxHashSet<ArcVarId> = func
        .blocks
        .iter()
        .filter_map(|b| match &b.terminator {
            ArcTerminator::Return { value } => Some(*value),
            _ => None,
        })
        .collect();

    let mut result: FxHashMap<ArcVarId, RcStrategy> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project {
                dst, value, field, ..
            } = instr
            {
                if !return_values.contains(dst) {
                    continue;
                }
                let root = resolve_root(*value);
                if project_return_params.get(&root) == Some(field) {
                    if let Some(strategy) = rc_strategy(func, *dst, pool) {
                        result.insert(*dst, strategy);
                    }
                }
            }
        }
    }
    result
}
