//! Recursive SCC processing for interprocedural range propagation.
//!
//! Extracted from `mod.rs` to keep file sizes under the 500-line limit.

use ori_arc::ir::ArcFunction;
use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::{collect_param_ranges, FunctionRangeInfo};
use crate::plan::ReprPlan;
use crate::range::fixpoint::range_fixpoint;
use crate::range::{RangeAnalysisConfig, ValueRange};

/// Build a seed map from `FunctionRangeInfo::param_ranges` for `range_fixpoint()`.
///
/// Maps each function parameter's `ArcVarId` to its interprocedural range
/// (from call-site analysis). The fixpoint loop uses this to initialize
/// entry block parameter variables instead of starting at `Bottom`.
pub(super) fn build_param_seed_map(
    func: &ArcFunction,
    info: &FunctionRangeInfo,
) -> FxHashMap<ori_arc::ArcVarId, ValueRange> {
    let mut seeds = FxHashMap::default();
    for pr in &info.param_ranges {
        if pr.param_index < func.params.len() {
            seeds.insert(func.params[pr.param_index].var, pr.range);
        }
    }
    seeds
}

/// Process a recursive SCC: iterate fixpoint until parameter + return ranges stabilize.
///
/// Returns the number of SCC iterations consumed.
#[expect(
    clippy::too_many_arguments,
    reason = "plan parameter required for unconstrained function detection in collect_param_ranges"
)]
pub(super) fn process_recursive_scc(
    scc: &ori_arc::graph::scc::Scc,
    func_map: &FxHashMap<Name, &ArcFunction>,
    pool: &Pool,
    config: &RangeAnalysisConfig,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &mut FxHashMap<Name, FunctionRangeInfo>,
    remaining_budget: usize,
    plan: &ReprPlan,
) -> usize {
    // TPR-03-035: Use the minimum of the per-SCC cap and the remaining total
    // budget. Without this, one recursive SCC can overshoot max_total_scc_iterations.
    let effective_cap = config.max_scc_iterations.min(remaining_budget);

    // Initialize all members with Bottom params.
    for name in &scc.members {
        if let Some(func) = func_map.get(name) {
            func_infos.insert(*name, FunctionRangeInfo::new_bottom(func.params.len()));
        }
    }

    let mut iteration = 0;
    loop {
        if iteration >= effective_cap {
            tracing::warn!(
                scc_size = scc.members.len(),
                iterations = iteration,
                "SCC fixpoint did not converge — widening to Top"
            );
            // Widen all parameter ranges to Top AND clear stale intermediate
            // results (TPR-03-028). Without clearing `results`, Phase 4 would
            // persist partially-converged var_ranges from the last iteration.
            for name in &scc.members {
                if let Some(func) = func_map.get(name) {
                    func_infos.insert(*name, FunctionRangeInfo::new_top(func.params.len()));
                }
                results.insert(
                    *name,
                    crate::range::fixpoint::RangeFixpointResult {
                        var_ranges: FxHashMap::default(),
                        field_summaries: crate::range::FieldSummaryTable::new(),
                        return_range: ValueRange::Top,
                        block_refinements: FxHashMap::default(),
                    },
                );
            }
            break;
        }

        let mut changed = false;

        for name in &scc.members {
            let Some(func) = func_map.get(name) else {
                continue;
            };

            // Collect parameter ranges from all call sites (including within the SCC).
            let new_info = collect_param_ranges(func, results, func_infos, func_map, pool, plan);

            // Check if parameter ranges changed.
            if let Some(old_info) = func_infos.get(name) {
                if old_info.param_ranges != new_info.param_ranges {
                    changed = true;
                }
            } else {
                changed = true;
            }

            func_infos.insert(*name, new_info.clone());

            // Re-run intraprocedural analysis with parameter seeds from call sites.
            // This is the key TPR-03-026 fix: the fixpoint starts with
            // interprocedural parameter constraints, enabling tighter results.
            let seeds = build_param_seed_map(func, &new_info);
            let result = range_fixpoint(func, pool, config, Some(&seeds), None);
            results.insert(*name, result);
        }

        iteration += 1;

        if !changed {
            tracing::debug!(
                scc_size = scc.members.len(),
                iterations = iteration,
                "SCC fixpoint converged"
            );
            break;
        }
    }

    iteration
}
