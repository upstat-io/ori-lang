//! Recursive SCC processing for interprocedural range propagation.

use ori_arc::ir::ArcFunction;
use rustc_hash::FxHashMap;

use crate::range::fixpoint::range_fixpoint;
use crate::range::ValueRange;

use super::analysis_context::{RangePropagationContext, RangePropagationState};
use super::{collect_param_ranges, FunctionRangeInfo};

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
pub(super) fn process_recursive_scc(
    scc: &ori_arc::graph::scc::Scc,
    context: RangePropagationContext<'_>,
    state: &mut RangePropagationState<'_>,
) -> RecursiveSccOutcome {
    for name in &scc.members {
        if let Some(func) = context.func_map.get(name) {
            state
                .func_infos
                .insert(*name, FunctionRangeInfo::new_bottom(func.params.len()));
        }
    }

    let mut iteration = 0;
    loop {
        if iteration >= context.config.max_scc_iterations {
            tracing::debug!(
                scc_size = scc.members.len(),
                iterations = iteration,
                members = ?scc.members,
                "recursive range SCC did not converge; replacing its summaries with Top"
            );
            for name in &scc.members {
                if let Some(func) = context.func_map.get(name) {
                    state
                        .func_infos
                        .insert(*name, FunctionRangeInfo::new_top(func.params.len()));
                }
                state.results.insert(
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
            let Some(func) = context.func_map.get(name) else {
                continue;
            };

            let new_info = collect_param_ranges(
                func,
                state.results,
                context.func_map,
                context.call_graph,
                context.pool,
                context.plan,
            );

            if let Some(old_info) = state.func_infos.get(name) {
                if old_info.param_ranges != new_info.param_ranges {
                    changed = true;
                }
            } else {
                changed = true;
            }

            state.func_infos.insert(*name, new_info.clone());

            let seeds = build_param_seed_map(func, &new_info);
            let result = range_fixpoint(func, context.pool, context.config, Some(&seeds), None);
            state.results.insert(*name, result);
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
