//! Interprocedural range propagation via function signatures.
//!
//! After intraprocedural range analysis computes per-variable ranges
//! within each function, this module propagates ranges across function
//! boundaries: argument ranges at call sites narrow callee parameters, and
//! callee return ranges narrow the caller's call-result variable.
//!
//! # Algorithm
//!
//! 1. Build a call graph from the set of `ArcFunction`s.
//! 2. Decompose into SCCs (strongly connected components) via Tarjan's algorithm.
//! 3. Process SCCs in reverse topological order (callers first):
//!    - **Non-recursive SCC**: single pass — collect argument ranges at all call
//!      sites, join into parameter ranges.
//!    - **Recursive SCC**: iterate — re-run `range_fixpoint()` with parameter
//!      constraints until parameter ranges stabilize or the per-SCC limit is reached.
//! 4. Feed stabilized callee return ranges back into callers.
//! 5. Store results in `ReprPlan::function_var_ranges`.
//!
//! # Budget
//!
//! - Every non-recursive SCC is processed exactly once.
//! - `max_scc_iterations` (default 10) caps each recursive SCC independently.
//! - A recursive SCC that reaches its cap gets `Top` summaries (safe fallback).

mod analysis_context;
mod feedback;
mod scc;

use ori_arc::graph::call_graph::CallGraph;
use ori_arc::graph::scc::compute_sccs;
use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator};
use ori_arc::{ArcBlockId, ArcVarId};
use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::plan::ReprPlan;
use crate::range::fixpoint::range_fixpoint;

use super::{is_int_typed, RangeAnalysisConfig, ValueRange};
use analysis_context::{RangePropagationContext, RangePropagationState};

/// Reason a function's parameters are unconstrained (all params → Top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnconstrainedReason {
    /// Function is `pub` or a trait impl method (registered in `ReprPlan`).
    PublicOrTraitImpl,
    /// Function is a closure/lambda (has captures).
    Closure,
}

/// Per-parameter range summary from call-site analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamRange {
    /// Which parameter index (0-based).
    pub param_index: usize,
    /// Inferred range for this parameter across all call sites.
    pub range: ValueRange,
}

/// Per-function range summary for interprocedural propagation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRangeInfo {
    /// Ranges inferred for each parameter (from all call sites).
    pub param_ranges: Vec<ParamRange>,
    /// Range of the return value (from intraprocedural analysis).
    pub return_range: ValueRange,
}

impl FunctionRangeInfo {
    /// Create a new info with all parameters set to `Bottom` (no callers seen).
    fn new_bottom(num_params: usize) -> Self {
        Self {
            param_ranges: (0..num_params)
                .map(|i| ParamRange {
                    param_index: i,
                    range: ValueRange::Bottom,
                })
                .collect(),
            return_range: ValueRange::Bottom,
        }
    }

    /// Create a new info with all parameters set to `Top` (unconstrained).
    fn new_top(num_params: usize) -> Self {
        Self {
            param_ranges: (0..num_params)
                .map(|i| ParamRange {
                    param_index: i,
                    range: ValueRange::Top,
                })
                .collect(),
            return_range: ValueRange::Top,
        }
    }
}

