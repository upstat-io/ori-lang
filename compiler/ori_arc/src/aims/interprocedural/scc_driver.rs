//! SCC-based fixed-point driver for interprocedural contract computation.
//!
//! Builds the call graph, computes Tarjan SCCs, and processes them in
//! topological order (callees before callers). Non-recursive SCCs get a
//! single intraprocedural pass; recursive SCCs iterate to convergence.
//! Diagnostics share the `ori_arc::aims::interprocedural` target so one filter
//! observes every participant in the fixed-point computation.

use std::collections::HashMap;
use std::hash::BuildHasher;

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::borrow::BuiltinOwnershipSets;
use crate::graph::call_graph::CallGraph;
use crate::graph::scc::compute_sccs;
use crate::ir::ArcFunction;
use crate::pipeline::callable_boundary::ValidatedCallableBoundaryFacts;
use crate::ArcClassification;

use super::super::contract::{FipContract, MemoryContract};
use super::super::intraprocedural::analyze_function;
use super::demand_propagation::tighten_uniqueness_from_callers;
use super::extract::extract_contract_with_call_ownership;

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
    analyze_program_with_external_contracts(
        functions,
        classifier,
        builtins,
        interner,
        &FxHashMap::default(),
    )
}

/// Compute local contracts while treating producer-frozen external contracts
/// as immutable interprocedural inputs.
pub(crate) fn analyze_program_with_external_contracts<S: BuildHasher>(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
    interner: &ori_ir::StringInterner,
    external_contracts: &HashMap<Name, MemoryContract, S>,
) -> FxHashMap<Name, MemoryContract> {
    analyze_program_with_external_contracts_and_boundaries(
        functions,
        classifier,
        builtins,
        interner,
        external_contracts,
        &ValidatedCallableBoundaryFacts::empty(),
    )
}

/// Compute local contracts with immutable external inputs and exact semantic
/// callable-boundary roles.
pub(crate) fn analyze_program_with_external_contracts_and_boundaries<S: BuildHasher>(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
    interner: &ori_ir::StringInterner,
    external_contracts: &HashMap<Name, MemoryContract, S>,
    callable_boundaries: &ValidatedCallableBoundaryFacts<'_>,
) -> FxHashMap<Name, MemoryContract> {
    let graph = CallGraph::build(functions);
    let sccs = compute_sccs(&graph);

    let func_by_name: FxHashMap<Name, &ArcFunction> =
        functions.iter().map(|f| (f.name, f)).collect();
    let exact_callables: FxHashSet<Name> = func_by_name
        .keys()
        .copied()
        .chain(external_contracts.keys().copied())
        .collect();

    let mut all_sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    // Pre-seed builtin contracts so call sites get accurate ownership info.
    super::super::builtins::seed_builtin_contracts(&mut all_sigs, builtins, interner);
    // Explicit compiled-unit imports override same-spelled builtin seeds. The
    // call target table makes the distinction structural, so the calculus
    // consumes the producer's exact contract rather than name heuristics.
    all_sigs.extend(
        external_contracts
            .iter()
            .map(|(&name, contract)| (name, contract.clone())),
    );

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
            let scc_sigs = analyze_scc_fixpoint(
                &scc_funcs,
                classifier,
                &all_sigs,
                interner,
                builtins,
                &exact_callables,
                callable_boundaries,
            );
            all_sigs.extend(scc_sigs);
        } else if let Some(&func) = func_by_name.get(&scc.members[0]) {
            let contract = analyze_scc_single(
                func,
                classifier,
                &all_sigs,
                interner,
                builtins,
                &exact_callables,
                callable_boundaries,
            );
            all_sigs.insert(func.name, contract);
        }
        // External/FFI functions not in `func_by_name` are skipped —
        // their contracts are looked up as conservative fallbacks
        // at call sites via `apply_callee_contract`.
    }

    // Post-fixpoint demand propagation. Tighten a callee
    // parameter's uniqueness to Unique when every caller passes a fresh,
    // single-use Construct argument.
    let immutable_external_names: rustc_hash::FxHashSet<_> =
        external_contracts.keys().copied().collect();
    tighten_uniqueness_from_callers(
        functions,
        classifier,
        &mut all_sigs,
        &immutable_external_names,
    );

    trace_contract_summary(&all_sigs, interner);

    all_sigs
}

