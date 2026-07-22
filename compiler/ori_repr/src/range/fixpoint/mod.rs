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
//!    projection refresh pass → recompute `return_range`
//!
//! # Phase
//!
//! Runs after ARC lowering, before LLVM codegen.
//! Results stored in `ReprPlan` via `set_var_ranges()` and `flush_to_repr_plan()`.

mod iteration;
mod narrowing;
mod terminator;
mod widen;

use core::fmt;
use ori_arc::graph::{compute_postorder, compute_predecessors};
use ori_arc::ir::ArcFunction;
use ori_arc::{ArcBlockId, ArcVarId};
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::field_summary::{
    update_element_summaries, update_element_summaries_from_terminator, update_field_summaries,
    ElementSummaryTable, FieldSummaryTable,
};
use super::transfer::{transfer, TransferContext};
use super::{RangeAnalysisConfig, ValueRange};
use iteration::{
    collect_comparison_thresholds, merge_block_params, propagate_refinements_through_jump_chains,
};
use narrowing::{
    recompute_element_summaries, recompute_field_summaries, recompute_return_range,
    refine_direct_range_inductions, run_narrowing_pass,
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
    /// at entry to that block. Used by `collect_param_ranges()`
    /// to compute block-local argument ranges at call sites.
    pub block_refinements: FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
}

/// Mutable state threaded through the fixpoint loop.
struct FixpointState {
    ranges: FxHashMap<ArcVarId, ValueRange>,
    direct_field_sources: FxHashMap<(ArcVarId, u32), ArcVarId>,
    field_summary_table: FieldSummaryTable,
    element_summary_table: ElementSummaryTable,
    block_refinements: FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    return_range: ValueRange,
}

/// Separates stable CFG/type inputs from mutable convergence state.
pub(super) struct FixpointContext<'a> {
    rpo: &'a [usize],
    func: &'a ArcFunction,
    pool: &'a Pool,
    predecessors: &'a [Vec<usize>],
    known_builtins: &'a super::KnownBuiltins,
    call_result_narrowings: &'a FxHashMap<ArcVarId, ValueRange>,
}

// Why: The context retains whole CFG and type tables; report identity and cardinalities only.
impl fmt::Debug for FixpointContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FixpointContext")
            .field("function", &self.func.name)
            .field("block_count", &self.func.blocks.len())
            .field("rpo_len", &self.rpo.len())
            .field(
                "call_result_narrowing_count",
                &self.call_result_narrowings.len(),
            )
            .finish()
    }
}