/// Run interprocedural range propagation across all functions.
///
/// This is the interprocedural propagation entry point. It:
/// 1. Runs intraprocedural `range_fixpoint()` for each function.
/// 2. Builds a call graph and decomposes into SCCs.
/// 3. Propagates argument ranges to callee parameters within each SCC.
/// 4. Stores final results in `ReprPlan`.
#[tracing::instrument(skip_all)]
pub fn propagate_ranges(
    plan: &mut ReprPlan,
    pool: &Pool,
    arc_functions: &[ArcFunction],
    config: &RangeAnalysisConfig,
) {
    if arc_functions.is_empty() {
        return;
    }

    // Build function lookup by name for O(1) access.
    let func_map: FxHashMap<Name, &ArcFunction> =
        arc_functions.iter().map(|f| (f.name, f)).collect();

    let mut results: FxHashMap<Name, super::fixpoint::RangeFixpointResult> = FxHashMap::default();
    for func in arc_functions {
        let result = range_fixpoint(func, pool, config, None, None);
        results.insert(func.name, result);
    }

    let call_graph = CallGraph::build(arc_functions);
    let sccs = compute_sccs(&call_graph);

    // Why: Parameter seeds require completed caller ranges, while `compute_sccs` returns callees first.
    let mut func_infos: FxHashMap<Name, FunctionRangeInfo> = FxHashMap::default();
    let mut recursive_scc_iters: usize = 0;
    let mut exhausted_recursive_sccs: usize = 0;
    let mut exhausted_functions: FxHashSet<Name> = FxHashSet::default();

    let analysis = RangePropagationContext {
        sccs: &sccs,
        call_graph: &call_graph,
        func_map: &func_map,
        pool,
        config,
        plan,
    };

    for scc in sccs.iter().rev() {
        if scc.is_recursive(&call_graph) {
            let mut state = RangePropagationState {
                results: &mut results,
                func_infos: &mut func_infos,
            };
            let outcome = scc::process_recursive_scc(scc, analysis, &mut state);
            recursive_scc_iters += outcome.iterations;
            if outcome.exhausted {
                exhausted_recursive_sccs += 1;
                exhausted_functions.extend(scc.members.iter().copied());
            }
        } else {
            // Non-recursive SCC (single function): collect param ranges, then
            // re-run fixpoint with seeds so interprocedural facts propagate.
            debug_assert_eq!(scc.members.len(), 1);
            let name = scc.members[0];
            if let Some(func) = func_map.get(&name) {
                let info = collect_param_ranges(func, &results, &func_map, &call_graph, pool, plan);
                // Build seed map from collected param ranges.
                let seeds = build_param_seed_map(func, &info);
                // Re-run fixpoint with seeded parameters.
                let result = range_fixpoint(func, pool, config, Some(&seeds), None);
                func_infos.insert(
                    name,
                    FunctionRangeInfo {
                        param_ranges: info.param_ranges,
                        return_range: result.return_range,
                    },
                );
                results.insert(name, result);
            }
        }
    }

    if exhausted_recursive_sccs > 0 {
        tracing::warn!(
            exhausted_recursive_sccs,
            affected_functions = exhausted_functions.len(),
            recursive_scc_iters,
            per_scc_iteration_limit = config.max_scc_iterations,
            "integer range analysis reached its recursive convergence limit; affected values \
             will keep canonical 64-bit representations, so program behavior is unchanged but \
             optimization may be lower. Set \
             ORI_LOG=ori_repr::range::signatures=debug to list affected function IDs; pass \
             --no-repr-opt to bypass this optimization, or report the source if this recurs"
        );
    }

    // Why: Caller-first parameter propagation initially lacks callee return ranges.
    feedback::feed_return_ranges_and_reprocess(
        analysis,
        RangePropagationState {
            results: &mut results,
            func_infos: &mut func_infos,
        },
        &exhausted_functions,
    );

    // INVARIANT: Local narrowing requires ABI widening and overflow guards; field summaries remain safe.
    // Field-range summaries are always stored because they are consumed
    // by integer narrowing itself, not by codegen directly.
    if plan.is_narrowing_safe_for_codegen() {
        for (name, result) in &results {
            plan.set_var_ranges(*name, result.var_ranges.clone());
        }
    }
    for result in results.values() {
        result.field_summaries.flush_to_repr_plan(plan);
        result.element_summaries.flush_to_repr_plan(plan);
    }

    merge_param_ranges_into_plan(plan, &func_infos, &func_map);

    tracing::debug!(
        functions = arc_functions.len(),
        sccs = sccs.len(),
        recursive_scc_iters,
        exhausted_recursive_sccs,
        "interprocedural range propagation complete"
    );
}

fn merge_param_ranges_into_plan(
    plan: &mut ReprPlan,
    func_infos: &FxHashMap<Name, FunctionRangeInfo>,
    func_map: &FxHashMap<Name, &ArcFunction>,
) {
    for (name, info) in func_infos {
        if let Some(func) = func_map.get(name) {
            for pr in &info.param_ranges {
                if pr.param_index < func.params.len() {
                    let param_var = func.params[pr.param_index].var;
                    let intra_range = plan.var_range(*name, param_var);
                    let narrowed = intra_range.meet(pr.range);
                    if let Some(ranges) = plan.function_var_ranges_mut(*name) {
                        ranges.insert(param_var, narrowed);
                    }
                }
            }
        }
    }
}

