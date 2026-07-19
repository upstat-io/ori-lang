//! Recursive SCC processing for interprocedural range propagation.

use ori_arc::graph::call_graph::CallGraph;
use ori_arc::ir::ArcFunction;
use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::{collect_param_ranges, FunctionRangeInfo};
use crate::plan::ReprPlan;
use crate::range::fixpoint::range_fixpoint;
use crate::range::{RangeAnalysisConfig, ValueRange};

/// Result of solving one genuinely recursive call-graph component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecursiveSccOutcome {
    /// Number of fixpoint iterations performed.
    pub iterations: usize,
    /// Whether the per-component convergence limit forced a conservative fallback.
    pub exhausted: bool,
}

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

/// Process a recursive SCC until its parameter summaries stabilize.
///
/// A component that does not converge within its per-component limit is
/// replaced with conservative `Top` summaries. Non-recursive components never
/// enter this routine and therefore never consume a recursive convergence
/// budget.
#[expect(
    clippy::too_many_arguments,
    reason = "plan parameter required for unconstrained function detection in collect_param_ranges"
)]
pub(super) fn process_recursive_scc(
    scc: &ori_arc::graph::scc::Scc,
    call_graph: &CallGraph,
    func_map: &FxHashMap<Name, &ArcFunction>,
    pool: &Pool,
    config: &RangeAnalysisConfig,
    results: &mut FxHashMap<Name, crate::range::fixpoint::RangeFixpointResult>,
    func_infos: &mut FxHashMap<Name, FunctionRangeInfo>,
    plan: &ReprPlan,
) -> RecursiveSccOutcome {
    // Initialize all members with Bottom params.
    for name in &scc.members {
        if let Some(func) = func_map.get(name) {
            func_infos.insert(*name, FunctionRangeInfo::new_bottom(func.params.len()));
        }
    }

    let mut iteration = 0;
    loop {
        if iteration >= config.max_scc_iterations {
            tracing::debug!(
                scc_size = scc.members.len(),
                iterations = iteration,
                members = ?scc.members,
                "recursive range SCC did not converge; replacing its summaries with Top"
            );
            // Widen all parameter ranges to Top AND clear stale intermediate
            // results. Without clearing `results`, Phase 4 would
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
                        element_summaries: crate::range::ElementSummaryTable::new(),
                        return_range: ValueRange::Top,
                        block_refinements: FxHashMap::default(),
                    },
                );
            }
            return RecursiveSccOutcome {
                iterations: iteration,
                exhausted: true,
            };
        }

        let mut changed = false;

        for name in &scc.members {
            let Some(func) = func_map.get(name) else {
                continue;
            };

            // Collect parameter ranges from all call sites (including within the SCC).
            let new_info = collect_param_ranges(func, results, func_map, call_graph, pool, plan);

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
            // This is the key fix: the fixpoint starts with
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
            return RecursiveSccOutcome {
                iterations: iteration,
                exhausted: false,
            };
        }
    }
}
