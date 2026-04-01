//! Helper functions for fixpoint iteration infrastructure.

use ori_arc::ir::{ArcFunction, ArcTerminator};
use ori_arc::{ArcBlockId, ArcVarId};
use rustc_hash::FxHashMap;

use super::ValueRange;
use ValueRange::Bottom;

/// Collect integer constants used in comparison operations as widening thresholds.
///
/// Scans the function for comparison `PrimOps` and extracts constant operand
/// values. These serve as "landmarks" that prevent widening from jumping
/// directly to ±MAX. For example, `i >= 10` yields threshold 10, causing
/// widening to produce `[0, 10]` instead of `[0, MAX]`.
///
/// Also includes ±1 neighbors since `i < N` uses N and `i >= N` uses N-1.
pub(super) fn collect_comparison_thresholds(
    func: &ArcFunction,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
) -> Vec<i64> {
    use ori_arc::ir::{ArcInstr, ArcValue, PrimOp};
    use ori_ir::BinaryOp;

    let mut thresholds = Vec::new();
    // Always include 0 — loop counters commonly start at 0
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
                    // Extract constant operand values from their ranges.
                    // Only use the exact constant — NOT ±1 neighbors.
                    // Neighbors cause infinite widening growth when the loop
                    // counter increments by 1 each iteration.
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
#[expect(
    clippy::too_many_arguments,
    reason = "fixpoint infrastructure passes — bundling would add indirection"
)]
pub(super) fn merge_block_params(
    block: &ori_arc::ir::ArcBlock,
    block_idx: usize,
    predecessors: &[Vec<usize>],
    func: &ArcFunction,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    iteration: usize,
    thresholds: &[i64],
) -> bool {
    use super::widen::update_range;

    let mut changed = false;
    for (param_idx, (param_var, _param_ty)) in block.params.iter().enumerate() {
        let mut merged = Bottom;
        for &pred_idx in &predecessors[block_idx] {
            let pred = &func.blocks[pred_idx];
            if let ArcTerminator::Jump { target, args, .. } = &pred.terminator {
                if target.index() == block_idx {
                    if let Some(&arg_var) = args.get(param_idx) {
                        let arg_range = ranges.get(&arg_var).copied().unwrap_or(Bottom);
                        merged = merged.join(arg_range);
                    }
                }
            }
        }
        // Apply conditional refinements from Branch/Switch.
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
            if ref_block == pred_id && !block_refinements.contains_key(&(block_id, ref_var)) {
                propagated.push((block_id, ref_var, ref_range));
            }
        }
    }
    let count = propagated.len();
    for (bid, var, range) in propagated {
        block_refinements.insert((bid, var), range);
    }
    if count > 0 {
        tracing::debug!(count, "propagated block refinements through jump chains");
    }
}
