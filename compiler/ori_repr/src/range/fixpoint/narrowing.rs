//! Post-fixpoint narrowing and recomputation passes.
//!
//! After the forward fixpoint converges (possibly with widened ranges),
//! these passes recover precision:
//! - **Narrowing**: intersects widened ranges with transfer function output
//! - **Field summary recomputation**: clears and rebuilds from final ranges
//! - **Return range recomputation**: rebuilds from final narrowed variables

use ori_arc::ir::{ArcFunction, ArcTerminator};
use ori_arc::ArcVarId;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::super::field_summary::{
    update_element_summaries, update_element_summaries_from_terminator, update_field_summaries,
    ElementSummaryTable, FieldSummaryTable,
};
use super::super::transfer::{transfer, transfer_known_call, TransferContext};
use super::super::{is_int_typed, ValueRange};
use super::{apply_block_refinements, narrow, restore_block_refinements};
use ori_arc::ArcBlockId;
use ValueRange::{Bottom, Top};

/// Run one narrowing pass over all blocks to recover precision lost to widening.
///
/// TPR-03-019: also re-merges block parameters from predecessors, applies
/// block refinements (branch/switch), and narrows invoke terminators.
/// This allows widened loop-header parameters to recover bounded ranges.
#[expect(
    clippy::too_many_arguments,
    reason = "fixpoint infrastructure passes — bundling would add indirection"
)]
pub(super) fn run_narrowing_pass(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    field_summary_table: &FieldSummaryTable,
    predecessors: &[Vec<usize>],
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    known_builtins: &super::super::KnownBuiltins,
    call_result_narrowings: &FxHashMap<ArcVarId, super::super::ValueRange>,
) {
    for &block_idx in rpo {
        let block = &func.blocks[block_idx];

        // Narrow block parameters from predecessor jump args.
        // Skip entry block parameters with no predecessors — they may be seeded
        // from interprocedural analysis, and narrowing against Bottom
        // (which means "no info from predecessors") would destroy those seeds.
        for (param_idx, (param_var, _)) in block.params.iter().enumerate() {
            if predecessors[block_idx].is_empty() {
                continue; // Entry block — preserve interprocedural seeds.
            }
            let mut merged = Bottom;
            for &pred_idx in &predecessors[block_idx] {
                let pred = &func.blocks[pred_idx];
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

        // Apply block-entry refinements temporarily (same as forward pass).
        let saved = apply_block_refinements(block, ranges, block_refinements);

        // Narrow body instructions.
        // Apply updates immediately so later instructions see narrowed values
        // from earlier instructions in the same block. This is critical for
        // loop body copy chains: %18 = %4 (narrowed via refinement) must be
        // visible when computing %20 = %18 + 1.
        let field_summaries = field_summary_table.as_map();
        for instr in &block.body {
            let computed = {
                let ctx = TransferContext {
                    ranges: &*ranges,
                    pool,
                    var_types: &func.var_types,
                    field_summaries,
                    known_builtins,
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

        // Restore temporary refinements.
        restore_block_refinements(ranges, saved);

        // Narrow invoke terminator.
        // TPR-03-034: also apply call_result_narrowings for Invoke dst (same
        // as forward pass), so return-range feedback reaches Invoke paths.
        if let ArcTerminator::Invoke {
            dst,
            ty,
            func: callee,
            ..
        } = &block.terminator
        {
            if is_int_typed(*ty, pool) {
                let mut computed = transfer_known_call(*callee, known_builtins).unwrap_or(Top);
                if let Some(&crn) = call_result_narrowings.get(dst) {
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
/// converged ranges. See TPR-03-016.
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
/// Same rationale as `recompute_field_summaries` — see TPR-03-016.
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
        // BUG-05-001: also check terminators for Invoke calls returning collections.
        update_element_summaries_from_terminator(
            &func.blocks[block_idx].terminator,
            pool,
            element_summary_table,
        );
    }
}

/// Recompute `return_range` from the final narrowed variable ranges.
///
/// During forward iterations, `return_range` accumulates pre-narrowing values.
/// After narrowing recovers precision for loop variables, `return_range` must
/// be recomputed so the interprocedural handoff uses the tightened ranges.
/// See TPR-03-021.
///
/// TPR-03-023: Only iterates reachable blocks (via `rpo`). Unreachable blocks
/// contain variables that were never analyzed, so `ranges.get()` returns `None`
/// and the `unwrap_or(Top)` fallback would pollute the return range.
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
