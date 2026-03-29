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
//!
//! ## TPR-03-032: Derived-local propagation
//!
//! When Step 1 narrows a caller's call-result `dst` variable, derived
//! locals (e.g., `let y = x + 1` where `x` is the narrowed dst) must
//! also be updated. This requires re-running the caller's intraprocedural
//! fixpoint with `call_result_narrowings` — a map of `(dst_var → callee
//! return range)` that is applied as `meet` after the Apply/Invoke
//! transfer function runs. This way the narrowed value propagates
//! naturally through the fixpoint to derived locals.

use ori_arc::graph::compute_postorder;
use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator};
use ori_arc::ArcVarId;
use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{build_param_seed_map, collect_param_ranges, FunctionRangeInfo};
use crate::range::fixpoint::range_fixpoint;
use crate::range::{RangeAnalysisConfig, ValueRange};

/// Feed callee return ranges back into callers' `results` and re-process
/// downstream functions whose parameter seeds or call-result vars changed.
///
/// Iterates to a fixpoint:
///
/// - Step 1: narrows caller call-result `dst` vars from callee `return_range`,
///   collecting per-caller narrowing maps for the fixpoint rerun
/// - Step 1b: recomputes `func_infos` return ranges from updated var ranges
/// - Step 2: re-collects parameter ranges (reverse topo: callers first)
///   and re-runs seeded fixpoint for functions with changed params or
///   narrowed call-result vars (TPR-03-032)
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
    plan: &crate::plan::ReprPlan,
) {
    // Accumulated narrowings across ALL feedback iterations. When Step 2
    // reruns a function's fixpoint, it must receive ALL callee return-range
    // narrowings discovered so far — not just the current iteration's.
    // Without this, a rerun in iteration N+1 for variable `v_b` would
    // overwrite `results[func]` and lose the narrowing for `v_a` that was
    // discovered in iteration N (oscillation bug).
    let mut accumulated_narrowings: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>> =
        FxHashMap::default();

    for iteration in 0..config.max_feedback_iterations {
        // Step 1: Narrow caller dst vars from callee return ranges.
        // Returns NEW per-caller narrowings for this iteration only.
        let new_narrowings = inject_callee_return_ranges(func_map, results, func_infos);

        if new_narrowings.is_empty() {
            tracing::debug!(iteration, "feedback converged — no dst vars changed");
            break;
        }

        // Merge new narrowings into the accumulated set (meet = intersection).
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

        // Step 1b: Recompute return ranges for functions whose return var
        // was narrowed by Step 1. Without this, the next iteration's Step 1
        // can't propagate the narrowed return range further up the chain.
        refresh_return_ranges(func_map, results, func_infos);

        // Step 2: Re-collect params and re-run fixpoint for affected functions.
        // Reverse topological order (callers first) so parameter propagation
        // cascades from callers to callees in a single pass.
        //
        // `new_narrowed` determines WHICH functions to rerun (convergence).
        // `accumulated_narrowings` provides ALL narrowings to the fixpoint
        // (correctness — prevents losing earlier iterations' narrowings).
        let step2_changed = reprocess_changed_functions(
            sccs,
            func_map,
            pool,
            config,
            results,
            func_infos,
            &new_narrowed,
            &accumulated_narrowings,
            plan,
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

/// Step 1: Narrow caller `dst` vars from callee return ranges in `results`.
///
/// Returns a map from caller name to per-variable narrowings (callee return
/// ranges for each call-result variable). These narrowings are passed to
/// `range_fixpoint` in Step 2 so derived locals propagate (TPR-03-032).
fn inject_callee_return_ranges(
    func_map: &FxHashMap<Name, &ArcFunction>,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &FxHashMap<Name, FunctionRangeInfo>,
) -> FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>> {
    let mut caller_narrowings: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>> =
        FxHashMap::default();
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
                    // Record narrowing for fixpoint rerun (TPR-03-032).
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

/// Step 1b: Recompute `func_infos` return ranges from the updated `results`.
///
/// After Step 1 narrows `dst` vars, the var that a function returns via
/// `Return { value }` may have been narrowed. This recomputes each function's
/// `return_range` so the next outer iteration can propagate it further.
///
/// TPR-03-033: Only reachable blocks are considered. Unreachable blocks
/// contain variables that were never analyzed by `range_fixpoint()`, so
/// `var_ranges.get()` returns `None` and the `unwrap_or(Top)` fallback
/// would pollute the return range — the same issue TPR-03-023 fixed in
/// `recompute_return_range()`.
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

/// Step 2: Re-collect parameter ranges and re-run fixpoint for functions
/// whose parameter seeds changed OR whose call-result variables were narrowed
/// by Step 1 (TPR-03-032). Reverse topological order (callers first)
/// so parameter propagation cascades from callers to callees in one pass.
///
/// `new_narrowed`: functions with NEW narrowings in this iteration (for
/// convergence — determines which functions to rerun).
/// `all_narrowings`: ALL accumulated narrowings across iterations (for
/// correctness — passed to `range_fixpoint` so previous narrowings survive).
///
/// Returns `true` if any function's results were updated.
#[expect(
    clippy::too_many_arguments,
    reason = "split new_narrowed/all_narrowings is load-bearing for correctness"
)]
fn reprocess_changed_functions(
    sccs: &[ori_arc::graph::scc::Scc],
    func_map: &FxHashMap<Name, &ArcFunction>,
    pool: &Pool,
    config: &RangeAnalysisConfig,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &mut FxHashMap<Name, FunctionRangeInfo>,
    new_narrowed: &FxHashSet<Name>,
    all_narrowings: &FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>>,
    plan: &crate::plan::ReprPlan,
) -> bool {
    let mut any_changed = false;

    // Reverse iteration: callers first → callees last, so re-computed caller
    // results are available when we collect callee parameter ranges.
    for scc in sccs.iter().rev() {
        for name in &scc.members {
            let Some(func) = func_map.get(name) else {
                continue;
            };
            let info = collect_param_ranges(func, results, func_infos, func_map, pool, plan);
            let params_changed = func_infos
                .get(name)
                .is_none_or(|old| old.param_ranges != info.param_ranges);
            let has_new_narrowings = new_narrowed.contains(name);

            // TPR-03-032: Rerun when call-result vars were narrowed, even if
            // params didn't change. The fixpoint will apply the narrowings to
            // produce correct derived-local ranges.
            if params_changed || has_new_narrowings {
                let seeds = build_param_seed_map(func, &info);
                // Pass ALL accumulated narrowings so previous iterations'
                // narrowings survive the results overwrite.
                let crn = all_narrowings.get(name);
                let result = range_fixpoint(func, pool, config, Some(&seeds), crn);
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
