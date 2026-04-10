//! Batch orchestration: run AIMS pipeline on all functions.
//!
//! Contains the batch entry point (`run_aims_pipeline_all`), the second-pass
//! TRMC contract refresh and FIP recomputation (`run_second_pass`), and
//! ownership application (`apply_aims_ownership`).

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::AimsPipelineConfig;
use crate::aims::contract::{MemoryContract, ParamContract};
use crate::aims::lattice::AccessClass;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::Ownership;
use crate::ArcClassification;

/// Run the AIMS pipeline on all functions (batch entry point).
///
/// Called from within `run_arc_pipeline_all` when the `aims` feature is active.
///
/// 1. Compute interprocedural contracts via `aims::analyze_program()`
/// 2. Apply ownership to function parameters
/// 3. Run per-function pipeline for each function
pub(crate) fn run_aims_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    builtins: &BuiltinOwnershipSets,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    // Step 1: interprocedural analysis -> MemoryContract per function.
    let mut contracts = {
        let _span = tracing::info_span!("analyze_program").entered();
        crate::aims::interprocedural::analyze_program(functions, classifier, builtins, interner)
    };

    // Step 2: apply ownership to function parameters.
    {
        let _span = tracing::info_span!("apply_ownership").entered();
        apply_aims_ownership(functions, &contracts);
    }

    // Steps 3-14: per-function pipeline.
    let config = AimsPipelineConfig {
        classifier,
        contracts: &contracts,
        pool,
        interner,
        builtins,
        verify_arc,
    };

    let mut all_problems = Vec::new();
    let mut total_rc = crate::pipeline::rc_count::RcOpCount::default();
    // Collect post-emission missed_reuses for second-pass FIP recomputation.
    // Preserves the full count (not just bool) so Bounded(n) contracts can
    // be re-verified with accurate evidence.
    let mut reuse_updates: Vec<(Name, usize)> = Vec::new();
    // Track TRMC-rewritten functions for contract refresh (Bug 2).
    let mut trmc_rewritten: Vec<Name> = Vec::new();
    for func in functions.iter_mut() {
        let result = super::run_aims_pipeline(func, &config);
        all_problems.extend(result.problems);
        reuse_updates.push((func.name, result.missed_reuses));
        if result.was_trmc_rewritten {
            trmc_rewritten.push(func.name);
        }
        let rc = crate::pipeline::rc_count::count_rc_ops(func);
        total_rc.inc += rc.inc;
        total_rc.dec += rc.dec;
    }

    // Second pass: TRMC contract refresh -> may_deallocate -> FIP.
    run_second_pass(
        functions,
        &mut contracts,
        &trmc_rewritten,
        &reuse_updates,
        classifier,
    );

    tracing::debug!(
        functions = functions.len(),
        rc_inc = total_rc.inc,
        rc_dec = total_rc.dec,
        rc_total = total_rc.total(),
        "AIMS pipeline RC operation totals"
    );

    all_problems
}

/// Second pass: refresh contracts for TRMC-rewritten functions, then
/// update `may_deallocate` and FIP classifications.
///
/// Ordering: (1) TRMC contract refresh, (2) `may_deallocate` update,
/// (3) FIP recomputation, (4) FIP re-verification.
fn run_second_pass(
    functions: &[ArcFunction],
    contracts: &mut FxHashMap<Name, MemoryContract>,
    trmc_rewritten: &[Name],
    reuse_updates: &[(Name, usize)],
    classifier: &dyn crate::ArcClassification,
) {
    // Phase 1: full contract refresh for TRMC-rewritten functions.
    // Re-run analysis + extraction on the rewritten IR to get accurate
    // ContextBehavior, FipContract, and EffectSummary.
    if !trmc_rewritten.is_empty() {
        let _span = tracing::info_span!("trmc_contract_refresh").entered();
        for &name in trmc_rewritten {
            // Find the rewritten function.
            let Some(func) = functions.iter().find(|f| f.name == name) else {
                continue;
            };
            // Re-analyze with current contracts as peer context.
            let state_map = crate::aims::intraprocedural::analyze_function(
                func,
                classifier,
                contracts,
                &[],
                Vec::new(),
            );
            let context_regions = crate::aims::normalize::detect_context_regions(func);
            // No SCC peers needed — TRMC rewrite is per-function and
            // the function's own contract is already in `contracts`.
            let new_contract = crate::aims::interprocedural::extract_contract(
                func,
                &state_map,
                classifier,
                contracts,
                &rustc_hash::FxHashSet::default(),
                &context_regions,
            );
            if let Some(old) = contracts.get_mut(&name) {
                tracing::debug!(
                    func = name.raw(),
                    old_unbounded = old.effects.has_unbounded_stack,
                    new_unbounded = new_contract.effects.has_unbounded_stack,
                    "TRMC full contract refresh"
                );
                *old = new_contract;
            }
        }
    }

    // Phase 2: update contracts with post-emission may_deallocate facts.
    {
        let _span = tracing::info_span!("post_emission_fip_update").entered();
        let mut downgrades = 0u32;
        for (name, missed_reuses) in reuse_updates {
            if let Some(contract) = contracts.get_mut(name) {
                contract.effects.may_deallocate = *missed_reuses > 0;
                if crate::aims::verify::fip::recompute_fip_for_may_deallocate(contract) {
                    downgrades += 1;
                    tracing::debug!(
                        func = name.raw(),
                        "FIP contract downgraded to Never after may_deallocate update"
                    );
                }
            }
        }
        if downgrades > 0 {
            tracing::info!(
                downgrades,
                "FIP contracts downgraded after may_deallocate update"
            );
        }
    }

    // Phase 3: re-verify FIP contracts with corrected data.
    {
        let _span = tracing::info_span!("post_emission_fip_verify").entered();
        debug_assert_eq!(
            functions.len(),
            reuse_updates.len(),
            "reuse_updates must match functions 1:1"
        );
        for (func, (update_name, missed_reuses)) in functions.iter().zip(reuse_updates.iter()) {
            debug_assert_eq!(
                func.name, *update_name,
                "reuse_updates order must match functions order"
            );
            if let Some(contract) = contracts.get(&func.name) {
                let evidence = crate::aims::realize::FipEvidence {
                    fip_gates: vec![],
                    missed_reuses: *missed_reuses,
                };
                let fip_errors =
                    crate::aims::verify::fip::verify_fip_contract(func.name, contract, &evidence);
                for e in &fip_errors {
                    tracing::error!("FIP post-recompute verification failed: {e}");
                    debug_assert!(false, "FIP post-recompute verification failed: {e}");
                }
            }
        }
    }
}

/// Apply AIMS ownership annotations to function parameters.
///
/// Sets `ArcParam.ownership` on each function from its `MemoryContract`.
/// Replaces `borrow::apply_borrows()` in the old pipeline.
pub(crate) fn apply_aims_ownership(
    functions: &mut [ArcFunction],
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    for func in functions {
        let Some(contract) = contracts.get(&func.name) else {
            continue;
        };
        for (param, pc) in func.params.iter_mut().zip(&contract.params) {
            param.ownership = param_contract_to_ownership(*pc);
        }
    }
}

/// Convert a `ParamContract` access class to the `Ownership` enum used by
/// `ArcParam`. This bridges the AIMS contract representation with the
/// existing ARC IR parameter ownership field.
fn param_contract_to_ownership(pc: ParamContract) -> Ownership {
    match pc.access {
        AccessClass::Borrowed => Ownership::Borrowed,
        AccessClass::Owned => Ownership::Owned,
    }
}
