//! AIMS decisions for logical ownership events, reuse, COW, and drop hints.
//!
//! [`realize_rc_reuse`] consumes converged state before block merging;
//! [`realize_annotations`] consumes the same state after merging. Both use the
//! shared [`AimsStateMap`] and decision functions.

pub mod decide;
#[cfg(test)]
mod dimension_consumer;
mod emit_unified;
pub use emit_unified::push_receiver_lineage_returned;
pub mod metrics;
pub mod rc_remark;
pub mod rl31_disjoint;
#[cfg(test)]
mod tests;

pub(crate) use rc_remark::emit_survivor_remarks_all_kept;
pub use rl31_disjoint::{prove_param_disjointness, ParamDisjointnessFacts};

use ori_ir::Name;
use ori_types::{Pool, TypeRegistry};
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::emit_reuse::FipGateRecord;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::uniqueness::drop_hints::DropHints;
use crate::uniqueness::CowAnnotations;

/// Outputs accumulated across ownership/reuse realization and annotations.
#[derive(Debug)]
pub struct RealizationResult {
    /// Logical ownership events materialized as `RcInc` and `RcDec`.
    pub rc_ops_inserted: usize,
    /// Reuse operations materialized as `Reset`, `Reuse`, and `IsShared`.
    pub reuse_ops_inserted: usize,
    /// COW annotations keyed by `(block_idx, instr_idx)`.
    pub cow_annotations: CowAnnotations,
    /// Drop hints keyed by `(block_idx, instr_idx)`.
    pub drop_hints: DropHints,
    /// Missed-reuse and gate evidence for FIP verification.
    pub fip_evidence: FipEvidence,
    /// Cross-dimensional metrics accumulated by both realization passes.
    pub synergy_metrics: metrics::SynergyMetrics,
}

/// FIP evidence accumulated for contract verification.
///
/// [`MemoryContract::fip`] remains the authoritative classification.
#[derive(Debug, Default)]
pub struct FipEvidence {
    /// FIP gate records from reuse emission.
    pub fip_gates: Vec<FipGateRecord>,
    /// Missed reuse opportunities in FIP functions.
    pub missed_reuses: usize,
}

/// Materializes logical ownership events and reuse before CFG merging.
///
/// The converged state supplies the class-ledger events; the current IR encodes
/// them with `RcInc`/`RcDec`, `Reset`/`Reuse`, argument ownership, and edge
/// cleanup. COW and drop outputs remain empty until [`realize_annotations`].
pub(crate) fn realize_rc_reuse(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    _builtins: &BuiltinOwnershipSets,
    pool: &Pool,
    type_registry: &TypeRegistry,
) -> RealizationResult {
    // INVARIANT: Argument ownership is converged before this realization pass.
    let (rc_ops_inserted, death_events, alloc_events, phase1_metrics) = {
        let _span = tracing::debug_span!("realize_rc_unified").entered();
        emit_unified::emit_rc_unified(func, state_map, pool, interner, contracts, type_registry)
    };

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
        let ops = reuse_result
            .static_reuses
            .checked_add(reuse_result.dynamic_reuses)
            .and_then(|count| count.checked_add(reuse_result.cross_block_reuses));
        let Some(ops) = ops else {
            panic!("AIMS reuse-operation count must fit usize");
        };
        let evidence = FipEvidence {
            fip_gates: reuse_result.fip_gates,
            missed_reuses: reuse_result.missed_reuses,
        };
        (ops, evidence)
    };

    // INVARIANT: Metrics report only after both realization passes contribute.
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

/// Inputs read together by post-merge annotation decisions.
pub struct AnnotationEnv<'a> {
    pub state_map: &'a AimsStateMap,
    pub interner: &'a ori_ir::StringInterner,
    pub pool: &'a Pool,
    pub contracts: &'a rustc_hash::FxHashMap<ori_ir::Name, crate::aims::contract::MemoryContract>,
    pub builtins: &'a crate::borrow::BuiltinOwnershipSets,
    pub func_names: &'a rustc_hash::FxHashSet<ori_ir::Name>,
}

