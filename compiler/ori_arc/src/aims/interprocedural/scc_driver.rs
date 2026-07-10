//! SCC-based fixed-point driver for interprocedural contract computation.
//!
//! Builds the call graph, computes Tarjan SCCs, and processes them in
//! topological order (callees before callers). Non-recursive SCCs get a
//! single intraprocedural pass; recursive SCCs iterate to convergence.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;
use crate::graph::call_graph::CallGraph;
use crate::graph::scc::compute_sccs;
use crate::ir::ArcFunction;
use crate::ArcClassification;

use super::super::contract::{FipContract, MemoryContract};
use super::super::intraprocedural::analyze_function;
use super::demand_propagation::tighten_uniqueness_from_callers;
use super::extract::extract_contract;

/// Compute [`MemoryContract`] for all functions via SCC-based fixed-point.
///
/// Processes SCCs in topological order (callees before callers). Each SCC
/// is analyzed to convergence before moving to the next. Returns a map
/// from function name to its converged contract.
///
/// # Parameters
///
/// - `functions` — all ARC IR functions in the program
/// - `classifier` — type classification (scalar vs ref)
/// - `builtins` — builtin ownership sets
/// - `interner` — string interner for builtin name lookup
pub fn analyze_program(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
    interner: &ori_ir::StringInterner,
) -> FxHashMap<Name, MemoryContract> {
    let graph = CallGraph::build(functions);
    let sccs = compute_sccs(&graph);

    let func_by_name: FxHashMap<Name, &ArcFunction> =
        functions.iter().map(|f| (f.name, f)).collect();

    let mut all_sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    // Pre-seed builtin contracts so call sites get accurate ownership info.
    super::super::builtins::seed_builtin_contracts(&mut all_sigs, builtins, interner);

    for scc in &sccs {
        if scc.is_recursive(&graph) {
            let scc_funcs: Vec<&ArcFunction> = scc
                .members
                .iter()
                .filter_map(|name| func_by_name.get(name).copied())
                .collect();
            if scc_funcs.is_empty() {
                continue;
            }
            let scc_sigs = analyze_scc_fixpoint(&scc_funcs, classifier, &all_sigs, interner);
            all_sigs.extend(scc_sigs);
        } else if let Some(&func) = func_by_name.get(&scc.members[0]) {
            let contract = analyze_scc_single(func, classifier, &all_sigs, interner);
            all_sigs.insert(func.name, contract);
        }
        // External/FFI functions not in `func_by_name` are skipped —
        // their contracts are looked up as conservative fallbacks
        // at call sites via `apply_callee_contract`.
    }

    // Post-fixpoint demand propagation. Tighten a callee
    // parameter's uniqueness to Unique when every caller passes a fresh,
    // single-use Construct argument.
    tighten_uniqueness_from_callers(functions, classifier, &mut all_sigs);

    // FIP coverage reporting.
    let mut fip_certified = 0u32;
    let mut fip_conditional = 0u32;
    let mut fip_bounded = 0u32;
    let mut fip_never = 0u32;
    let mut fbip_count = 0u32;
    for contract in all_sigs.values() {
        match &contract.fip {
            FipContract::Certified => fip_certified += 1,
            FipContract::Conditional { .. } => fip_conditional += 1,
            FipContract::Bounded(_) => fip_bounded += 1,
            FipContract::Never => fip_never += 1,
        }
        if contract.is_fbip {
            fbip_count += 1;
        }
    }
    tracing::debug!(
        functions = all_sigs.len(),
        fip_certified,
        fip_conditional,
        fip_bounded,
        fip_never,
        fbip_count,
        "AIMS interprocedural analysis complete — FIP coverage"
    );

    // Per-contract forwarder visibility: one event per function naming its
    // `transfers_through_return` params, gated by `tracing::enabled!` (zero
    // overhead off). Answers "is callee @inner's forwarder contract in the set
    // a caller @main consults?" — the impl-method interprocedural-contract
    // diagnostic. `ORI_LOG=ori_arc::aims::interprocedural=debug`.
    if tracing::enabled!(target: "ori_arc::aims::interprocedural", tracing::Level::DEBUG) {
        for (name, contract) in &all_sigs {
            let ttr: Vec<usize> = contract
                .params
                .iter()
                .enumerate()
                .filter(|(_, p)| p.transfers_through_return)
                .map(|(i, _)| i)
                .collect();
            let param_dims: Vec<String> = contract
                .params
                .iter()
                .map(|p| {
                    format!(
                        "{:?}/{:?}/{:?}/iter={}",
                        p.access, p.consumption, p.cardinality, p.iter_consumes
                    )
                })
                .collect();
            tracing::debug!(
                target: "ori_arc::aims::interprocedural",
                fn_name = interner.lookup(*name),
                params = contract.params.len(),
                transfers_through_return_params = ?ttr,
                param_dims = ?param_dims,
                may_deallocate = contract.effects.may_deallocate,
                "AIMS contract computed",
            );
        }
    }

    all_sigs
}

