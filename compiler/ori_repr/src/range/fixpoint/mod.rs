//! Fixed-point iteration for value range analysis.
//!
//! Implements the core fixpoint loop that drives range analysis over
//! `ArcFunction` basic blocks. Computes per-variable `ValueRange` for
//! every int-typed variable in a function by iterating until convergence.
//!
//! # Algorithm
//!
//! 1. Compute RPO block ordering (forward dataflow)
//! 2. For each block: merge block parameters, transfer body instructions,
//!    process terminator (refinements + invoke)
//! 3. Apply widening after `WIDEN_THRESHOLD` iterations to guarantee termination
//! 4. After convergence: 2 narrowing passes → recompute field summaries →
//!    projection refresh pass (TPR-03-022) → recompute `return_range` (TPR-03-021)
//!
//! # Phase
//!
//! Runs after ARC lowering, before LLVM codegen.
//! Results stored in `ReprPlan` via `set_var_ranges()` and `flush_to_repr_plan()`.

mod narrowing;
mod terminator;

use ori_arc::graph::{compute_postorder, compute_predecessors};
use ori_arc::ir::{ArcBlock, ArcFunction, ArcTerminator};
use ori_arc::{ArcBlockId, ArcVarId};
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use super::field_summary::{update_field_summaries, FieldSummaryTable};
use super::transfer::{transfer, TransferContext};
use super::{RangeAnalysisConfig, ValueRange};
use narrowing::{recompute_field_summaries, recompute_return_range, run_narrowing_pass};
use terminator::process_terminator;
use ValueRange::{Bottom, Bounded, Top};

/// Widening threshold — start widening after this many iterations.
const WIDEN_THRESHOLD: usize = 3;

/// Standard widening: if a bound grew since the previous iteration,
/// push it to infinity. This guarantees termination by ensuring the
/// ascending chain reaches `Top` in at most 2 widening steps per bound.
#[must_use]
/// Widen with optional comparison thresholds.
///
/// Standard VRP widening: when a bound is growing, jump to the NEXT threshold
/// instead of ±MAX. This ensures loop counters converge to their comparison
/// bounds (e.g., `i < 10` → widen to `[0, 10]` instead of `[0, MAX]`).
///
/// Without thresholds, falls back to the classic widening (jump to ±MAX).
pub fn widen_with_thresholds(
    previous: ValueRange,
    current: ValueRange,
    thresholds: &[i64],
) -> ValueRange {
    match (previous, current) {
        (Bottom, x) => x,
        (_, Bottom) => Bottom,
        (Top, _) | (_, Top) => Top,
        (Bounded { lo: p_lo, hi: p_hi }, Bounded { lo: c_lo, hi: c_hi }) => {
            let new_lo = if c_lo < p_lo {
                // Lower bound is shrinking — find the next threshold below c_lo
                thresholds
                    .iter()
                    .rev()
                    .find(|&&t| t <= c_lo)
                    .copied()
                    .unwrap_or(i64::MIN)
            } else {
                c_lo
            };
            let new_hi = if c_hi > p_hi {
                // Upper bound is growing — find the next threshold above c_hi
                thresholds
                    .iter()
                    .find(|&&t| t >= c_hi)
                    .copied()
                    .unwrap_or(i64::MAX)
            } else {
                c_hi
            };
            if new_lo == i64::MIN && new_hi == i64::MAX {
                Top
            } else {
                Bounded {
                    lo: new_lo,
                    hi: new_hi,
                }
            }
        }
    }
}

pub fn widen(previous: ValueRange, current: ValueRange) -> ValueRange {
    widen_with_thresholds(previous, current, &[])
}

/// Narrowing: intersect widened result with transfer function output
/// to recover precision lost during widening. Always tightens (or
/// preserves) the widened bound — never widens it.
#[must_use]
pub fn narrow(widened: ValueRange, computed: ValueRange) -> ValueRange {
    widened.meet(computed)
}