/// Run one forward iteration of the fixpoint loop over all blocks in RPO.
///
/// Returns `true` if any range changed during this iteration.
///
/// When `call_result_narrowings` is non-empty, Apply/Invoke dst variables
/// are narrowed (via `meet`) with the callee return range after the transfer
/// function runs. This enables derived locals downstream of call results
/// to propagate the callee return range through the fixpoint.
fn run_forward_iteration(
    context: &FixpointContext<'_>,
    state: &mut FixpointState,
    iteration: usize,
    thresholds: &[i64],
) -> bool {
    // Clear stale refinements from prior iterations.
    // Refinements are recomputed fresh each iteration from the current ranges,
    // preventing widened scrutinee ranges from preserving overly-tight
    // refinements computed in earlier iterations.
    state.block_refinements.clear();

    let mut changed = false;
    for &block_idx in context.rpo {
        let block = &context.func.blocks[block_idx];

        changed |= merge_block_params(
            context,
            block,
            block_idx,
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
        if context.predecessors[block_idx].len() == 1 {
            let pred_idx = context.predecessors[block_idx][0];
            let pred_id = context.func.blocks[pred_idx].id;
            let inherited: Vec<_> = state
                .block_refinements
                .iter()
                .filter(|&(&(rb, _), _)| rb == pred_id)
                .map(|(&(_, rv), &range)| (block.id, rv, range))
                .collect();
            for (bid, var, range) in inherited {
                state.block_refinements.entry((bid, var)).or_insert(range);
            }
        }

        let saved =
            iteration::apply_block_refinements(block, &mut state.ranges, &state.block_refinements);

        for instr in &block.body {
            update_field_summaries(
                instr,
                &state.ranges,
                &context.func.var_types,
                context.pool,
                &mut state.field_summary_table,
            );

            update_element_summaries(
                instr,
                &state.ranges,
                &context.func.var_types,
                context.pool,
                &mut state.element_summary_table,
            );

            let ctx = TransferContext {
                ranges: &state.ranges,
                pool: context.pool,
                var_types: &context.func.var_types,
                field_summaries: state.field_summary_table.as_map(),
                direct_field_sources: &state.direct_field_sources,
                known_builtins: context.known_builtins,
            };
            let mut new_range = transfer(instr, &ctx);
            if let Some(var) = instr.defined_var() {
                // Why: `Apply`/`Invoke` transfer yields `Top`, so callee ranges narrow results here.
                if let Some(&narrowing) = context.call_result_narrowings.get(&var) {
                    new_range = new_range.meet(narrowing);
                }
                // INVARIANT: Only block parameters join and widen; body variables are SSA definitions.
                let old = state.ranges.get(&var).copied().unwrap_or(Bottom);
                if new_range != old {
                    tracing::trace!(var = var.index(), ?old, ?new_range, "body var updated");
                    state.ranges.insert(var, new_range);
                    changed = true;
                }
            }
        }

        iteration::restore_block_refinements(&mut state.ranges, saved);

        // check terminators for Invoke calls returning collections.
        update_element_summaries_from_terminator(
            &block.terminator,
            context.pool,
            &mut state.element_summary_table,
        );

        changed |= process_terminator(
            context,
            block,
            &mut state.ranges,
            &mut state.block_refinements,
            &mut state.return_range,
            iteration,
            thresholds,
        );
    }
    changed
}

/// Post-fixpoint narrowing: propagate refinements, run narrowing passes,
/// recompute field summaries, and finalize return range.
fn run_post_fixpoint_narrowing(
    context: &FixpointContext<'_>,
    state: &mut FixpointState,
) -> RangeFixpointResult {
    propagate_refinements_through_jump_chains(
        context.func,
        context.predecessors,
        &mut state.block_refinements,
    );

    // Alternate structural induction recovery with ordinary transfer
    // narrowing until both monotone fact tables stabilize. Outer-loop body
    // facts can bound the start/step of a nested direct Range, so later rounds
    // intentionally consume earlier results without an arbitrary pass budget.
    loop {
        let ranges_before = state.ranges.clone();
        let refinements_before = state.block_refinements.clone();
        refine_direct_range_inductions(
            context.func,
            context.pool,
            &mut state.ranges,
            context.predecessors,
            &mut state.block_refinements,
        );
        propagate_refinements_through_jump_chains(
            context.func,
            context.predecessors,
            &mut state.block_refinements,
        );
        run_narrowing_pass(
            context,
            &mut state.ranges,
            &state.field_summary_table,
            &state.direct_field_sources,
            &state.block_refinements,
        );
        if state.ranges == ranges_before && state.block_refinements == refinements_before {
            break;
        }
    }

    recompute_field_summaries(
        context.rpo,
        context.func,
        context.pool,
        &state.ranges,
        &mut state.field_summary_table,
    );

    recompute_element_summaries(
        context.rpo,
        context.func,
        context.pool,
        &state.ranges,
        &mut state.element_summary_table,
    );

    // Final narrowing pass with recomputed field summaries.
    run_narrowing_pass(
        context,
        &mut state.ranges,
        &state.field_summary_table,
        &state.direct_field_sources,
        &state.block_refinements,
    );

    // Recompute return_range from final narrowed ranges.
    let return_range =
        recompute_return_range(context.rpo, context.func, context.pool, &state.ranges);

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
/// function runs, enabling derived locals to propagate.
#[tracing::instrument(skip_all)]
pub(crate) fn range_fixpoint(
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
    let direct_field_sources = func
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter_map(|instr| match instr {
            ori_arc::ir::ArcInstr::Construct { dst, ty, args, .. }
                if pool.tag(pool.resolve_fully(*ty)) == ori_types::Tag::Range =>
            {
                Some(
                    args.iter()
                        .enumerate()
                        .filter_map(|(field, source)| {
                            u32::try_from(field)
                                .ok()
                                .map(|field| ((*dst, field), *source))
                        })
                        .collect::<Vec<_>>(),
                )
            }
            _ => None,
        })
        .flatten()
        .collect();

    let mut state = FixpointState {
        ranges: FxHashMap::default(),
        direct_field_sources,
        field_summary_table: FieldSummaryTable::new(),
        element_summary_table: ElementSummaryTable::new(),
        block_refinements: FxHashMap::default(),
        return_range: Bottom,
    };

    // Seed entry block parameters from interprocedural constraints if provided.
    // This is the key mechanism for when interprocedural analysis collects call-site
    // argument ranges and passes them here, the fixpoint starts with tighter
    // initial bounds instead of Bottom, enabling transitive propagation.
    if let Some(seeds) = initial_param_ranges {
        for (var, &range) in seeds {
            state.ranges.insert(*var, range);
        }
    }

    let empty_narrowings = FxHashMap::default();
    let crn = call_result_narrowings.unwrap_or(&empty_narrowings);
    let context = FixpointContext {
        rpo: &rpo,
        func,
        pool,
        predecessors: &predecessors,
        known_builtins: &config.known_builtins,
        call_result_narrowings: crn,
    };

    // Thresholds for guided widening, populated after iteration 0.
    let mut thresholds: Vec<i64> = Vec::new();

    let mut iteration = 0;
    loop {
        let changed = run_forward_iteration(&context, &mut state, iteration, &thresholds);

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

    run_post_fixpoint_narrowing(&context, &mut state)
}

#[cfg(test)]
mod tests;
