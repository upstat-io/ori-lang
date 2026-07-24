//! Fixpoint refinements are temporary overlays and widening thresholds remain finite.

use ori_arc::ir::{ArcFunction, ArcTerminator};
use ori_arc::{ArcBlockId, ArcVarId};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{FixpointContext, ValueRange};
use ValueRange::Bottom;

/// Apply block-entry refinements to non-parameter variables temporarily.
pub(super) fn apply_block_refinements(
    block: &ori_arc::ir::ArcBlock,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
) -> Vec<(ArcVarId, ValueRange)> {
    let mut saved = Vec::new();
    let param_vars: FxHashSet<_> = block.params.iter().map(|(var, _)| *var).collect();

    for (&(ref_block, ref_var), &refinement) in block_refinements {
        if ref_block != block.id || param_vars.contains(&ref_var) {
            continue;
        }
        if let Some(&current) = ranges.get(&ref_var) {
            let refined = current.meet(refinement);
            if refined != current {
                saved.push((ref_var, current));
                ranges.insert(ref_var, refined);
            }
        }
    }
    saved
}

/// Restore ranges temporarily refined for a block.
pub(super) fn restore_block_refinements(
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    saved: Vec<(ArcVarId, ValueRange)>,
) {
    for (var, original) in saved {
        ranges.insert(var, original);
    }
}

/// Collect integer constants used in comparison operations as widening thresholds.
///
/// Scans the function for comparison `PrimOps` and extracts constant operand
/// values. These serve as "landmarks" that prevent widening from jumping
/// directly to ±MAX. For example, `i >= 10` yields threshold 10, causing
/// widening to produce `[0, 10]` instead of `[0, MAX]`.
///
/// Only exact constants are included: neighboring values would make an
/// incrementing loop discover a fresh threshold on every iteration.
pub(super) fn collect_comparison_thresholds(
    func: &ArcFunction,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
) -> Vec<i64> {
    use ori_arc::ir::{ArcInstr, ArcValue, PrimOp};
    use ori_ir::BinaryOp;

    let mut thresholds = Vec::new();
    // Why: Zero keeps a shrinking zero-based lower bound from widening to `i64::MIN`.
    thresholds.push(0);

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                value:
                    ArcValue::PrimOp {
                        op: PrimOp::Binary(op),
                        args,
                    },
                ..
            } = instr
            {
                if matches!(
                    op,
                    BinaryOp::Lt
                        | BinaryOp::LtEq
                        | BinaryOp::Gt
                        | BinaryOp::GtEq
                        | BinaryOp::Eq
                        | BinaryOp::NotEq
                ) && args.len() == 2
                {
                    // Why: neighboring constants would make incrementing loops
                    // discover a fresh widening threshold on every iteration.
                    for &arg in args {
                        if let Some(val) = ranges.get(&arg).and_then(ValueRange::is_constant) {
                            thresholds.push(val);
                        }
                    }
                }
            }
        }
    }
    thresholds.sort_unstable();
    thresholds.dedup();
    thresholds
}

/// Process block parameters (phi-like merging from predecessor `Jump` args).
pub(super) fn merge_block_params(
    context: &FixpointContext<'_>,
    block: &ori_arc::ir::ArcBlock,
    block_idx: usize,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    iteration: usize,
    thresholds: &[i64],
) -> bool {
    use super::widen::update_range;

    let mut changed = false;
    for (param_idx, (param_var, _param_ty)) in block.params.iter().enumerate() {
        let mut merged = Bottom;
        for &pred_idx in &context.predecessors[block_idx] {
            let pred = &context.func.blocks[pred_idx];
            if let ArcTerminator::Jump { target, args, .. } = &pred.terminator {
                if target.index() == block_idx {
                    if let Some(&arg_var) = args.get(param_idx) {
                        let arg_range = ranges.get(&arg_var).copied().unwrap_or(Bottom);
                        merged = merged.join(arg_range);
                    }
                }
            }
        }
        if let Some(&refinement) = block_refinements.get(&(block.id, *param_var)) {
            merged = merged.meet(refinement);
        }
        changed |= update_range(ranges, *param_var, merged, iteration, thresholds);
    }
    changed
}

/// Propagate block refinements through single-predecessor jump chains.
///
/// The ARC pipeline may split the loop body into multiple blocks:
/// `bb1 → Branch → bb4 (false, gets refinement) → Jump bb5 → body → Jump bb1`.
/// Without propagation, bb5 doesn't inherit bb4's refinement, so the narrowing
/// pass can't tighten loop-body variables.
pub(super) fn propagate_refinements_through_jump_chains(
    func: &ArcFunction,
    predecessors: &[Vec<usize>],
    block_refinements: &mut FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
) {
    let mut propagated = Vec::new();
    for block in &func.blocks {
        let block_id = block.id;
        let block_idx = block_id.index();
        if predecessors[block_idx].len() != 1 {
            continue;
        }
        let pred_idx = predecessors[block_idx][0];
        let pred_id = func.blocks[pred_idx].id;
        for (&(ref_block, ref_var), &ref_range) in block_refinements.iter() {
            if ref_block == pred_id {
                propagated.push((block_id, ref_var, ref_range));
            }
        }
    }
    let mut count = 0;
    for (bid, var, range) in propagated {
        if let std::collections::hash_map::Entry::Vacant(entry) =
            block_refinements.entry((bid, var))
        {
            entry.insert(range);
            count += 1;
        }
    }
    if count > 0 {
        tracing::debug!(count, "propagated block refinements through jump chains");
    }
}
