//! Return-range feedback across the call graph.
//!
//! Callers precede callees in reverse topological order, so their initial
//! variable ranges lack callee return ranges. Feedback narrows call results,
//! refreshes return ranges, and reruns fixpoints whose seeds changed.
//!
//! Iteration continues to convergence or `config.max_feedback_iterations`.
//!
//! ## Derived-local propagation
//!
//! `call_result_narrowings` meets each callee return range into its caller
//! destination before transfer resumes, propagating bounds to derived locals.

use ori_arc::graph::compute_postorder;
use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator};
use ori_arc::ArcVarId;
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::range::fixpoint::range_fixpoint;
use crate::range::ValueRange;

use super::analysis_context::{RangePropagationContext, RangePropagationState};
use super::{build_param_seed_map, collect_param_ranges, FunctionRangeInfo};

/// Feeds callee return ranges into callers and reprocesses affected functions.
///
/// Each iteration narrows call-result variables, refreshes return summaries,
/// and reruns seeded fixpoints in caller-first order. Iteration stops at
/// convergence or `config.max_feedback_iterations`.
pub(super) fn feed_return_ranges_and_reprocess(
    context: RangePropagationContext<'_>,
    mut state: RangePropagationState<'_>,
    exhausted_functions: &FxHashSet<Name>,
) {
    // INVARIANT: Reruns retain every accumulated narrowing to prevent oscillation.
    let mut accumulated_narrowings: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>> =
        FxHashMap::default();

    for iteration in 0..context.config.max_feedback_iterations {
        let new_narrowings = inject_callee_return_ranges(
            context.func_map,
            state.results,
            state.func_infos,
            exhausted_functions,
        );

        if new_narrowings.is_empty() {
            tracing::debug!(iteration, "feedback converged — no dst vars changed");
            break;
        }

        let new_narrowed: FxHashSet<Name> = new_narrowings.keys().copied().collect();
        for (name, vars) in new_narrowings {
            let entry = accumulated_narrowings.entry(name).or_default();
            for (var, range) in vars {
                entry
                    .entry(var)
                    .and_modify(|existing| *existing = existing.meet(range))
                    .or_insert(range);
            }
        }

        refresh_return_ranges(
            context.func_map,
            state.results,
            state.func_infos,
            exhausted_functions,
        );

        let step2_changed = reprocess_changed_functions(
            context,
            &mut state,
            &new_narrowed,
            &accumulated_narrowings,
            exhausted_functions,
        );

        if !step2_changed {
            tracing::debug!(
                iteration,
                "feedback converged — no function results changed"
            );
            break;
        }

        tracing::debug!(iteration, "feedback iteration complete — changes detected");
    }
}

/// Narrows caller destinations from callee return ranges in `results`.
///
/// Returns a map from caller name to per-variable narrowings (callee return
/// ranges for each call-result variable) for fixpoint propagation.
fn inject_callee_return_ranges(
    func_map: &FxHashMap<Name, &ArcFunction>,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &FxHashMap<Name, FunctionRangeInfo>,
    exhausted_functions: &FxHashSet<Name>,
) -> FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>> {
    let mut caller_narrowings: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>> =
        FxHashMap::default();
    for caller_func in func_map.values() {
        if exhausted_functions.contains(&caller_func.name) {
            continue;
        }
        let Some(cr) = results.get_mut(&caller_func.name) else {
            continue;
        };
        for block in &caller_func.blocks {
            for (dst, callee) in call_sites_in_block(block) {
                let ret = func_infos
                    .get(&callee)
                    .map_or(ValueRange::Top, |i| i.return_range);
                if matches!(ret, ValueRange::Top) {
                    continue;
                }
                let old = cr.var_ranges.get(&dst).copied().unwrap_or(ValueRange::Top);
                let narrowed = old.meet(ret);
                if narrowed != old {
                    cr.var_ranges.insert(dst, narrowed);
                    caller_narrowings
                        .entry(caller_func.name)
                        .or_default()
                        .insert(dst, ret);
                }
            }
        }
    }
    caller_narrowings
}