/// Report converged contract coverage and per-function demand dimensions.
fn trace_contract_summary(
    all_sigs: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) {
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

    // Why: Build per-parameter summaries only while contract tracing is enabled.
    if tracing::enabled!(target: "ori_arc::aims::interprocedural", tracing::Level::DEBUG) {
        for (name, contract) in all_sigs {
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
                        "{:?}/{:?}/{:?}/iter={}/ralias={:?}",
                        p.access, p.consumption, p.cardinality, p.iter_consumes, p.return_alias
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
                return_uniqueness = ?contract.return_info.uniqueness,
                returns_fresh_self_alloc = contract.return_info.returns_fresh_self_alloc,
                preserves_freshness = contract.return_info.preserves_freshness,
                "AIMS contract computed",
            );
        }
    }
}

/// Analyze a non-recursive function in a single pass.
pub(super) fn analyze_scc_single(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    all_sigs: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    builtins: &BuiltinOwnershipSets,
    exact_callables: &FxHashSet<Name>,
    callable_boundaries: &ValidatedCallableBoundaryFacts<'_>,
) -> MemoryContract {
    let state_map = analyze_function(func, classifier, all_sigs, &[], Vec::new());
    // Non-recursive: empty SCC peer set → has_unbounded_stack = false.
    // No context regions for non-recursive (TRMC requires recursion).
    let empty_peers = rustc_hash::FxHashSet::default();
    let mut contract = extract_contract_with_call_ownership(&super::ContractExtractionInput {
        func,
        state_map: &state_map,
        classifier,
        sigs: all_sigs,
        scc_peers: &empty_peers,
        context_regions: &[],
        interner,
        builtins,
        exact_callables,
    });
    callable_boundaries.constrain_contract(func.name, &mut contract);
    contract
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
    builtins: &BuiltinOwnershipSets,
    exact_callables: &FxHashSet<Name>,
    callable_boundaries: &ValidatedCallableBoundaryFacts<'_>,
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

    // Local contracts shadow finalized external contracts while the SCC
    // iterates. Cloning keeps `analyze_function` on its concrete map API.
    let mut combined_sigs = external_sigs.clone();

    let mut changed = true;
    let mut iterations = 0usize;
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
            let mut new_contract =
                extract_contract_with_call_ownership(&super::ContractExtractionInput {
                    func,
                    state_map: &state_map,
                    classifier,
                    sigs: &combined_sigs,
                    scc_peers: &scc_peers,
                    context_regions: &context_regions,
                    interner,
                    builtins,
                    exact_callables,
                });
            callable_boundaries.constrain_contract(func.name, &mut new_contract);

            let old_contract = &local_sigs[&func.name];
            if &new_contract != old_contract {
                // INVARIANT: Contract joins move only toward conservative.
                let joined = old_contract.join(&new_contract);
                if &joined != old_contract {
                    local_sigs.insert(func.name, joined);
                    changed = true;
                }
            }
        }
        let Some(next_iteration) = iterations.checked_add(1) else {
            panic!("AIMS fixed-point iteration count must fit usize");
        };
        iterations = next_iteration;
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
fn compute_convergence_bound(scc_funcs: &[&ArcFunction]) -> usize {
    const PARAM_HEIGHT_WITH_MAY_ESCAPE: usize = 17;
    const RETURN_HEIGHT: usize = 8;
    const EFFECT_HEIGHT: usize = 5;
    const CONTEXT_HEIGHT: usize = 4;
    const FIXED_HEIGHT: usize = RETURN_HEIGHT + EFFECT_HEIGHT + CONTEXT_HEIGHT;

    let bound = scc_funcs.iter().try_fold(0usize, |total, f| {
        let member_height = f
            .params
            .len()
            .checked_mul(PARAM_HEIGHT_WITH_MAY_ESCAPE)?
            .checked_add(FIXED_HEIGHT)?;
        total.checked_add(member_height)
    });
    let Some(bound) = bound else {
        panic!("AIMS convergence-bound height sum must fit usize");
    };
    bound
}
