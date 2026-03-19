//! Unified RC emission: per-block walk with inline death/alloc event collection.
//!
//! Extracted from `realize/mod.rs` to stay under the 500-line file limit.
//! Called by [`super::realize_rc_reuse()`] as Phase 1 sub-step B.

use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::emit_reuse::{AllocEvent, DeathEvent};
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

use super::metrics;
use super::walk;

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
        block_id, coalesce_block_rc, collect_all_borrowed_defs, collect_borrowed_defs,
        collect_defined_vars, collect_inline_enum_projected_defs, collect_iter_element_defs,
        collect_project_borrowed_defs, compute_child_effective_last_use, emit_dead_at_entry_decs,
        emit_dead_invoke_dsts, emit_edge_cleanup, emit_terminator_rc, precompute_block_uses,
        BlockCtx,
    };

    debug_assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    let all_borrowed_defs = collect_all_borrowed_defs(func);
    let project_borrowed_defs = collect_project_borrowed_defs(func);
    let iter_element_defs = collect_iter_element_defs(func, interner);
    let inline_enum_projected_defs = collect_inline_enum_projected_defs(func, pool);
    let iter_fn_name = interner.intern("iter");
    let mut all_death_events = Vec::new();
    let mut all_alloc_events = Vec::new();
    let mut block_deferred: FxHashMap<usize, Vec<(ArcVarId, RcStrategy)>> = FxHashMap::default();
    let mut synergy = metrics::SynergyMetrics::default();

    // Phase 1: per-block RC emission via unified forward walk.
    for block_idx in 0..func.blocks.len() {
        let blk = block_id(block_idx);
        let use_info = precompute_block_uses(&func.blocks[block_idx]);
        let defined_in_block = collect_defined_vars(&func.blocks[block_idx]);
        let borrowed_defs = collect_borrowed_defs(&func.blocks[block_idx]);
        let child_elu = compute_child_effective_last_use(&func.blocks[block_idx], &use_info);

        let old_body = std::mem::take(&mut func.blocks[block_idx].body);
        let mut new_body: Vec<ArcInstr> = Vec::with_capacity(old_body.len() * 2);

        // NLL pattern: BlockCtx borrows func immutably; after last use of ctx,
        // the borrow ends and func is available for mutation.
        let ctx = BlockCtx {
            func,
            blk,
            state_map,
            defined_in_block: &defined_in_block,
            borrowed_defs: &borrowed_defs,
            all_borrowed_defs: &all_borrowed_defs,
            project_borrowed_defs: &project_borrowed_defs,
            iter_element_defs: &iter_element_defs,
            inline_enum_projected_defs: &inline_enum_projected_defs,
            use_info: &use_info,
            pool,
            child_effective_last_use: &child_elu,
        };

        // Phase A: RcDec for variables live at entry, unused, dead at exit.
        emit_dead_at_entry_decs(&ctx, &mut new_body);

        // Phase B: unified forward walk (decide() + inline event collection).
        let walk::BodyWalkResult {
            uses_so_far,
            terminator_deferred,
            death_events,
            alloc_events,
            walk_metrics,
        } = walk::walk_body_unified(&ctx, &old_body, &mut new_body, iter_fn_name);
        synergy.merge(&walk_metrics);

        // Phase C: terminator uses and cleanup.
        emit_terminator_rc(&ctx, block_idx, uses_so_far, &mut new_body);

        // After last use of ctx, NLL releases the immutable borrow.

        // For terminators without successors, emit deferred parent decs
        // in the body. For terminators with successors, return them for
        // edge cleanup.
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
            block_deferred.insert(block_idx, edge_deferred);
        }

        all_death_events.extend(death_events);
        all_alloc_events.extend(alloc_events);
    }

    // Phase 1.5: dead Invoke result cleanup.
    emit_dead_invoke_dsts(func, state_map, pool, &all_borrowed_defs);

    // Phase 2: inter-block edge cleanup (with deferred parent decs).
    emit_edge_cleanup(func, state_map, pool, &all_borrowed_defs, &block_deferred);

    // Phase 3: RC coalescing peephole — merge adjacent RC ops per block.
    for block in &mut func.blocks {
        coalesce_block_rc(&mut block.body);
    }

    let rc_count = count_rc_ops(func);
    (rc_count, all_death_events, all_alloc_events, synergy)
}

/// Count RC operations (`RcInc` + `RcDec`) in a function.
fn count_rc_ops(func: &ArcFunction) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .count()
}
