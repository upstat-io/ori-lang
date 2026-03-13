//! Unified realization — one decision surface for all AIMS outputs.
//!
//! Replaces the four separate emission passes (`emit_rc_ops`, `emit_reuse`,
//! `compute_aims_cow_annotations`, `compute_aims_drop_hints`) with a
//! two-phase realization that reads the converged [`AimsStateMap`] through
//! unified decision functions.
//!
//! # Architecture
//!
//! - **Phase 1** ([`realize_rc_reuse`]): pre-merge. Forward walk calling
//!   `decide()` for RC and reuse decisions. Calls edge cleanup at the end.
//! - **Phase 2** ([`realize_annotations`]): post-merge. Walks post-merge IR
//!   using ArcVarId-keyed state lookups for COW and drop hint decisions.
//!
//! Both phases share the same [`AimsStateMap`] and decision surface.
//! No output owns an independent decision procedure.
//!
//! # References
//!
//! - Section 10 of the AIMS plan (`plans/aims/section-10-unified-realization.md`)
//! - Perceus (Reinking et al., PLDI 2021): unified RC + reuse
//! - FP² (Marshall et al., ESOP 2022): FIP-guided reuse decisions

pub mod decide;
#[cfg(test)]
mod tests;
mod walk;

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::emit_reuse::{AllocEvent, DeathEvent, FipGateRecord};
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};
use crate::uniqueness::drop_hints::DropHints;
use crate::uniqueness::CowAnnotations;

/// Result of the unified realization — all outputs in one struct.
///
/// Phase 1 (`realize_rc_reuse`) populates `rc_ops_inserted`,
/// `reuse_ops_inserted`, and `fip_evidence`. Phase 2
/// (`realize_annotations`) populates `cow_annotations` and `drop_hints`.
#[derive(Debug)]
pub struct RealizationResult {
    /// RC operations inserted (`RcInc` + `RcDec` count).
    pub rc_ops_inserted: usize,
    /// Reuse operations inserted (Reset + Reuse + `IsShared` count).
    pub reuse_ops_inserted: usize,
    /// COW annotations computed in Phase 2, keyed by `(block_idx, instr_idx)`.
    pub cow_annotations: CowAnnotations,
    /// Drop hints computed in Phase 2, keyed by `(block_idx, instr_idx)`.
    pub drop_hints: DropHints,
    /// FIP diagnostic evidence (missed reuses, gate records).
    /// NOT the authoritative FIP classification — that is
    /// `MemoryContract.fip`, owned by interprocedural analysis.
    pub fip_evidence: FipEvidence,
}

/// FIP diagnostic evidence accumulated during realization.
///
/// This is NOT the authoritative FIP classification. `MemoryContract.fip`
/// is authoritative (computed by `extract_contract()` in interprocedural
/// analysis). Realization consumes the contract and emits evidence that
/// verification can cross-check against it.
#[derive(Debug, Default)]
pub struct FipEvidence {
    /// FIP gate records from reuse emission.
    pub fip_gates: Vec<FipGateRecord>,
    /// Missed reuse opportunities in FIP functions.
    pub missed_reuses: usize,
}

