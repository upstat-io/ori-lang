//! Return-range feedback pass (TPR-03-030).
//!
//! Phase 3 processes callers before callees (reverse topo for parameter
//! propagation). This means callers' `var_ranges` don't reflect callees'
//! return ranges. This module feeds callee return ranges back into
//! callers' results, then re-collects parameter ranges and re-runs
//! fixpoints for functions whose seeds changed.

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
/// 1. Narrows each caller's call-result `dst` var from the callee's `return_range`
/// 2. Re-collects parameter ranges for functions whose callers changed
/// 3. Re-runs seeded fixpoint for functions with new parameter seeds
pub(super) fn feed_return_ranges_and_reprocess(
    sccs: &[ori_arc::graph::scc::Scc],
    func_map: &FxHashMap<Name, &ArcFunction>,
    pool: &Pool,
    config: &RangeAnalysisConfig,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &mut FxHashMap<Name, FunctionRangeInfo>,
) {
    // Step 1: Narrow caller dst vars from callee return ranges.
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

    if !any_changed {
        return;
    }

    // Step 2: Re-collect params and re-run fixpoint for affected functions.
    // Forward topological order (callees last) so re-computed results propagate.
    for scc in sccs {
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
            }
        }
    }
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
