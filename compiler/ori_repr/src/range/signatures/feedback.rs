//! Return-range feedback pass (TPR-03-030, iterated per TPR-03-031).
//!
//! Phase 3 processes callers before callees (reverse topo for parameter
//! propagation). This means callers' `var_ranges` don't reflect callees'
//! return ranges. This module feeds callee return ranges back into
//! callers' results, then re-collects parameter ranges and re-runs
//! fixpoints for functions whose seeds changed.
//!
//! The feedback loop iterates until convergence (no changes) or
//! `config.max_feedback_iterations` is reached, enabling multi-hop
//! return-range chains to propagate through arbitrary-depth call chains.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator};
use ori_arc::ArcVarId;
use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::{build_param_seed_map, collect_param_ranges, FunctionRangeInfo};
use crate::range::fixpoint::range_fixpoint;
use crate::range::{RangeAnalysisConfig, ValueRange};

/// Feed callee return ranges back into callers' `results` and re-process
/// downstream functions whose parameter seeds changed.
///
/// Iterates to a fixpoint:
///
/// - Step 1: narrows caller call-result `dst` vars from callee `return_range`
/// - Step 1b: recomputes `func_infos` return ranges from updated var ranges
/// - Step 2: re-collects parameter ranges (reverse topo: callers first)
///   and re-runs seeded fixpoint for changed functions
///
/// Each iteration pushes return-range information one hop deeper through the
/// call graph. Bounded by `config.max_feedback_iterations` (default 5).
pub(super) fn feed_return_ranges_and_reprocess(
    sccs: &[ori_arc::graph::scc::Scc],
    func_map: &FxHashMap<Name, &ArcFunction>,
    pool: &Pool,
    config: &RangeAnalysisConfig,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &mut FxHashMap<Name, FunctionRangeInfo>,
) {
    for iteration in 0..config.max_feedback_iterations {
        // Step 1: Narrow caller dst vars from callee return ranges.
        let step1_changed = inject_callee_return_ranges(func_map, results, func_infos);

        if !step1_changed {
            tracing::debug!(iteration, "feedback converged — no dst vars changed");
            break;
        }

        // Step 1b: Recompute return ranges for functions whose return var
        // was narrowed by Step 1. Without this, the next iteration's Step 1
        // can't propagate the narrowed return range further up the chain.
        refresh_return_ranges(func_map, results, func_infos);

        // Step 2: Re-collect params and re-run fixpoint for affected functions.
        // Reverse topological order (callers first) so parameter propagation
        // cascades from callers to callees in a single pass.
        let step2_changed =
            reprocess_changed_functions(sccs, func_map, pool, config, results, func_infos);

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

/// Step 1: Narrow caller `dst` vars from callee return ranges in `results`.
///
/// Returns `true` if any `dst` variable was narrowed.
fn inject_callee_return_ranges(
    func_map: &FxHashMap<Name, &ArcFunction>,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &FxHashMap<Name, FunctionRangeInfo>,
) -> bool {
    let mut any_changed = false;
    for caller_func in func_map.values() {
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
                    any_changed = true;
                }
            }
        }
    }
    any_changed
}

/// Step 1b: Recompute `func_infos` return ranges from the updated `results`.
///
/// After Step 1 narrows `dst` vars, the var that a function returns via
/// `Return { value }` may have been narrowed. This recomputes each function's
/// `return_range` so the next outer iteration can propagate it further.
fn refresh_return_ranges(
    func_map: &FxHashMap<Name, &ArcFunction>,
    results: &FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &mut FxHashMap<Name, FunctionRangeInfo>,
) {
    for func in func_map.values() {
        let Some(result) = results.get(&func.name) else {
            continue;
        };
        let Some(info) = func_infos.get_mut(&func.name) else {
            continue;
        };
        // Compute return range from all Return terminators in the function.
        let mut ret_range = ValueRange::Bottom;
        for block in &func.blocks {
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

/// Step 2: Re-collect parameter ranges and re-run fixpoint for functions
/// whose parameter seeds changed. Reverse topological order (callers first)
/// so parameter propagation cascades from callers to callees in one pass.
///
/// Returns `true` if any function's results were updated.
fn reprocess_changed_functions(
    sccs: &[ori_arc::graph::scc::Scc],
    func_map: &FxHashMap<Name, &ArcFunction>,
    pool: &Pool,
    config: &RangeAnalysisConfig,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &mut FxHashMap<Name, FunctionRangeInfo>,
) -> bool {
    let mut any_changed = false;
    // Reverse iteration: callers first → callees last, so re-computed caller
    // results are available when we collect callee parameter ranges.
    for scc in sccs.iter().rev() {
        for name in &scc.members {
            let Some(func) = func_map.get(name) else {
                continue;
            };
            let info = collect_param_ranges(func, results, func_infos, func_map, pool);
            let params_changed = func_infos
                .get(name)
                .is_none_or(|old| old.param_ranges != info.param_ranges);
            if params_changed {
                let seeds = build_param_seed_map(func, &info);
                let result = range_fixpoint(func, pool, config, Some(&seeds));
                func_infos.insert(
                    *name,
                    FunctionRangeInfo {
                        param_ranges: info.param_ranges,
                        return_range: result.return_range,
                    },
                );
                results.insert(*name, result);
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
