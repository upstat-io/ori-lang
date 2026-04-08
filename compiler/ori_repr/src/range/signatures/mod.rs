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
//! 3. Process SCCs in forward topological order (leaves first):
//!    - **Non-recursive SCC**: single pass — collect argument ranges at all call
//!      sites, join into parameter ranges.
//!    - **Recursive SCC**: iterate — re-run `range_fixpoint()` with parameter
//!      constraints until parameter + return ranges stabilize or budget exhausted.
//! 4. Store results in `ReprPlan::function_var_ranges`.
//!
//! # Budget
//!
//! - `max_scc_iterations` (default 10): per-SCC iteration cap.
//! - `max_total_scc_iterations` (default 50): cross-SCC cap.
//! - If either is exceeded, remaining parameters get `Top` (safe fallback).

mod feedback;
mod scc;

use ori_arc::graph::call_graph::CallGraph;
use ori_arc::graph::scc::compute_sccs;
use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator};
use ori_arc::{ArcBlockId, ArcVarId};
use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::{is_int_typed, RangeAnalysisConfig, ValueRange};
use crate::plan::ReprPlan;
use crate::range::fixpoint::range_fixpoint;

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

    // Phase 1: Intraprocedural analysis for each function (no seeds).
    let mut results: FxHashMap<Name, super::fixpoint::RangeFixpointResult> = FxHashMap::default();
    for func in arc_functions {
        let result = range_fixpoint(func, pool, config, None, None);
        results.insert(func.name, result);
    }

    // Phase 2: Build call graph and compute SCCs.
    let call_graph = CallGraph::build(arc_functions);
    let sccs = compute_sccs(&call_graph);

    // Phase 3: Process SCCs in reverse topological order (callers first).
    // Parameter ranges flow top-down (caller → callee), so callers must be
    // processed first: when we seed callee C's fixpoint, all callers of C
    // must already have their final ranges so C's param seed is accurate.
    // `compute_sccs()` returns forward order (leaves first); we reverse it.
    let mut func_infos: FxHashMap<Name, FunctionRangeInfo> = FxHashMap::default();
    let mut total_scc_iters: usize = 0;

    for scc in sccs.iter().rev() {
        if total_scc_iters >= config.max_total_scc_iterations {
            tracing::warn!(
                remaining_sccs = sccs.len(),
                "SCC budget exhausted — remaining functions get Top"
            );
            // Assign Top to all remaining unprocessed functions.
            for member in &scc.members {
                if let Some(func) = func_map.get(member) {
                    func_infos.insert(*member, FunctionRangeInfo::new_top(func.params.len()));
                }
            }
            continue;
        }

        if scc.is_recursive(&call_graph) {
            // Recursive SCC: iterate to fixpoint with parameter seeding.
            // pass remaining total budget so the SCC doesn't
            // overshoot max_total_scc_iterations.
            let remaining_budget = config
                .max_total_scc_iterations
                .saturating_sub(total_scc_iters);
            total_scc_iters += scc::process_recursive_scc(
                scc,
                &func_map,
                pool,
                config,
                &mut results,
                &mut func_infos,
                remaining_budget,
                plan,
            );
        } else {
            // Non-recursive SCC (single function): collect param ranges, then
            // re-run fixpoint with seeds so interprocedural facts propagate.
            debug_assert_eq!(scc.members.len(), 1);
            let name = scc.members[0];
            if let Some(func) = func_map.get(&name) {
                let info = collect_param_ranges(func, &results, &func_infos, &func_map, pool, plan);
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
            total_scc_iters += 1;
        }
    }

    // Phase 3.5: Return-range feedback.
    // Phase 3 processes callers first (reverse topo) for parameter propagation,
    // so callee return ranges aren't in callers' var_ranges. This pass feeds
    // callee return ranges back into results, then re-collects parameter ranges
    // and re-runs fixpoints for functions whose seeds changed.
    feedback::feed_return_ranges_and_reprocess(
        &sccs,
        &func_map,
        pool,
        config,
        &mut results,
        &mut func_infos,
        plan,
    );

    // Phase 4: Store results in ReprPlan.
    // Per-variable ranges are only stored when narrowing is safe for codegen.
    // The ARC emitter reads var_range() to insert trunc/sext for local variables —
    // without ABI widening and overflow guards, applying narrowed widths to locals
    // is unsound. Field-range summaries are always stored because they are consumed
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

    // Phase 5: Merge interprocedural parameter ranges into ReprPlan.
    for (name, info) in &func_infos {
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

    tracing::debug!(
        functions = arc_functions.len(),
        sccs = sccs.len(),
        total_scc_iters,
        "interprocedural range propagation complete"
    );
}

/// Collect parameter ranges for a single function from all call sites.
///
/// For each call site targeting this function, joins the argument ranges
/// into the corresponding parameter range.
fn collect_param_ranges(
    target_func: &ArcFunction,
    results: &FxHashMap<Name, super::fixpoint::RangeFixpointResult>,
    _func_infos: &FxHashMap<Name, FunctionRangeInfo>,
    func_map: &FxHashMap<Name, &ArcFunction>,
    pool: &Pool,
    plan: &ReprPlan,
) -> FunctionRangeInfo {
    let num_params = target_func.params.len();

    // Check if this function is unconstrained — pub, trait impl, or closure.
    // Unconstrained functions may be called from external code or via dynamic
    // dispatch, so their parameter ranges must stay Top (full i64 range).
    // Check if this function is unconstrained. We check:
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
    // registered in the unconstrained set (TPR-03-043/044/046).
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

    // Scan all functions for call sites targeting this function.
    for caller_func in func_map.values() {
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
