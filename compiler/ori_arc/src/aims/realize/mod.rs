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
pub mod metrics;
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
use crate::uniqueness::{CowAnnotations, CowMode};

/// Result of the unified realization — all outputs in one struct.
///
/// Phase 1 (`realize_rc_reuse`) populates `rc_ops_inserted`,
/// `reuse_ops_inserted`, and `fip_evidence`. Phase 2
/// (`realize_annotations`) populates `cow_annotations` and `drop_hints`.
/// Both phases accumulate into `synergy_metrics`.
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
    /// Cross-dimensional synergy metrics (Section 11.2).
    ///
    /// Phase 1 populates RC/reuse metrics, Phase 2 populates COW metrics.
    /// `canonicalize_cross_fires` is set externally from backward analysis.
    pub synergy_metrics: metrics::SynergyMetrics,
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
    let (rc_ops_inserted, death_events, alloc_events, phase1_metrics) = {
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

    // Phase 1 synergy metrics from the unified walk.
    // Report deferred to Phase 2 (realize_annotations) so both phases
    // contribute before the single report call.
    let synergy_metrics = phase1_metrics;

    RealizationResult {
        rc_ops_inserted,
        reuse_ops_inserted,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        fip_evidence,
        synergy_metrics,
    }
}

/// Phase 2: COW and drop hint annotations (post-merge).
///
/// Walks the post-merge IR once, building an [`AnnotationSiteContext`] for
/// each COW or drop site and calling [`decide_annotations()`] to get the
/// unified Phase 2 decision. This replaces the separate calls to
/// `compute_aims_cow_annotations()` and `compute_aims_drop_hints()`.
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
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn realize_annotations(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    contracts: &rustc_hash::FxHashMap<ori_ir::Name, crate::aims::contract::MemoryContract>,
    builtins: &crate::borrow::BuiltinOwnershipSets,
    result: &mut RealizationResult,
) {
    use crate::aims::emit_rc::{
        block_id, collect_borrowed_call_args, collect_param_borrowed_vars,
        collect_rc_incremented_vars,
    };

    let _span = tracing::debug_span!("realize_annotations").entered();

    let cow_names = crate::borrow::all_cow_method_names(interner);
    let param_vars: rustc_hash::FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let param_borrowed_vars = collect_param_borrowed_vars(func);
    let rc_incremented = collect_rc_incremented_vars(func);
    let borrowed_call_args = collect_borrowed_call_args(func, contracts, builtins);

    let mut cow_annotations = CowAnnotations::new();
    let mut drop_hints = DropHints::new();

    let ann_ctx = AnnotationWalkCtx {
        func,
        state_map,
        pool,
        cow_names: &cow_names,
        param_vars: &param_vars,
        param_borrowed_vars: &param_borrowed_vars,
        rc_incremented: &rc_incremented,
        borrowed_call_args: &borrowed_call_args,
    };

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = block_id(block_idx);
        annotate_block(
            &ann_ctx,
            blk,
            block_idx,
            block,
            &mut cow_annotations,
            &mut drop_hints,
            &mut result.synergy_metrics,
        );
    }

    if !drop_hints.is_empty() {
        tracing::debug!(
            function = func.name.raw(),
            unique_drops = drop_hints.len(),
            "AIMS drop hint analysis complete"
        );
    }

    result.cow_annotations = cow_annotations;
    result.drop_hints = drop_hints;

    // Report combined Phase 1+2 metrics.
    result.synergy_metrics.report(func.name.raw());
}

/// Pre-computed context for the Phase 2 annotation walk.
struct AnnotationWalkCtx<'a> {
    func: &'a ArcFunction,
    state_map: &'a AimsStateMap,
    pool: &'a Pool,
    cow_names: &'a rustc_hash::FxHashSet<Name>,
    param_vars: &'a rustc_hash::FxHashSet<ArcVarId>,
    param_borrowed_vars: &'a rustc_hash::FxHashSet<ArcVarId>,
    rc_incremented: &'a rustc_hash::FxHashSet<ArcVarId>,
    borrowed_call_args: &'a rustc_hash::FxHashSet<ArcVarId>,
}