/// Join every call-site argument range into the target parameter range.
fn collect_param_ranges(
    target_func: &ArcFunction,
    results: &FxHashMap<Name, super::fixpoint::RangeFixpointResult>,
    func_map: &FxHashMap<Name, &ArcFunction>,
    call_graph: &CallGraph,
    pool: &Pool,
    plan: &ReprPlan,
) -> FunctionRangeInfo {
    let num_params = target_func.params.len();

    // Unconstrained functions may be called from external code or via dynamic
    // dispatch, so their parameter ranges must stay Top (full i64 range). Check:
    // 1. (None, name) for pub top-level functions
    // 2. (Some(self_type), name) for trait impl methods (self-type from first param)
    // 3. (None, qualified_name) for type-qualified analysis-only functions
    //    (covers both methods and associated functions via their __impl_ name)
    let self_type = target_func
        .params
        .first()
        .map(|p| target_func.var_type(p.var));
    let is_pub_unconstrained = plan.is_unconstrained_fn(None, target_func.name);
    let is_trait_impl_unconstrained = if let Some(st) = self_type {
        plan.is_unconstrained_fn(Some(st), target_func.name)
    } else {
        false
    };
    // Also check by the ARC function's own name as a qualified key —
    // analysis-only functions use __impl_{idx}_{method} names that are
    // registered in the unconstrained set.
    let is_qualified_unconstrained = plan.is_qualified_unconstrained(target_func.name);
    let unconstrained =
        if is_pub_unconstrained || is_trait_impl_unconstrained || is_qualified_unconstrained {
            Some(UnconstrainedReason::PublicOrTraitImpl)
        } else if target_func.num_captures > 0 {
            Some(UnconstrainedReason::Closure)
        } else {
            None
        };

    if let Some(reason) = unconstrained {
        tracing::debug!(
            func = ?target_func.name,
            ?reason,
            "unconstrained function — all params Top"
        );
        let mut info = FunctionRangeInfo::new_top(num_params);
        // Return range still comes from intraprocedural analysis.
        if let Some(result) = results.get(&target_func.name) {
            info.return_range = result.return_range;
        }
        return info;
    }

    let mut info = FunctionRangeInfo::new_bottom(num_params);

    // The call graph's reverse index avoids rescanning unrelated functions for
    // every target. The body scan remains necessary to recover argument vars
    // and block-local refinements for each direct call site.
    for caller_name in call_graph.callers_of(target_func.name) {
        let Some(caller_func) = func_map.get(caller_name) else {
            continue;
        };
        let Some(caller_result) = results.get(&caller_func.name) else {
            continue;
        };

        for block in &caller_func.blocks {
            // Check body instructions for Apply/PartialApply.
            for instr in &block.body {
                match instr {
                    ArcInstr::Apply {
                        func: callee_name,
                        args,
                        ..
                    } if *callee_name == target_func.name => {
                        // Use block-local refined ranges at the
                        // call site instead of function-global var_ranges.
                        let local = block_local_ranges(
                            &caller_result.var_ranges,
                            &caller_result.block_refinements,
                            block.id,
                            args,
                        );
                        join_arg_ranges(args, &local, &target_func.params, &mut info, pool);
                    }
                    _ => {}
                }
            }

            // Check terminator for Invoke.
            if let ArcTerminator::Invoke {
                func: callee_name,
                args,
                ..
            } = &block.terminator
            {
                if *callee_name == target_func.name {
                    let local = block_local_ranges(
                        &caller_result.var_ranges,
                        &caller_result.block_refinements,
                        block.id,
                        args,
                    );
                    join_arg_ranges(args, &local, &target_func.params, &mut info, pool);
                }
            }
        }
    }

    // If no call sites found, parameters stay Bottom (function is dead code
    // or only called externally — narrowing will treat Bottom same as Top for safety).
    // Actually, Bottom for "never called internally" should be Top for safety.
    for pr in &mut info.param_ranges {
        if pr.range == ValueRange::Bottom {
            // No internal call sites found — could be called externally.
            pr.range = ValueRange::Top;
        }
    }

    // Return range comes from intraprocedural analysis.
    if let Some(result) = results.get(&target_func.name) {
        info.return_range = result.return_range;
    }

    info
}

/// Join argument ranges from a single call site into the parameter info.
fn join_arg_ranges(
    args: &[ArcVarId],
    caller_ranges: &FxHashMap<ArcVarId, ValueRange>,
    target_params: &[ori_arc::ir::ArcParam],
    info: &mut FunctionRangeInfo,
    pool: &Pool,
) {
    for (i, arg_var) in args.iter().enumerate() {
        if i >= target_params.len() || i >= info.param_ranges.len() {
            break;
        }
        // Only propagate ranges for int-typed parameters.
        if !is_int_typed(target_params[i].ty, pool) {
            continue;
        }
        let arg_range = caller_ranges
            .get(arg_var)
            .copied()
            .unwrap_or(ValueRange::Top);
        info.param_ranges[i].range = info.param_ranges[i].range.join(arg_range);
    }
}

/// Compute block-local variable ranges at a call site.
///
/// Intersects the function-global `var_ranges` with any block-entry refinements
/// for the call site's block. When a call like `helper(x)` sits inside an
/// `if x < 5` branch, the block refinement narrows `x` from its global range
/// `[0, 10]` to `[0, 4]` at the call site, yielding a tighter callee parameter.
fn block_local_ranges(
    global_ranges: &FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    block_id: ArcBlockId,
    args: &[ArcVarId],
) -> FxHashMap<ArcVarId, ValueRange> {
    let mut local = FxHashMap::default();
    for &var in args {
        let global = global_ranges.get(&var).copied().unwrap_or(ValueRange::Top);
        let effective = match block_refinements.get(&(block_id, var)) {
            Some(&refinement) => global.meet(refinement),
            None => global,
        };
        local.insert(var, effective);
    }
    local
}

use scc::build_param_seed_map;

#[cfg(test)]
mod tests;