/// Result of range analysis for a single function.
#[derive(Debug)]
pub struct RangeFixpointResult {
    /// Per-variable ranges within this function.
    pub var_ranges: FxHashMap<ArcVarId, ValueRange>,
    /// Field-level range summaries from `Construct` instructions.
    pub field_summaries: FieldSummaryTable,
    /// Join of all `Return` terminator value ranges (for §03.5 interprocedural).
    pub return_range: ValueRange,
}

/// Merge or widen a variable's range, returning whether it changed.
fn update_range(
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    var: ArcVarId,
    new_range: ValueRange,
    iteration: usize,
    thresholds: &[i64],
) -> bool {
    let old = ranges.get(&var).copied().unwrap_or(Bottom);
    let merged = if iteration > WIDEN_THRESHOLD {
        widen_with_thresholds(old, old.join(new_range), thresholds)
    } else {
        old.join(new_range)
    };
    if merged == old {
        false
    } else {
        tracing::trace!(var = var.index(), ?old, ?merged, "range updated");
        ranges.insert(var, merged);
        true
    }
}

/// Collect integer constants used in comparison operations as widening thresholds.
///
/// Scans the function for comparison `PrimOps` and extracts constant operand
/// values. These serve as "landmarks" that prevent widening from jumping
/// directly to ±MAX. For example, `i >= 10` yields threshold 10, causing
/// widening to produce `[0, 10]` instead of `[0, MAX]`.
///
/// Also includes ±1 neighbors since `i < N` uses N and `i >= N` uses N-1.
fn collect_comparison_thresholds(
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
fn merge_block_params(
    block: &ArcBlock,
    block_idx: usize,
    predecessors: &[Vec<usize>],
    func: &ArcFunction,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    iteration: usize,
    thresholds: &[i64],
) -> bool {
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

/// Apply block-entry refinements to non-parameter variables as a temporary
/// overlay. Returns a vec of `(var, original_range)` pairs for later restoration.
///
/// Branch/Switch terminators produce refinements for specific variables in
/// successor blocks. Block parameters get refined during `merge_block_params`,
/// but non-parameter variables that are live across the branch also need
/// refinement during body processing. Since non-param variables share a
/// single global range entry, we apply the refinement temporarily and restore
/// afterward. See TPR-03-015.
pub(super) fn apply_block_refinements(
    block: &ArcBlock,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
) -> Vec<(ArcVarId, ValueRange)> {
    let mut saved: Vec<(ArcVarId, ValueRange)> = Vec::new();
    let param_vars: FxHashSet<ArcVarId> = block.params.iter().map(|(v, _)| *v).collect();

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

/// Restore ranges that were temporarily refined for a block.
pub(super) fn restore_block_refinements(
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    saved: Vec<(ArcVarId, ValueRange)>,
) {
    for (var, original) in saved {
        ranges.insert(var, original);
    }
}

/// Mutable state threaded through the fixpoint loop.
struct FixpointState {
    ranges: FxHashMap<ArcVarId, ValueRange>,
    field_summary_table: FieldSummaryTable,
    block_refinements: FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    return_range: ValueRange,
}

/// Run one forward iteration of the fixpoint loop over all blocks in RPO.
///
/// Returns `true` if any range changed during this iteration.
///
/// When `call_result_narrowings` is non-empty, Apply/Invoke dst variables
/// are narrowed (via `meet`) with the callee return range after the transfer
/// function runs. This enables derived locals downstream of call results
/// to propagate the callee return range through the fixpoint (TPR-03-032).
#[expect(
    clippy::too_many_arguments,
    reason = "fixpoint infrastructure — bundling would add indirection for one extra map ref"
)]
fn run_forward_iteration(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    predecessors: &[Vec<usize>],
    state: &mut FixpointState,
    iteration: usize,
    known_builtins: &super::KnownBuiltins,
    call_result_narrowings: &FxHashMap<ArcVarId, ValueRange>,
    thresholds: &[i64],
) -> bool {
    // TPR-03-020: Clear stale refinements from prior iterations.
    // Refinements are recomputed fresh each iteration from the current ranges,
    // preventing widened scrutinee ranges from preserving overly-tight
    // refinements computed in earlier iterations.
    state.block_refinements.clear();

    let mut changed = false;
    for &block_idx in rpo {
        let block = &func.blocks[block_idx];

        changed |= merge_block_params(
            block,
            block_idx,
            predecessors,
            func,
            &mut state.ranges,
            &state.block_refinements,
            iteration,
            thresholds,
        );

        let saved = apply_block_refinements(block, &mut state.ranges, &state.block_refinements);

        for instr in &block.body {
            update_field_summaries(
                instr,
                &state.ranges,
                &func.var_types,
                pool,
                &mut state.field_summary_table,
            );
            let ctx = TransferContext {
                ranges: &state.ranges,
                pool,
                var_types: &func.var_types,
                field_summaries: state.field_summary_table.as_map(),
                known_builtins,
            };
            let mut new_range = transfer(instr, &ctx);
            if let Some(var) = instr.defined_var() {
                // TPR-03-032: Apply callee return-range narrowing to call-result
                // variables. The transfer function for Apply/Invoke returns Top
                // (unknown function), but we have the callee's return range from
                // interprocedural analysis. Applying `meet` here lets the narrowed
                // value propagate to derived locals through subsequent iterations.
                if let Some(&narrowing) = call_result_narrowings.get(&var) {
                    new_range = new_range.meet(narrowing);
                }
                changed |= update_range(&mut state.ranges, var, new_range, iteration, thresholds);
            }
        }

        restore_block_refinements(&mut state.ranges, saved);

        changed |= process_terminator(
            block,
            func,
            pool,
            &mut state.ranges,
            &mut state.block_refinements,
            &mut state.return_range,
            iteration,
            known_builtins,
            call_result_narrowings,
            thresholds,
        );
    }
    changed
}

/// Post-fixpoint narrowing: propagate refinements, run narrowing passes,
/// recompute field summaries, and finalize return range.
fn run_post_fixpoint_narrowing(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    predecessors: &[Vec<usize>],
    state: &mut FixpointState,
    known_builtins: &super::KnownBuiltins,
    crn: &FxHashMap<ArcVarId, ValueRange>,
) -> RangeFixpointResult {
    propagate_refinements_through_jump_chains(func, predecessors, &mut state.block_refinements);

    // Run 2 narrowing passes: the second allows block-param ranges
    // (narrowed in pass 1 via body variables) to propagate back through
    // the predecessor merge. This recovers bounded loop variables.
    for _ in 0..2 {
        run_narrowing_pass(
            rpo,
            func,
            pool,
            &mut state.ranges,
            &state.field_summary_table,
            predecessors,
            &state.block_refinements,
            known_builtins,
            crn,
        );
    }

    recompute_field_summaries(
        rpo,
        func,
        pool,
        &state.ranges,
        &mut state.field_summary_table,
    );

    // TPR-03-022: Final narrowing pass with recomputed field summaries.
    run_narrowing_pass(
        rpo,
        func,
        pool,
        &mut state.ranges,
        &state.field_summary_table,
        predecessors,
        &state.block_refinements,
        known_builtins,
        crn,
    );

    // TPR-03-021: Recompute return_range from final narrowed ranges.
    let return_range = recompute_return_range(rpo, func, pool, &state.ranges);

    RangeFixpointResult {
        var_ranges: state.ranges.clone(),
        field_summaries: std::mem::take(&mut state.field_summary_table),
        return_range,
    }
}

/// Propagate block refinements through single-predecessor jump chains.
///
/// The ARC pipeline may split the loop body into multiple blocks:
/// `bb1 → Branch → bb4 (false, gets refinement) → Jump bb5 → body → Jump bb1`.
/// Without propagation, bb5 doesn't inherit bb4's refinement, so the narrowing
/// pass can't tighten loop-body variables.
fn propagate_refinements_through_jump_chains(
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

/// Run intraprocedural range analysis on a single function.
///
/// Computes `ValueRange` for every int-typed variable by iterating over
/// basic blocks in RPO until convergence (or `max_iterations` reached).
/// Uses widening to guarantee termination for loops, then narrowing
/// passes to recover precision.
///
/// When `initial_param_ranges` is `Some`, entry block parameters are seeded
/// from the provided map instead of starting at `Bottom`. This enables
/// interprocedural propagation: call-site argument ranges (collected by §03.5)
/// constrain the callee's parameters, yielding tighter results than the
/// standalone intraprocedural pass.
///
/// When `call_result_narrowings` is `Some`, Apply/Invoke dst variables
/// are narrowed (via `meet`) with callee return ranges after the transfer
/// function runs, enabling derived locals to propagate (TPR-03-032).
#[tracing::instrument(skip_all)]
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the only hasher used in range analysis"
)]
pub fn range_fixpoint(
    func: &ArcFunction,
    pool: &Pool,
    config: &RangeAnalysisConfig,
    initial_param_ranges: Option<&FxHashMap<ArcVarId, ValueRange>>,
    call_result_narrowings: Option<&FxHashMap<ArcVarId, ValueRange>>,
) -> RangeFixpointResult {
    if func.blocks.len() > config.max_blocks {
        tracing::warn!(
            func = func.name.raw(),
            blocks = func.blocks.len(),
            "skipping range analysis — function too large"
        );
        return RangeFixpointResult {
            var_ranges: FxHashMap::default(),
            field_summaries: FieldSummaryTable::new(),
            return_range: Top,
        };
    }

    let rpo = {
        let mut po = compute_postorder(func);
        po.reverse();
        po
    };
    let predecessors = compute_predecessors(func);
    let mut state = FixpointState {
        ranges: FxHashMap::default(),
        field_summary_table: FieldSummaryTable::new(),
        block_refinements: FxHashMap::default(),
        return_range: Bottom,
    };

    // Seed entry block parameters from interprocedural constraints if provided.
    // This is the key mechanism for TPR-03-026: when §03.5 collects call-site
    // argument ranges and passes them here, the fixpoint starts with tighter
    // initial bounds instead of Bottom, enabling transitive propagation.
    if let Some(seeds) = initial_param_ranges {
        for (var, &range) in seeds {
            state.ranges.insert(*var, range);
        }
    }

    let empty_narrowings = FxHashMap::default();
    let crn = call_result_narrowings.unwrap_or(&empty_narrowings);

    // §04.4: Thresholds for guided widening, populated after iteration 0.
    let mut thresholds: Vec<i64> = Vec::new();

    let mut iteration = 0;
    loop {
        let changed = run_forward_iteration(
            &rpo,
            func,
            pool,
            &predecessors,
            &mut state,
            iteration,
            &config.known_builtins,
            crn,
            &thresholds,
        );

        // After the first iteration, constant ranges are populated.
        // Collect comparison thresholds for guided widening before
        // widening triggers (at WIDEN_THRESHOLD).
        if iteration == 0 {
            thresholds = collect_comparison_thresholds(func, &state.ranges);
            if !thresholds.is_empty() {
                tracing::debug!(count = thresholds.len(), "collected widening thresholds");
            }
        }
        iteration += 1;
        if !changed || iteration >= config.max_iterations {
            break;
        }
    }

    tracing::debug!(
        func = func.name.raw(),
        iterations = iteration,
        non_top = state.ranges.values().filter(|r| !matches!(r, Top)).count(),
        "range analysis complete"
    );

    run_post_fixpoint_narrowing(
        &rpo,
        func,
        pool,
        &predecessors,
        &mut state,
        &config.known_builtins,
        crn,
    )
}

#[cfg(test)]
mod tests;