/// Annotate a single block's body and terminator for Phase 2 decisions.
fn annotate_block(
    ctx: &AnnotationWalkCtx<'_>,
    blk: crate::ir::ArcBlockId,
    block_idx: usize,
    block: &crate::ir::ArcBlock,
    cow_annotations: &mut CowAnnotations,
    drop_hints: &mut DropHints,
    synergy: &mut metrics::SynergyMetrics,
) {
    use crate::aims::emit_rc::{is_borrow_disjoint_from_siblings, is_collection_var};
    use crate::aims::realize::decide::{decide_annotations, AnnotationSiteContext};

    for (instr_idx, instr) in block.body.iter().enumerate() {
        let is_cow_site = matches!(
            instr,
            ArcInstr::Apply { func: callee, args, .. }
                if ctx.cow_names.contains(callee) && !args.is_empty()
        );
        let is_drop_site = matches!(instr, ArcInstr::RcDec { .. });

        if !is_cow_site && !is_drop_site {
            continue;
        }

        let var = match instr {
            ArcInstr::Apply { args, .. } if is_cow_site => args[0],
            ArcInstr::RcDec { var, .. } => *var,
            _ => continue,
        };

        let state = ctx.state_map.var_state_at_block_entry(blk, var);
        let site_ctx = AnnotationSiteContext {
            var,
            uniqueness: state.uniqueness,
            rc_incremented: ctx.rc_incremented.contains(&var),
            is_param: ctx.param_vars.contains(&var),
            is_param_borrowed: ctx.param_borrowed_vars.contains(&var),
            is_borrowed_call_arg: ctx.borrowed_call_args.contains(&var),
            rc_incremented_set: ctx.rc_incremented,
            is_excluded: ctx.state_map.is_excluded(var),
            access: state.access,
            consumption: state.consumption,
            cardinality: state.cardinality,
            shape: ctx.state_map.var_shape(var),
            is_borrow_disjoint: is_borrow_disjoint_from_siblings(ctx.state_map, var),
            is_collection: is_collection_var(ctx.func, var, ctx.pool),
        };

        let decisions = decide_annotations(&site_ctx, is_cow_site, is_drop_site);

        if let Some(mode) = decisions.cow {
            synergy.total_cow_decisions += 1;
            // Cross-dim upgrade: MaybeShared uniqueness → StaticUnique via
            // combined state proof (COW-aware borrowing, CollectionBuffer+Once,
            // ReusableCtor+Once, disjoint borrow).
            if site_ctx.uniqueness == crate::aims::lattice::Uniqueness::MaybeShared
                && mode == CowMode::StaticUnique
            {
                synergy.cow_upgrades += 1;
            }
            tracing::debug!(
                block_idx,
                instr_idx,
                receiver = var.raw(),
                ?mode,
                "COW annotation"
            );
            cow_annotations.set(block_idx, instr_idx, mode);
        }
        if decisions.drop_hint {
            drop_hints.mark_unique(block_idx, instr_idx);
        }
    }

    // Terminator: check for COW Invoke.
    if let ArcTerminator::Invoke {
        func: callee, args, ..
    } = &block.terminator
    {
        if ctx.cow_names.contains(callee) && !args.is_empty() {
            let receiver = args[0];
            let state = ctx.state_map.var_state_at_block_entry(blk, receiver);
            let site_ctx = AnnotationSiteContext {
                var: receiver,
                uniqueness: state.uniqueness,
                rc_incremented: ctx.rc_incremented.contains(&receiver),
                is_param: ctx.param_vars.contains(&receiver),
                is_param_borrowed: ctx.param_borrowed_vars.contains(&receiver),
                is_borrowed_call_arg: ctx.borrowed_call_args.contains(&receiver),
                rc_incremented_set: ctx.rc_incremented,
                is_excluded: ctx.state_map.is_excluded(receiver),
                access: state.access,
                consumption: state.consumption,
                cardinality: state.cardinality,
                shape: ctx.state_map.var_shape(receiver),
                is_borrow_disjoint: is_borrow_disjoint_from_siblings(ctx.state_map, receiver),
                is_collection: is_collection_var(ctx.func, receiver, ctx.pool),
            };

            let decisions = decide_annotations(&site_ctx, true, false);
            if let Some(mode) = decisions.cow {
                synergy.total_cow_decisions += 1;
                if site_ctx.uniqueness == crate::aims::lattice::Uniqueness::MaybeShared
                    && mode == CowMode::StaticUnique
                {
                    synergy.cow_upgrades += 1;
                }
                cow_annotations.set(block_idx, block.body.len(), mode);
            }
        }
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
fn emit_rc_unified(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
) -> (
    usize,
    Vec<DeathEvent>,
    Vec<AllocEvent>,
    metrics::SynergyMetrics,
) {
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
        } = walk::walk_body_unified(&ctx, &old_body, &mut new_body);
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
    use crate::ir::ArcInstr;
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .count()
}
