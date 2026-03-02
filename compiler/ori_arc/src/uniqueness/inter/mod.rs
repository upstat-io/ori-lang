//! Interprocedural uniqueness analysis (Section 07.3).
//!
//! Extends intraprocedural uniqueness analysis to the whole program by
//! computing [`UniquenessSummary`] for every function. Summaries capture
//! whether a function's return value is provably `Unique` (RC == 1),
//! allowing callers to treat call results as unique rather than the
//! conservative `MaybeShared`.
//!
//! # Algorithm
//!
//! Uses the same SCC-based fixpoint pattern as borrow inference:
//!
//! 1. Build call graph, compute SCCs via Tarjan's algorithm.
//! 2. Process SCCs in topological order (callees before callers).
//! 3. Non-recursive SCCs: single-pass analysis.
//! 4. Recursive SCCs: iterate within the SCC until convergence.
//!
//! For each function, we run the intraprocedural analysis with known
//! callee summaries, then extract the return value uniqueness from all
//! `Return` terminators.
//!
//! # Collection Method Summaries
//!
//! Built-in COW operations (push, insert, etc.) have hardcoded summaries:
//! their return value is always `Unique` because COW guarantees the result
//! has RC == 1 (either in-place mutation or fresh copy).
//!
//! # References
//!
//! - Lean 4 `Borrow.lean`: SCC-based fixpoint for ownership inference
//! - Koka `Parc.hs`: reverse-liveness ownership propagation
//! - Roc `morphic_lib/analyze.rs`: alias analysis with update modes

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::graph::call_graph::CallGraph;
use crate::graph::scc::compute_sccs;
use crate::ir::{ArcFunction, ArcTerminator};
use crate::liveness::{compute_liveness, BlockLiveness};
use crate::ArcClassification;

use super::intra::analyze_with_summaries;
use super::{Uniqueness, UniquenessSummary};

/// Analyze uniqueness across all functions in a program.
///
/// Produces a [`UniquenessSummary`] for each function, capturing the
/// uniqueness of its return value. Summaries enable callers to refine
/// `Apply` results from `MaybeShared` to the callee's actual return
/// uniqueness.
///
/// # Arguments
///
/// * `functions` — ARC IR functions to analyze.
/// * `classifier` — type classification for RC tracking.
/// * `builtin_summaries` — hardcoded summaries for external/runtime
///   functions (e.g., COW operations). These are not in the call graph
///   but their return uniqueness is known statically.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the concrete type used throughout ori_arc"
)]
pub fn analyze_program(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    builtin_summaries: &FxHashMap<Name, UniquenessSummary>,
) -> FxHashMap<Name, UniquenessSummary> {
    let graph = CallGraph::build(functions);
    let sccs = compute_sccs(&graph);

    let func_by_name: FxHashMap<Name, &ArcFunction> =
        functions.iter().map(|f| (f.name, f)).collect();

    // Start with builtin summaries as the base — user function summaries
    // are added as each SCC is processed.
    let mut all_summaries = builtin_summaries.clone();

    for scc in &sccs {
        if scc.is_recursive(&graph) {
            analyze_recursive_scc(scc, &func_by_name, classifier, &mut all_summaries);
        } else if let Some(&func) = func_by_name.get(&scc.members[0]) {
            let summary = analyze_single_function(func, classifier, &all_summaries);
            all_summaries.insert(func.name, summary);
        }
    }

    all_summaries
}

/// Analyze a non-recursive function and produce its summary.
///
/// Computes liveness internally. For SCC fixpoint loops where liveness is
/// invariant across iterations, use [`summarize_with_liveness`] instead.
fn analyze_single_function(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    summaries: &FxHashMap<Name, UniquenessSummary>,
) -> UniquenessSummary {
    let liveness = compute_liveness(func, classifier);
    summarize_with_liveness(func, classifier, &liveness, summaries)
}

/// Analyze a function with precomputed liveness and produce its summary.
///
/// Liveness depends only on `func` and `classifier`, not on `summaries`,
/// so it can be computed once and reused across fixpoint iterations.
fn summarize_with_liveness(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    summaries: &FxHashMap<Name, UniquenessSummary>,
) -> UniquenessSummary {
    let result = analyze_with_summaries(func, classifier, liveness, summaries);
    let return_val = compute_return_uniqueness(func, &result);

    tracing::debug!(
        function = func.name.raw(),
        return_uniqueness = %return_val,
        "interprocedural summary computed"
    );

    UniquenessSummary {
        params: func
            .params
            .iter()
            .map(|_| Uniqueness::MaybeShared)
            .collect(),
        return_val,
        // Conservative: only set true when the return is Unique even
        // with all params as MaybeShared (which is the standard analysis).
        preserves_freshness: return_val == Uniqueness::Unique,
    }
}