/// Derives COW and drop annotations from post-merge IR in one walk.
///
/// Each candidate site receives one [`AnnotationSiteContext`], so COW and drop
/// decisions read the same converged facts.
pub(crate) fn realize_annotations(
    func: &ArcFunction,
    env: &AnnotationEnv<'_>,
    result: &mut RealizationResult,
) {
    use crate::aims::emit_rc::{
        block_id, collect_borrowed_call_args, collect_param_borrowed_vars,
        collect_rc_incremented_vars,
    };

    let _span = tracing::debug_span!("realize_annotations").entered();

    let cow_names = crate::borrow::all_cow_method_names(env.interner);
    let param_vars: rustc_hash::FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let param_borrowed_vars = collect_param_borrowed_vars(func);
    let rc_incremented =
        collect_rc_incremented_vars(func, env.state_map.birth_site_partition(), env.contracts);
    let borrowed_call_args =
        collect_borrowed_call_args(func, env.contracts, env.builtins, env.func_names);

    let mut cow_annotations = CowAnnotations::new();
    let mut drop_hints = DropHints::new();

    let ann_ctx = AnnotationWalkCtx {
        func,
        state_map: env.state_map,
        pool: env.pool,
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

    result.synergy_metrics.report(func.name.raw());
}

/// Precomputed inputs shared across annotation sites.
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

/// Applies COW and drop decisions to one block and its terminator.
fn annotate_block(
    ctx: &AnnotationWalkCtx<'_>,
    blk: crate::ir::ArcBlockId,
    block_idx: usize,
    block: &crate::ir::ArcBlock,
    cow_annotations: &mut CowAnnotations,
    drop_hints: &mut DropHints,
    synergy: &mut metrics::SynergyMetrics,
) {
    use crate::aims::emit_rc::{
        has_borrows_from_aggregate, is_borrow_disjoint_from_siblings, is_collection_var,
    };
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
        // INVARIANT: Only uniqueness is contract-narrowed at COW sites.
        let site_ctx = AnnotationSiteContext {
            var,
            uniqueness: ctx.state_map.effective_uniqueness_at_block_entry(blk, var),
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
            is_borrow_disjoint: is_borrow_disjoint_from_siblings(ctx.state_map, var, blk),
            has_active_borrows: has_borrows_from_aggregate(ctx.state_map, var),
            is_collection: is_collection_var(ctx.func, var, ctx.pool),
        };

        let decisions = decide_annotations(&site_ctx, is_cow_site, is_drop_site);

        if let Some(mode) = decisions.cow {
            synergy.total_cow_decisions += 1;
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

    if let ArcTerminator::Invoke {
        func: callee, args, ..
    } = &block.terminator
    {
        if ctx.cow_names.contains(callee) && !args.is_empty() {
            let receiver = args[0];
            let state = ctx.state_map.var_state_at_block_entry(blk, receiver);
            // INVARIANT: Invoke and Apply sites use the same effective uniqueness.
            let site_ctx = AnnotationSiteContext {
                var: receiver,
                uniqueness: ctx
                    .state_map
                    .effective_uniqueness_at_block_entry(blk, receiver),
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
                is_borrow_disjoint: is_borrow_disjoint_from_siblings(ctx.state_map, receiver, blk),
                has_active_borrows: has_borrows_from_aggregate(ctx.state_map, receiver),
                is_collection: is_collection_var(ctx.func, receiver, ctx.pool),
            };

            let decisions = decide_annotations(&site_ctx, true, false);
            if let Some(mode) = decisions.cow {
                synergy.total_cow_decisions += 1;
                cow_annotations.set(block_idx, block.body.len(), mode);
            }
        }
    }
}
