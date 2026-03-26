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
//! 4. After convergence, run one narrowing pass to recover precision
//!
//! # Phase
//!
//! Runs after ARC lowering, before LLVM codegen.
//! Results stored in `ReprPlan` via `set_var_ranges()` and `flush_to_repr_plan()`.

use ori_arc::graph::{compute_postorder, compute_predecessors};
use ori_arc::ir::{ArcBlock, ArcFunction, ArcTerminator};
use ori_arc::{ArcBlockId, ArcVarId};
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use super::conditional::refine_from_branch;
use super::field_summary::{update_field_summaries, FieldSummaryTable};
use super::transfer::{transfer, transfer_known_call, TransferContext};
use super::{is_int_typed, RangeAnalysisConfig, ValueRange};
use ValueRange::{Bottom, Bounded, Top};

/// Widening threshold — start widening after this many iterations.
const WIDEN_THRESHOLD: usize = 3;

/// Standard widening: if a bound grew since the previous iteration,
/// push it to infinity. This guarantees termination by ensuring the
/// ascending chain reaches `Top` in at most 2 widening steps per bound.
#[must_use]
pub fn widen(previous: ValueRange, current: ValueRange) -> ValueRange {
    match (previous, current) {
        (Bottom, x) => x,
        (_, Bottom) => Bottom,
        (Top, _) | (_, Top) => Top,
        (Bounded { lo: p_lo, hi: p_hi }, Bounded { lo: c_lo, hi: c_hi }) => {
            let new_lo = if c_lo < p_lo { i64::MIN } else { c_lo };
            let new_hi = if c_hi > p_hi { i64::MAX } else { c_hi };
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
) -> bool {
    let old = ranges.get(&var).copied().unwrap_or(Bottom);
    let merged = if iteration > WIDEN_THRESHOLD {
        widen(old, old.join(new_range))
    } else {
        old.join(new_range)
    };
    if merged == old {
        false
    } else {
        ranges.insert(var, merged);
        true
    }
}

/// Process block parameters (phi-like merging from predecessor `Jump` args).
fn merge_block_params(
    block: &ArcBlock,
    block_idx: usize,
    predecessors: &[Vec<usize>],
    func: &ArcFunction,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    iteration: usize,
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
        changed |= update_range(ranges, *param_var, merged, iteration);
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
fn apply_block_refinements(
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
fn restore_block_refinements(
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    saved: Vec<(ArcVarId, ValueRange)>,
) {
    for (var, original) in saved {
        ranges.insert(var, original);
    }
}

/// Process a block's terminator: `Invoke` defines a variable,
/// `Branch`/`Switch` produce refinements, `Return` accumulates return range.
fn process_terminator(
    block: &ArcBlock,
    func: &ArcFunction,
    pool: &Pool,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &mut FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    return_range: &mut ValueRange,
    iteration: usize,
) -> bool {
    let mut changed = false;
    match &block.terminator {
        ArcTerminator::Invoke {
            dst,
            ty,
            func: callee,
            ..
        } => {
            if is_int_typed(*ty, pool) {
                let new_range = transfer_known_call(*callee, pool).unwrap_or(Top);
                changed |= update_range(ranges, *dst, new_range, iteration);
            }
        }
        ArcTerminator::Branch {
            cond,
            then_block,
            else_block,
        } => {
            let refinements = refine_from_branch(*cond, ranges, &block.body);
            for r in &refinements {
                block_refinements.insert((*then_block, r.var), r.true_range);
                block_refinements.insert((*else_block, r.var), r.false_range);
            }
        }
        ArcTerminator::Switch {
            scrutinee,
            cases,
            default: _,
        } => {
            for &(case_val, case_block) in cases {
                if let Ok(val) = i64::try_from(case_val) {
                    block_refinements
                        .insert((case_block, *scrutinee), Bounded { lo: val, hi: val });
                }
            }
        }
        ArcTerminator::Return { value } => {
            if is_int_typed(func.return_type, pool) {
                let ret_range = ranges.get(value).copied().unwrap_or(Top);
                *return_range = return_range.join(ret_range);
            }
        }
        ArcTerminator::Jump { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
    changed
}

/// Run one narrowing pass over all blocks to recover precision lost to widening.
fn run_narrowing_pass(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    field_summary_table: &FieldSummaryTable,
) {
    for &block_idx in rpo {
        let block = &func.blocks[block_idx];
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
    }
}

/// Run range analysis fixpoint on a single function.
///
/// Computes `ValueRange` for every int-typed variable by iterating over
/// basic blocks in RPO until convergence (or `max_iterations` reached).
/// Uses widening to guarantee termination for loops, then one narrowing
/// pass to recover precision.
#[tracing::instrument(skip_all)]
pub fn range_fixpoint(
    func: &ArcFunction,
    pool: &Pool,
    config: &RangeAnalysisConfig,
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

    let mut ranges: FxHashMap<ArcVarId, ValueRange> = FxHashMap::default();
    let mut return_range = Bottom;
    let mut iteration = 0;

    let rpo = {
        let mut po = compute_postorder(func);
        po.reverse();
        po
    };

    let predecessors = compute_predecessors(func);
    let mut field_summary_table = FieldSummaryTable::new();
    let mut block_refinements: FxHashMap<(ArcBlockId, ArcVarId), ValueRange> = FxHashMap::default();

    loop {
        let mut changed = false;
        for &block_idx in &rpo {
            let block = &func.blocks[block_idx];

            changed |= merge_block_params(
                block,
                block_idx,
                &predecessors,
                func,
                &mut ranges,
                &block_refinements,
                iteration,
            );

            // Step 1.5: Temporarily apply refinements to non-parameter vars.
            let saved = apply_block_refinements(block, &mut ranges, &block_refinements);

            for instr in &block.body {
                update_field_summaries(
                    instr,
                    &ranges,
                    &func.var_types,
                    pool,
                    &mut field_summary_table,
                );
                let ctx = TransferContext {
                    ranges: &ranges,
                    pool,
                    var_types: &func.var_types,
                    field_summaries: field_summary_table.as_map(),
                };
                let new_range = transfer(instr, &ctx);
                if let Some(var) = instr.defined_var() {
                    changed |= update_range(&mut ranges, var, new_range, iteration);
                }
            }

            // Restore pre-refinement ranges for non-parameter variables.
            restore_block_refinements(&mut ranges, saved);

            changed |= process_terminator(
                block,
                func,
                pool,
                &mut ranges,
                &mut block_refinements,
                &mut return_range,
                iteration,
            );
        }

        iteration += 1;
        if !changed || iteration >= config.max_iterations {
            break;
        }
    }

    tracing::debug!(
        func = func.name.raw(),
        iterations = iteration,
        non_top = ranges.values().filter(|r| !matches!(r, Top)).count(),
        "range analysis complete"
    );

    run_narrowing_pass(&rpo, func, pool, &mut ranges, &field_summary_table);

    // Recompute field summaries from final (post-narrowing) ranges.
    // During the fixpoint loop, field summaries may accumulate wider
    // ranges from pre-convergence iterations. See TPR-03-016.
    field_summary_table.clear();
    for &block_idx in &rpo {
        for instr in &func.blocks[block_idx].body {
            update_field_summaries(
                instr,
                &ranges,
                &func.var_types,
                pool,
                &mut field_summary_table,
            );
        }
    }

    RangeFixpointResult {
        var_ranges: ranges,
        field_summaries: field_summary_table,
        return_range,
    }
}

#[cfg(test)]
mod tests;