/// Analyze a non-recursive function in a single pass.
pub(super) fn analyze_scc_single(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    all_sigs: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> MemoryContract {
    let state_map = analyze_function(func, classifier, all_sigs, &[], Vec::new());
    // Non-recursive: empty SCC peer set → has_unbounded_stack = false.
    // No context regions for non-recursive (TRMC requires recursion).
    let empty_peers = rustc_hash::FxHashSet::default();
    extract_contract(
        func,
        &state_map,
        classifier,
        all_sigs,
        &empty_peers,
        &[],
        interner,
    )
}

/// Analyze a mutually recursive SCC via fixed-point iteration.
///
/// Convergence: contracts are monotonic (params can only promote toward
/// conservative, return uniqueness can only weaken). Each iteration must
/// promote at least one dimension of one parameter, guaranteeing
/// termination in bounded iterations.
fn analyze_scc_fixpoint(
    scc_funcs: &[&ArcFunction],
    classifier: &dyn ArcClassification,
    external_sigs: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashMap<Name, MemoryContract> {
    // Build the SCC peer set for constant-stack analysis.
    let scc_peers: rustc_hash::FxHashSet<Name> = scc_funcs.iter().map(|f| f.name).collect();

    // Initialize all SCC members to most-optimistic contracts.
    let mut local_sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    local_sigs.reserve(scc_funcs.len());
    for &func in scc_funcs {
        local_sigs.insert(
            func.name,
            MemoryContract::all_borrowed(func.params.len(), FipContract::Never),
        );
    }

    // Build a combined sig map: external (finalized) + local (iterating).
    // Local sigs shadow external ones for SCC members.
    //
    // NOTE: This clones `external_sigs` once per SCC. A layered lookup
    // (check local first, then external) would avoid the clone but
    // requires changing `analyze_function`'s `&FxHashMap` parameter to
    // a trait. Performance note: clone cost is O(depth) where depth is
    // the number of finalized contracts, negligible for typical programs.
    let mut combined_sigs = external_sigs.clone();

    let mut changed = true;
    let mut iterations = 0u32;
    while changed {
        changed = false;

        // Update combined with current local sigs.
        for (&name, contract) in &local_sigs {
            combined_sigs.insert(name, contract.clone());
        }

        for &func in scc_funcs {
            let state_map = analyze_function(func, classifier, &combined_sigs, &[], Vec::new());
            // Detect TRMC context regions (detection only — no rewrite during
            // interprocedural fixpoint; the rewrite runs in the per-function
            // pipeline after contracts converge).
            let context_regions = crate::aims::normalize::detect_context_regions(func);
            let new_contract = extract_contract(
                func,
                &state_map,
                classifier,
                &combined_sigs,
                &scc_peers,
                &context_regions,
                interner,
            );

            let old_contract = &local_sigs[&func.name];
            if &new_contract != old_contract {
                // Join to ensure monotonicity: contracts can only grow
                // toward conservative.
                let joined = old_contract.join(&new_contract);
                if &joined != old_contract {
                    local_sigs.insert(func.name, joined);
                    changed = true;
                }
            }
        }
        iterations += 1;
    }

    // Convergence bound: each iteration promotes at least one lattice step
    // (single-height increment); the total height-sum is the upper bound.
    let total_height = compute_convergence_bound(scc_funcs);
    debug_assert!(
        iterations <= total_height.saturating_add(1),
        "AIMS fixed-point exceeded convergence bound: \
         {iterations} iterations for {total_height} height units"
    );

    tracing::debug!(
        scc_size = scc_funcs.len(),
        iterations,
        "AIMS SCC fixed-point converged"
    );

    local_sigs
}

/// Worst-case fixpoint convergence bound for an SCC (IC-7 height-sum).
fn compute_convergence_bound(scc_funcs: &[&ArcFunction]) -> u32 {
    // Each iteration promotes at least one lattice step; the height-weighted
    // sum over all SCC members is the upper bound. Per-member height:
    //   per-param height: access(1) + consumption(3) + cardinality(2)
    //                   + locality(4) + uniqueness(2) + may_escape(1)
    //                   + may_share(1) + transfers_through_return(1)
    //                   + return_alias(2) = 17
    //   return height:    uniqueness(2) + freshness(1) + locality(4)
    //                   + shape(1) = 8
    //   effect height:    5 fixpoint-participating bools (may_allocate +
    //                   may_deallocate + may_share + may_throw +
    //                   has_unbounded_stack); alloc_only_on_slow_path is
    //                   post-realization (excluded). The spec lists 6 because
    //                   it includes may_read_inaccessible (target-only, not yet
    //                   in shipped struct); shipped reality is 5.
    //   context height:  4 boolean fields (PL-7/PL-11)
    //
    // return_alias: Option<ReturnAliasShape> contributes height 2 (chain
    // None < Some(Project) < Some(Direct)) to the per-param total.
    // transfers_through_return: bool (height 1) remains the callee-side
    // gate; return_alias is the caller-side carrier, both join
    // componentwise in IC-3.
    //
    // Spec IC-7 lists per-param height 13 (omitting may_escape, slated for
    // removal) plus transfers_through_return (1) + return_alias (2) = 16.
    // Shipped ParamContract still has may_escape, so the runtime bound
    // matches shipped reality at 17; it drops to 16 when may_escape is removed.
    // Height-weighted (not dimension-count) form matches the worst-case
    // fixpoint convergence proof for IC-7.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "param count per function bounded by u32::MAX in practice"
    )]
    scc_funcs
        .iter()
        .map(|f| {
            let param_height = f.params.len() as u32 * 17;
            let return_height = 8u32;
            let effect_height = 5u32;
            let context_height = 4u32;
            param_height + return_height + effect_height + context_height
        })
        .sum()
}
