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

mod helpers;
mod narrowing;
mod terminator;
mod widen;

use ori_arc::graph::{compute_postorder, compute_predecessors};
use ori_arc::ir::{ArcBlock, ArcFunction};
use ori_arc::{ArcBlockId, ArcVarId};
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use super::field_summary::{
    update_element_summaries, update_element_summaries_from_terminator, update_field_summaries,
    ElementSummaryTable, FieldSummaryTable,
};
use super::transfer::{transfer, TransferContext};
use super::{RangeAnalysisConfig, ValueRange};
use helpers::{
    collect_comparison_thresholds, merge_block_params, propagate_refinements_through_jump_chains,
};
use narrowing::{
    recompute_element_summaries, recompute_field_summaries, recompute_return_range,
    run_narrowing_pass,
};
use terminator::process_terminator;
use widen::update_range;
use ValueRange::{Bottom, Top};

pub use widen::{narrow, widen, widen_with_thresholds};

/// Result of range analysis for a single function.
#[derive(Debug)]
pub struct RangeFixpointResult {
    /// Per-variable ranges within this function.
    pub var_ranges: FxHashMap<ArcVarId, ValueRange>,
    /// Field-level range summaries from `Construct` instructions.
    pub field_summaries: FieldSummaryTable,
    /// Element-level range summaries from collection construction/reuse sites.
    pub element_summaries: ElementSummaryTable,
    /// Join of all `Return` terminator value ranges (for interprocedural propagation).
    pub return_range: ValueRange,
    /// Conditional refinements at block entries (from Branch/Switch terminators).
    ///
    /// Keyed by `(block_id, var)` — the range a variable is known to have
    /// at entry to that block. Used by `collect_param_ranges()` (TPR-03-037)
    /// to compute block-local argument ranges at call sites.
    pub block_refinements: FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
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
    element_summary_table: ElementSummaryTable,
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

        // Propagate refinements from single-predecessor parents to this block.
        // The AIMS pipeline may split the loop body (e.g., bb3 → bb4 → bb5),
        // so the branch refinement targets bb4 but the actual body is in bb5.
        // Without inline propagation, bb5 doesn't see the refinement during
        // the forward iteration, causing body computations to use the widened
        // phi range instead of the refined range. This makes i+1 overshoot
        // the comparison threshold, preventing convergence.
        if predecessors[block_idx].len() == 1 {
            let pred_idx = predecessors[block_idx][0];
            let pred_id = func.blocks[pred_idx].id;
            let inherited: Vec<_> = state
                .block_refinements
                .iter()
                .filter(|&(&(rb, rv), _)| {
                    rb == pred_id && !state.block_refinements.contains_key(&(block.id, rv))
                })
                .map(|(&(_, rv), &range)| (block.id, rv, range))
                .collect();
            for (bid, var, range) in inherited {
                state.block_refinements.insert((bid, var), range);
            }
        }

        let saved = apply_block_refinements(block, &mut state.ranges, &state.block_refinements);

        for instr in &block.body {
            update_field_summaries(
                instr,
                &state.ranges,
                &func.var_types,
                pool,
                &mut state.field_summary_table,
            );
            update_element_summaries(
                instr,
                &state.ranges,
                &func.var_types,
                pool,
                &mut state.element_summary_table,
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
                // Body variables are SSA: defined exactly once per block. Their
                // range is fully determined by the transfer function — no need to
                // join with prior iterations or apply widening. Using join+widening
                // on body variables (especially copies of loop counter phis like
                // `%6 = %4`) causes spurious widening that poisons the back-edge
                // contribution, preventing loop counter convergence.
                //
                // Only block parameters (phi nodes) need join+widening, which is
                // handled by merge_block_params() above.
                let old = state.ranges.get(&var).copied().unwrap_or(Bottom);
                if new_range != old {
                    tracing::trace!(var = var.index(), ?old, ?new_range, "body var updated");
                    state.ranges.insert(var, new_range);
                    changed = true;
                }
            }
        }

        restore_block_refinements(&mut state.ranges, saved);

        // BUG-05-001: check terminators for Invoke calls returning collections.
        update_element_summaries_from_terminator(
            &block.terminator,
            pool,
            &mut state.element_summary_table,
        );

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
    recompute_element_summaries(
        rpo,
        func,
        pool,
        &state.ranges,
        &mut state.element_summary_table,
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
        element_summaries: std::mem::take(&mut state.element_summary_table),
        return_range,
        block_refinements: state.block_refinements.clone(),
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
/// interprocedural propagation: call-site argument ranges (collected by interprocedural analysis)
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
            element_summaries: ElementSummaryTable::new(),
            return_range: Top,
            block_refinements: FxHashMap::default(),
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
        element_summary_table: ElementSummaryTable::new(),
        block_refinements: FxHashMap::default(),
        return_range: Bottom,
    };

    // Seed entry block parameters from interprocedural constraints if provided.
    // This is the key mechanism for TPR-03-026: when interprocedural analysis collects call-site
    // argument ranges and passes them here, the fixpoint starts with tighter
    // initial bounds instead of Bottom, enabling transitive propagation.
    if let Some(seeds) = initial_param_ranges {
        for (var, &range) in seeds {
            state.ranges.insert(*var, range);
        }
    }

    let empty_narrowings = FxHashMap::default();
    let crn = call_result_narrowings.unwrap_or(&empty_narrowings);

    // Thresholds for guided widening, populated after iteration 0.
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