/// Phase 1: RC and reuse emission (pre-merge).
///
/// Reads the converged [`AimsStateMap`], emits `RcInc`/`RcDec` and
/// `Reset`/`Reuse` operations, populates `arg_ownership` on
/// `Apply`/`Invoke` instructions, and calls edge cleanup.
///
/// Returns a partial [`RealizationResult`] — `cow_annotations` and
/// `drop_hints` are empty (populated by Phase 2 after `merge_blocks`).
///
/// # Pipeline position
///
/// Runs AFTER `analyze_function()` (step 4) and BEFORE `verify()` (step 6).
/// Replaces old steps 4 (`emit_arg_ownership`), 6 (`emit_rc_ops`), and
/// 7 (`emit_reuse`).
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn realize_rc_reuse(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    builtins: &BuiltinOwnershipSets,
    pool: &Pool,
) -> RealizationResult {
    // Sub-step A: emit arg_ownership (previously standalone step 4).
    {
        let _span = tracing::debug_span!("realize_arg_ownership").entered();
        crate::aims::emit_rc::arg_ownership::emit_arg_ownership(
            func, contracts, interner, builtins, pool,
        );
    }

    // Sub-step B: unified RC emission + inline event collection.
    // Replaces emit_rc_ops() with a forward walk routing all decisions
    // through decide(), collecting death/alloc events inline.
    let (rc_ops_inserted, death_events, alloc_events) = {
        let _span = tracing::debug_span!("realize_rc_unified").entered();
        emit_rc_unified(func, state_map, pool)
    };

    // Sub-step C: emit reuse from collected events (replaces emit_reuse scan).
    let (reuse_ops_inserted, fip_evidence) = {
        let _span = tracing::debug_span!("realize_reuse").entered();
        let reuse_result = crate::aims::emit_reuse::emit_reuse_from_events(
            func,
            &death_events,
            &alloc_events,
            contracts,
        );
        if !reuse_result.fip_gates.is_empty() {
            tracing::debug!(
                function = func.name.raw(),
                fip_gates = reuse_result.fip_gates.len(),
                "FIP gate records captured during realization"
            );
        }
        let ops = reuse_result.static_reuses
            + reuse_result.dynamic_reuses
            + reuse_result.cross_block_reuses;
        let evidence = FipEvidence {
            fip_gates: reuse_result.fip_gates,
            missed_reuses: reuse_result.missed_reuses,
        };
        (ops, evidence)
    };

    RealizationResult {
        rc_ops_inserted,
        reuse_ops_inserted,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        fip_evidence,
    }
}

/// Phase 2: COW and drop hint annotations (post-merge).
///
/// Reads the [`AimsStateMap`] via ArcVarId-keyed lookups on the post-merge
/// IR. Computes `cow_annotations` and `drop_hints`, completing the
/// [`RealizationResult`].
///
/// # Pipeline position
///
/// Runs AFTER `merge_blocks()` (step 9). Replaces old steps 11a
/// (`compute_aims_cow_annotations`) and 12 (`compute_aims_drop_hints`).
///
/// # Panics
///
/// Does NOT panic on failure — logs `tracing::error!` and leaves
/// annotations empty (functionally correct but suboptimal).
pub fn realize_annotations(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    result: &mut RealizationResult,
) {
    // Sub-step D: COW annotations (previously step 11a).
    result.cow_annotations = {
        let _span = tracing::debug_span!("realize_cow").entered();
        crate::aims::emit_rc::cow::compute_aims_cow_annotations(func, state_map, interner)
    };

    // Sub-step E: drop hints (previously step 12).
    result.drop_hints = {
        let _span = tracing::debug_span!("realize_drop_hints").entered();
        crate::aims::emit_rc::drop_hints::compute_aims_drop_hints(func, state_map, pool)
    };
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
fn emit_rc_unified(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
) -> (usize, Vec<DeathEvent>, Vec<AllocEvent>) {
    use crate::aims::emit_rc::{
        block_id, coalesce_block_rc, collect_all_borrowed_defs, collect_borrowed_defs,
        collect_defined_vars, compute_child_effective_last_use, emit_dead_at_entry_decs,
        emit_dead_invoke_dsts, emit_edge_cleanup, emit_terminator_rc, precompute_block_uses,
        BlockCtx,
    };

    debug_assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    let all_borrowed_defs = collect_all_borrowed_defs(func);
    let mut all_death_events = Vec::new();
    let mut all_alloc_events = Vec::new();
    let mut block_deferred: FxHashMap<usize, Vec<(ArcVarId, RcStrategy)>> = FxHashMap::default();

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
        } = walk::walk_body_unified(&ctx, &old_body, &mut new_body);

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
    (rc_count, all_death_events, all_alloc_events)
}

/// Count RC operations (`RcInc` + `RcDec`) in a function.
fn count_rc_ops(func: &ArcFunction) -> usize {
    use crate::ir::ArcInstr;
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .count()
}
