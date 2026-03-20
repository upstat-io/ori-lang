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
mod emit_unified;
pub mod metrics;
#[cfg(test)]
mod tests;
mod walk;
mod walk_dec;

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::emit_reuse::FipGateRecord;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
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

    // Sub-step A2: insert RcInc for borrowed-param COW receivers.
    // Borrowed function params at COW call receiver positions need an RcInc
    // to prevent the runtime from seeing the buffer as unique (RC=1) when the
    // caller also holds a reference. Without this, COW push/set/insert may
    // realloc in place or dec the old buffer, invalidating the caller's pointer.
    inject_cow_borrowed_receiver_incs(func, interner, pool);

    // Sub-step B: unified RC emission + inline event collection.
    // Replaces emit_rc_ops() with a forward walk routing all decisions
    // through decide(), collecting death/alloc events inline.
    let (rc_ops_inserted, death_events, alloc_events, phase1_metrics) = {
        let _span = tracing::debug_span!("realize_rc_unified").entered();
        emit_unified::emit_rc_unified(func, state_map, pool, interner)
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

// emit_rc_unified and count_rc_ops moved to emit_unified.rs

/// Pre-pass: inject `RcInc` before COW calls whose receiver is a borrowed
/// function parameter (or alias thereof).
///
/// Borrowed parameters don't increment the refcount (that's the point of
/// borrowing). But COW operations check `ori_rc_is_unique(data)` at runtime
/// and will see RC=1 (the caller's sole reference), taking the fast path
/// (realloc/mutate in place) — invalidating the caller's pointer.
///
/// By inserting an `RcInc` before the COW call, the runtime sees RC=2 and
/// correctly takes the slow path (copy + dec old). The extra inc is matched
/// by the COW slow path's dec of the old buffer.
fn inject_cow_borrowed_receiver_incs(
    func: &mut crate::ir::ArcFunction,
    interner: &ori_ir::StringInterner,
    _pool: &ori_types::Pool,
) {
    use crate::aims::emit_rc::collect_cow_borrowed_receivers;
    use crate::ir::{ArcInstr, ArcTerminator, RcStrategy};

    let cow_borrowed_receivers = collect_cow_borrowed_receivers(func, interner);
    if cow_borrowed_receivers.is_empty() {
        return;
    }

    let cow_names = crate::borrow::all_cow_method_names(interner);

    for block in &mut func.blocks {
        // Body instructions: Apply COW calls with borrowed receivers
        let mut insertions: Vec<(usize, ArcVarId)> = Vec::new();
        for (i, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if cow_names.contains(callee) && !args.is_empty() {
                    let receiver = args[0];
                    if cow_borrowed_receivers.contains(&receiver) {
                        insertions.push((i, receiver));
                    }
                }
            }
        }
        // Insert in reverse order to preserve indices
        for (idx, var) in insertions.into_iter().rev() {
            block.body.insert(
                idx,
                ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                },
            );
        }

        // Terminator: Invoke COW call — insert RcInc at end of body
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if cow_names.contains(callee) && !args.is_empty() {
                let receiver = args[0];
                if cow_borrowed_receivers.contains(&receiver) {
                    block.body.push(ArcInstr::RcInc {
                        var: receiver,
                        count: 1,
                        strategy: RcStrategy::HeapPointer,
                    });
                }
            }
        }
    }
}