/// Recomputes `func_infos` return ranges from updated `results`.
///
/// Only reachable blocks contribute; treating an unanalyzed unreachable value
/// as `Top` would erase a precise return bound.
fn refresh_return_ranges(
    func_map: &FxHashMap<Name, &ArcFunction>,
    results: &FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &mut FxHashMap<Name, FunctionRangeInfo>,
    exhausted_functions: &FxHashSet<Name>,
) {
    for func in func_map.values() {
        if exhausted_functions.contains(&func.name) {
            continue;
        }
        let Some(result) = results.get(&func.name) else {
            continue;
        };
        let Some(info) = func_infos.get_mut(&func.name) else {
            continue;
        };
        // Compute RPO (reachable blocks only) — matches `range_fixpoint()`.
        let rpo = {
            let mut po = compute_postorder(func);
            po.reverse();
            po
        };
        // Compute return range from reachable Return terminators only.
        let mut ret_range = ValueRange::Bottom;
        for &block_idx in &rpo {
            let block = &func.blocks[block_idx];
            if let ArcTerminator::Return { value } = &block.terminator {
                let var_range = result
                    .var_ranges
                    .get(value)
                    .copied()
                    .unwrap_or(ValueRange::Top);
                ret_range = ret_range.join(var_range);
            }
        }
        // Only narrow — never widen beyond what the fixpoint computed.
        if ret_range != ValueRange::Bottom {
            info.return_range = info.return_range.meet(ret_range);
        }
    }
}

/// Re-collects parameter ranges and re-runs fixpoint for functions
/// whose parameter seeds changed OR whose call-result variables were narrowed
/// by return-range propagation. Reverse topological order (callers first)
/// propagates parameter ranges from callers to callees in one pass.
///
/// `new_narrowed`: functions with NEW narrowings in this iteration (for
/// convergence — determines which functions to rerun).
/// `all_narrowings`: all accumulated narrowings retained by `range_fixpoint`.
///
/// Returns `true` if any function's results were updated.
fn reprocess_changed_functions(
    context: RangePropagationContext<'_>,
    state: &mut RangePropagationState<'_>,
    new_narrowed: &FxHashSet<Name>,
    all_narrowings: &FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>>,
    exhausted_functions: &FxHashSet<Name>,
) -> bool {
    let mut any_changed = false;

    // Reverse iteration: callers first → callees last, so re-computed caller
    // results are available when callee parameter ranges are collected.
    for scc in context.sccs.iter().rev() {
        for name in &scc.members {
            // A recursive SCC that exhausted its convergence limit was
            // deliberately reset to Top. A later return-feedback iteration
            // must not resurrect its partially converged summaries.
            if exhausted_functions.contains(name) {
                continue;
            }
            let Some(func) = context.func_map.get(name) else {
                continue;
            };
            let info = collect_param_ranges(
                func,
                state.results,
                context.func_map,
                context.call_graph,
                context.pool,
                context.plan,
            );
            let params_changed = state
                .func_infos
                .get(name)
                .is_none_or(|old| old.param_ranges != info.param_ranges);
            let has_new_narrowings = new_narrowed.contains(name);

            // Why: Narrowed call results affect derived locals even with stable parameter seeds.
            if params_changed || has_new_narrowings {
                let seeds = build_param_seed_map(func, &info);
                // INVARIANT: A rerun receives every accumulated call-result narrowing.
                let crn = all_narrowings.get(name);
                let result = range_fixpoint(func, context.pool, context.config, Some(&seeds), crn);
                state.func_infos.insert(
                    *name,
                    FunctionRangeInfo {
                        param_ranges: info.param_ranges,
                        return_range: result.return_range,
                    },
                );
                state.results.insert(*name, result);
                any_changed = true;
            }
        }
    }
    any_changed
}

/// Extract `(dst, callee_name)` pairs from all call sites in a block.
fn call_sites_in_block(block: &ori_arc::ir::ArcBlock) -> Vec<(ArcVarId, Name)> {
    let mut sites = Vec::new();
    for instr in &block.body {
        if let ArcInstr::Apply { dst, func, .. } = instr {
            sites.push((*dst, *func));
        }
    }
    if let ArcTerminator::Invoke { dst, func, .. } = &block.terminator {
        sites.push((*dst, *func));
    }
    sites
}
