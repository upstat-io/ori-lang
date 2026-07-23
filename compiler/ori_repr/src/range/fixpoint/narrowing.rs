//! Post-fixpoint narrowing and recomputation passes.
//!
//! After the forward fixpoint converges (possibly with widened ranges),
//! these passes recover precision:
//! - **Narrowing**: intersects widened ranges with transfer function output
//! - **Field summary recomputation**: clears and rebuilds from final ranges
//! - **Return range recomputation**: rebuilds from final narrowed variables

use super::super::field_summary::{
    update_element_summaries, update_element_summaries_from_terminator, update_field_summaries,
    ElementSummaryTable, FieldSummaryTable,
};
use super::super::transfer::{transfer, transfer_known_call, TransferContext};
use super::super::{is_int_typed, ValueRange};
use super::iteration::{apply_block_refinements, restore_block_refinements};
use super::{narrow, FixpointContext};
use ori_arc::ir::{ArcFunction, ArcTerminator};
use ori_arc::ArcBlockId;
use ori_arc::ArcVarId;
use ori_types::Pool;
use rustc_hash::FxHashMap;
use ValueRange::{Bottom, Top};

/// Run one narrowing pass over all blocks to recover precision lost to widening.
///
/// Also re-merges block parameters from predecessors, applies
/// block refinements (branch/switch), and narrows invoke terminators.
/// This allows widened loop-header parameters to recover bounded ranges.
pub(super) fn run_narrowing_pass(
    context: &FixpointContext<'_>,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    field_summary_table: &FieldSummaryTable,
    direct_field_sources: &FxHashMap<(ArcVarId, u32), ArcVarId>,
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
) {
    for &block_idx in context.rpo {
        let block = &context.func.blocks[block_idx];

        // INVARIANT: Predecessor `Bottom` cannot replace seeded entry-parameter ranges.
        for (param_idx, (param_var, _)) in block.params.iter().enumerate() {
            if context.predecessors[block_idx].is_empty() {
                continue;
            }
            let mut merged = Bottom;
            for &pred_idx in &context.predecessors[block_idx] {
                let pred = &context.func.blocks[pred_idx];
                if let ArcTerminator::Jump { target, args, .. } = &pred.terminator {
                    if target.index() == block_idx {
                        if let Some(&arg_var) = args.get(param_idx) {
                            merged = merged.join(ranges.get(&arg_var).copied().unwrap_or(Bottom));
                        }
                    }
                }
            }
            if let Some(&refinement) = block_refinements.get(&(block.id, *param_var)) {
                merged = merged.meet(refinement);
            }
            if let Some(&widened) = ranges.get(param_var) {
                let narrowed = narrow(widened, merged);
                if narrowed != widened {
                    ranges.insert(*param_var, narrowed);
                }
            }
        }

        let saved = apply_block_refinements(block, ranges, block_refinements);

        // INVARIANT: Each transfer observes earlier narrowed values in the block.
        let field_summaries = field_summary_table.as_map();
        for instr in &block.body {
            let computed = {
                let ctx = TransferContext {
                    ranges: &*ranges,
                    pool: context.pool,
                    var_types: &context.func.var_types,
                    field_summaries,
                    direct_field_sources,
                    known_builtins: context.known_builtins,
                };
                transfer(instr, &ctx)
            };

            let Some(var) = instr.defined_var() else {
                continue;
            };
            if let Some(&widened) = ranges.get(&var) {
                let narrowed = narrow(widened, computed);
                if narrowed != widened {
                    ranges.insert(var, narrowed);
                }
            }
        }

        restore_block_refinements(ranges, saved);

        if let ArcTerminator::Invoke {
            dst,
            ty,
            func: callee,
            ..
        } = &block.terminator
        {
            if is_int_typed(*ty, context.pool) {
                let mut computed =
                    transfer_known_call(*callee, context.known_builtins).unwrap_or(Top);
                if let Some(&crn) = context.call_result_narrowings.get(dst) {
                    computed = computed.meet(crn);
                }
                if let Some(&widened) = ranges.get(dst) {
                    let narrowed = narrow(widened, computed);
                    if narrowed != widened {
                        ranges.insert(*dst, narrowed);
                    }
                }
            }
        }
    }
}

/// Recompute field summaries from final ranges (post-narrowing).
///
/// During the fixpoint loop, field summaries may accumulate wider ranges
/// from pre-convergence iterations. This clears and recomputes from the
/// converged ranges.
pub(super) fn recompute_field_summaries(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
    field_summary_table: &mut FieldSummaryTable,
) {
    field_summary_table.clear();
    for &block_idx in rpo {
        for instr in &func.blocks[block_idx].body {
            update_field_summaries(instr, ranges, &func.var_types, pool, field_summary_table);
        }
    }
}

/// Recompute element summaries from final (post-narrowing) variable ranges.
///
/// Rebuilds element summaries without pre-convergence ranges.
pub(super) fn recompute_element_summaries(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
    element_summary_table: &mut ElementSummaryTable,
) {
    element_summary_table.clear();
    for &block_idx in rpo {
        for instr in &func.blocks[block_idx].body {
            update_element_summaries(instr, ranges, &func.var_types, pool, element_summary_table);
        }
        update_element_summaries_from_terminator(
            &func.blocks[block_idx].terminator,
            pool,
            element_summary_table,
        );
    }
}

/// Recomputes the return range after loop narrowing. Only RPO-reachable blocks
/// contribute; including unanalyzed unreachable variables would introduce
/// `Top` through the missing-range fallback.
pub(super) fn recompute_return_range(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
) -> ValueRange {
    if !is_int_typed(func.return_type, pool) {
        return Bottom;
    }
    let mut result = Bottom;
    for &block_idx in rpo {
        let block = &func.blocks[block_idx];
        if let ArcTerminator::Return { value } = &block.terminator {
            let ret_range = ranges.get(value).copied().unwrap_or(Top);
            result = result.join(ret_range);
        }
    }
    result
}
