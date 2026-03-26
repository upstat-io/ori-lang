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

use super::super::field_summary::{update_field_summaries, FieldSummaryTable};
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
pub(super) fn run_narrowing_pass(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    field_summary_table: &FieldSummaryTable,
    predecessors: &[Vec<usize>],
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
) {
    for &block_idx in rpo {
        let block = &func.blocks[block_idx];

        // Narrow block parameters from predecessor jump args.
        for (param_idx, (param_var, _)) in block.params.iter().enumerate() {
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
        let mut updates: Vec<(ArcVarId, ValueRange)> = Vec::new();
        {
            let ctx = TransferContext {
                ranges,
                pool,
                var_types: &func.var_types,
                field_summaries: field_summary_table.as_map(),
            };
            for instr in &block.body {
                let computed = transfer(instr, &ctx);
                let Some(var) = instr.defined_var() else {
                    continue;
                };
                if let Some(&widened) = ranges.get(&var) {
                    let narrowed = narrow(widened, computed);
                    if narrowed != widened {
                        updates.push((var, narrowed));
                    }
                }
            }
        }
        for (var, narrowed) in updates {
            ranges.insert(var, narrowed);
        }

        // Restore temporary refinements.
        restore_block_refinements(ranges, saved);

        // Narrow invoke terminator.
        if let ArcTerminator::Invoke {
            dst,
            ty,
            func: callee,
            ..
        } = &block.terminator
        {
            if is_int_typed(*ty, pool) {
                let computed = transfer_known_call(*callee, pool).unwrap_or(Top);
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

/// Recompute `return_range` from the final narrowed variable ranges.
///
/// During forward iterations, `return_range` accumulates pre-narrowing values.
/// After narrowing recovers precision for loop variables, `return_range` must
/// be recomputed so the §03.5 interprocedural handoff uses the tightened ranges.
/// See TPR-03-021.
pub(super) fn recompute_return_range(
    func: &ArcFunction,
    pool: &Pool,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
) -> ValueRange {
    if !is_int_typed(func.return_type, pool) {
        return Bottom;
    }
    let mut result = Bottom;
    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            let ret_range = ranges.get(value).copied().unwrap_or(Top);
            result = result.join(ret_range);
        }
    }
    result
}