/// Analyze a recursive SCC via fixpoint iteration.
///
/// All members start with conservative summaries (`MaybeShared` return).
/// Each iteration re-analyzes every member using the current summaries.
/// Iteration stops when no summary changes — convergence is guaranteed
/// because uniqueness can only move from `Unique` toward `MaybeShared`
/// (monotonic in the lattice).
fn analyze_recursive_scc(
    scc: &crate::graph::scc::Scc,
    func_by_name: &FxHashMap<Name, &ArcFunction>,
    classifier: &dyn ArcClassification,
    all_summaries: &mut FxHashMap<Name, UniquenessSummary>,
) {
    // Initialize SCC members with conservative summaries.
    for &name in &scc.members {
        if let Some(&func) = func_by_name.get(&name) {
            let summary = UniquenessSummary::conservative(func.params.len());
            all_summaries.insert(name, summary);
        }
    }

    // Precompute liveness for all SCC members. Liveness depends only on
    // func + classifier (invariant across iterations), not on summaries.
    // This avoids N*(K-1) redundant recomputations for N members, K iterations.
    let liveness_cache: FxHashMap<Name, BlockLiveness> = scc
        .members
        .iter()
        .filter_map(|&name| {
            func_by_name
                .get(&name)
                .map(|&func| (name, compute_liveness(func, classifier)))
        })
        .collect();

    let mut iteration = 0u32;
    loop {
        iteration += 1;
        let mut changed = false;

        for &name in &scc.members {
            let (Some(&func), Some(liveness)) =
                (func_by_name.get(&name), liveness_cache.get(&name))
            else {
                continue;
            };

            let new_summary = summarize_with_liveness(func, classifier, liveness, all_summaries);

            if all_summaries.get(&name) != Some(&new_summary) {
                all_summaries.insert(name, new_summary);
                changed = true;
            }
        }

        if !changed {
            tracing::debug!(
                scc_size = scc.members.len(),
                iterations = iteration,
                "recursive SCC uniqueness analysis converged"
            );
            break;
        }
    }
}

/// Extract the return value uniqueness from intraprocedural analysis results.
///
/// Joins the uniqueness of the return value across all `Return` terminators
/// in the function. If a function has multiple return points with different
/// uniqueness states, the join (lattice LUB) produces the conservative result.
fn compute_return_uniqueness(
    func: &ArcFunction,
    result: &super::intra::UniquenessResult,
) -> Uniqueness {
    let mut return_uniq: Option<Uniqueness> = None;

    for (block_idx, block) in func.blocks.iter().enumerate() {
        if let ArcTerminator::Return { value } = &block.terminator {
            let val_uniq = result.block_out[block_idx].get(*value);
            return_uniq = Some(match return_uniq {
                None => val_uniq,
                Some(existing) => existing.join(val_uniq),
            });
        }
    }

    return_uniq.unwrap_or(Uniqueness::MaybeShared)
}

/// Build hardcoded summaries for COW collection methods.
///
/// COW operations always return `Unique` results because:
/// - **Fast path** (RC == 1): mutated in-place, still RC == 1.
/// - **Slow path** (RC > 1): fresh copy allocated, RC starts at 1.
///
/// The returned map should be passed to [`analyze_program`] as
/// `builtin_summaries`. Names are interned via the provided set.
///
/// # Arguments
///
/// * `cow_method_names` — set of method `Name`s that are COW operations
///   (e.g., `push`, `insert`, `pop`, `reverse`, `sort`, `remove`, `union`).
///   Use [`crate::borrow::all_cow_method_names`] to get the union of all
///   COW method sets (both `consuming_receiver_builtin_names` for list COW
///   and `consuming_receiver_only_builtin_names` for map/set COW).
/// * `shared_method_names` — set of method `Name`s that return shared
///   values (e.g., `slice`, `substring`). These share backing storage.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashSet/FxHashMap are the concrete types used throughout ori_arc"
)]
pub fn build_cow_summaries(
    cow_method_names: &rustc_hash::FxHashSet<Name>,
    shared_method_names: &rustc_hash::FxHashSet<Name>,
) -> FxHashMap<Name, UniquenessSummary> {
    let mut summaries = FxHashMap::default();

    for &name in cow_method_names {
        summaries.insert(
            name,
            UniquenessSummary {
                // Empty: builtin summaries only use `return_val`. Callers must not
                // index into `params` — see UniquenessSummary field docs.
                params: Vec::new(),
                return_val: Uniqueness::Unique,
                preserves_freshness: true,
            },
        );
    }

    for &name in shared_method_names {
        summaries.insert(
            name,
            UniquenessSummary {
                // Empty: builtin summaries only use `return_val`.
                params: Vec::new(),
                return_val: Uniqueness::MaybeShared,
                preserves_freshness: false,
            },
        );
    }

    summaries
}

#[cfg(test)]
mod tests;
