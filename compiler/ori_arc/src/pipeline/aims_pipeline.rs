//! AIMS pipeline implementation.
//!
//! Replaces the sequential analysis passes (borrow inference, liveness,
//! uniqueness, RC insertion, reset/reuse, RC elimination) with the unified
//! AIMS analysis + emission pipeline.
//!
//! # Pipeline (Section 10 — unified realization)
//!
//! **Interprocedural** (once across all functions):
//! 1. `aims::analyze_program()` — compute `MemoryContract` per function
//! 2. `aims::apply_ownership()` — populate `ArcParam.ownership`
//!
//! **Per-function** (steps 3–12):
//! 3. `compute_var_reprs()` — fill `ValueRepr` per variable
//! 3a. `aims::normalize_function()` — TRMC context region detection
//! 4. `aims::analyze_function()` — backward dataflow → converged state map
//! 5. `aims::realize_rc_reuse()` — Phase 1: `arg_ownership` + RC + reuse (pre-merge)
//! 5a. `aims::verify::fip::verify_fip_contract()` — FIP enforcement verification
//! 6. `verify()` — ARC IR sanity check
//! 7. `run_aims_verify()` — AIMS contract vs IR consistency
//! 8. `detect_tail_calls()` + `rewrite_tail_calls()`
//! 9. `merge_blocks()` — CFG cleanup
//! 10. `aims::realize_annotations()` — Phase 2: COW + drop hints (post-merge)
//! 11. `verify()` — final sanity check
//! 12. FBIP enforcement — read-only diagnostic

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::{MemoryContract, ParamContract};
use crate::aims::lattice::AccessClass;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::Ownership;
use crate::ArcClassification;

/// Configuration for the AIMS per-function pipeline.
///
/// Bundles the shared parameters that `run_aims_pipeline` needs, avoiding
/// the 7-parameter signature anti-pattern from the old pipeline.
pub(crate) struct AimsPipelineConfig<'a> {
    pub classifier: &'a dyn ArcClassification,
    pub contracts: &'a FxHashMap<Name, MemoryContract>,
    pub pool: &'a Pool,
    pub interner: &'a ori_ir::StringInterner,
    pub builtins: &'a BuiltinOwnershipSets,
    pub verify_arc: bool,
    // Note: `disabled_canonicalize_rules` was considered for debugging
    // cross-dimension regressions (Section 11 §11.3 Option A) but deferred —
    // per-rule unit tests (lattice/tests.rs) and end-to-end synergy tests
    // (realize/tests.rs) provide sufficient coverage for regression detection.
}

/// Result of `run_aims_pipeline` for a single function.
pub(crate) struct AimsPipelineResult {
    pub problems: Vec<ArcProblem>,
    /// Post-emission missed reuse count from `FipEvidence`. Used by the
    /// second pass to compute `may_deallocate` (> 0) and to re-verify
    /// `Bounded(n)` contracts with accurate counts.
    pub missed_reuses: usize,
    /// Whether this function was TRMC-rewritten (and the rewrite survived
    /// both structural and semantic verification). Used by the second pass
    /// to mark `has_unbounded_stack = false` on refreshed contracts.
    pub was_trmc_rewritten: bool,
}

/// Run the AIMS pipeline on a single function (steps 3–12).
///
/// Called from within `run_arc_pipeline` when the `aims` feature is active.
/// Interprocedural contracts must already be computed and passed via `config`.
pub(crate) fn run_aims_pipeline(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> AimsPipelineResult {
    // Steps 3–3a: compute var_reprs, detect immortals, normalize.
    // When TRMC rewrite fires (was_transformed), re-run from step 3
    // because new variables need ValueRepr entries and immortal detection.
    // The rewrite is idempotent — at most 2 iterations.
    //
    // `pre_trmc_func` is saved before TRMC rewrite for semantic rollback
    // (Bug 5: if post-analysis uniqueness verification fails, we restore
    // the pre-rewrite function and re-run analysis).
    let mut did_trmc_transform = false;
    let mut pre_trmc_func: Option<ArcFunction> = None;
    let (norm_result, immortals) = {
        let contract = config.contracts.get(&func.name);
        loop {
            // Step 3: compute value representations.
            {
                let _span = tracing::info_span!("compute_var_reprs").entered();
                func.var_reprs = crate::ir::compute_var_reprs(func, config.classifier, config.pool);
            }

            // Step 3.5: detect immortal variables.
            let immortals = detect_immortals(func, config);

            // Step 3a: normalize — detect + rewrite TRMC context regions.
            // Save pre-rewrite state for semantic rollback.
            let saved = func.clone();
            let norm_result = {
                let _span = tracing::info_span!("normalize_function").entered();
                crate::aims::normalize::normalize_function(func, contract)
            };

            if norm_result.was_transformed {
                did_trmc_transform = true;
                pre_trmc_func = Some(saved);
                tracing::debug!(
                    func = func.name.raw(),
                    "TRMC rewrite applied, re-running var_reprs and immortals"
                );
                continue;
            }

            break (norm_result, immortals);
        }
    };

    // Intraprocedural analysis → converged state map.
    let state_map = {
        let _span = tracing::info_span!("analyze_function").entered();
        crate::aims::intraprocedural::analyze_function(
            func,
            config.classifier,
            config.contracts,
            &norm_result.context_regions,
            immortals,
        )
    };

    // Step 4a: TRMC semantic soundness verification.
    let (state_map, trmc_rewrite_survived) =
        verify_trmc_soundness(func, state_map, did_trmc_transform, pre_trmc_func, config);

    // Phase 1: RC + reuse + arg_ownership (pre-merge).
    let mut result = {
        let _span = tracing::info_span!("realize_rc_reuse").entered();
        crate::aims::realize::realize_rc_reuse(
            func,
            &state_map,
            config.contracts,
            config.interner,
            config.builtins,
            config.pool,
        )
    };

    // Post-emission missed_reuses count for the second pass (FP² Theorem 2).
    let missed_reuses = result.fip_evidence.missed_reuses;

    // Step 5a: FIP enforcement pre-check (Section 12.3).
    // Cross-checks FipContract against realization evidence. At this point,
    // the contract has optimistic may_deallocate=false from interprocedural
    // analysis — `CertifiedButHasMissedReuses` mismatches are expected and
    // will be corrected by the second pass. But structural violations
    // (`CertifiedButUnboundedStack`, `BoundedExceeded`) are genuine bugs
    // that should be caught immediately.
    if let Some(contract) = config.contracts.get(&func.name) {
        let fip_errors = crate::aims::verify::fip::verify_fip_contract(
            func.name,
            contract,
            &result.fip_evidence,
        );
        for e in &fip_errors {
            use crate::aims::verify::fip::FipVerificationError;
            match e {
                FipVerificationError::CertifiedButHasMissedReuses { .. } => {
                    // Expected: may_deallocate is stale (optimistic default).
                    // Second pass will recompute contract.fip and re-verify.
                    tracing::debug!("FIP pre-check (will recompute in second pass): {e}");
                }
                FipVerificationError::CertifiedButUnboundedStack { .. }
                | FipVerificationError::BoundedExceeded { .. } => {
                    // Genuine bug: structural violations are known at
                    // interprocedural analysis time, not post-emission facts.
                    tracing::error!("FIP verification failed: {e}");
                    debug_assert!(false, "FIP verification failed: {e}");
                }
            }
        }
    }

    // Set canonicalize cross-dim fires from converged state analysis.
    result.synergy_metrics.canonicalize_cross_fires = state_map.count_cross_dim_states();

    // Verify, AIMS-verify, tail calls, merge.
    verify_and_merge(func, config);

    // Phase 2: COW + drop hints (post-merge).
    {
        let _span = tracing::info_span!("realize_annotations").entered();
        crate::aims::realize::realize_annotations(
            func,
            &state_map,
            config.interner,
            config.pool,
            &mut result,
        );
    }
    func.cow_annotations = result.cow_annotations;
    func.drop_hints = result.drop_hints;

    // Final verification + FBIP.
    let problems = emit_postprocess(func, config);

    AimsPipelineResult {
        problems,
        missed_reuses,
        was_trmc_rewritten: trmc_rewrite_survived,
    }
}

/// Step 4a: TRMC semantic soundness verification.
///
/// After analysis converges, verify that context variables are Unique
/// at all Set sites. On failure, roll back to pre-rewrite function and
/// re-run analysis on the restored version.
///
/// Returns `(state_map, trmc_rewrite_survived)`.
fn verify_trmc_soundness(
    func: &mut ArcFunction,
    state_map: crate::aims::intraprocedural::AimsStateMap,
    did_trmc_transform: bool,
    pre_trmc_func: Option<ArcFunction>,
    config: &AimsPipelineConfig<'_>,
) -> (crate::aims::intraprocedural::AimsStateMap, bool) {
    if !did_trmc_transform {
        return (state_map, false);
    }

    let _span = tracing::info_span!("verify_trmc_soundness").entered();
    let errors = crate::aims::normalize::verify::verify_trmc_soundness(func, &state_map);
    if errors.is_empty() {
        tracing::debug!(func = func.name.raw(), "TRMC soundness verified");
        return (state_map, true);
    }

    for error in &errors {
        tracing::warn!("{error}");
    }
    tracing::warn!(
        func = func.name.raw(),
        errors = errors.len(),
        "TRMC soundness verification failed, rolling back"
    );

    // Restore pre-rewrite function and re-run analysis.
    if let Some(original) = pre_trmc_func {
        *func = original;
        func.var_reprs = crate::ir::compute_var_reprs(func, config.classifier, config.pool);
        let restored_immortals = detect_immortals(func, config);
        let restored_regions =
            crate::aims::normalize::normalize_function(func, None).context_regions;
        let restored_map = crate::aims::intraprocedural::analyze_function(
            func,
            config.classifier,
            config.contracts,
            &restored_regions,
            restored_immortals,
        );
        (restored_map, false)
    } else {
        (state_map, false)
    }
}

/// Detect immortal variables (heap-allocated constants with `MAX_REFCOUNT`).
fn detect_immortals(func: &ArcFunction, config: &AimsPipelineConfig<'_>) -> Vec<bool> {
    let _span = tracing::info_span!("detect_immortals").entered();
    let imm = crate::aims::immortal::detect_immortals(func, config.interner);
    let immortal_count = crate::aims::immortal::count_immortals(&imm);
    if immortal_count > 0 {
        tracing::debug!(
            function = func.name.raw(),
            immortal_count,
            "immortal variables detected"
        );
    }
    imm
}

/// Post-emission steps: final verify + FBIP.
fn emit_postprocess(func: &mut ArcFunction, config: &AimsPipelineConfig<'_>) -> Vec<ArcProblem> {
    {
        let _span = tracing::info_span!("verify_final").entered();
        super::run_verify(func, "after AIMS pipeline", config.verify_arc);
    }

    check_fbip(func, config)
}

/// Verify, AIMS-verify, detect tail calls, merge blocks.
fn verify_and_merge(func: &mut ArcFunction, config: &AimsPipelineConfig<'_>) {
    {
        let _span = tracing::info_span!("verify_post_emission").entered();
        super::run_verify(func, "after AIMS emission", config.verify_arc);
    }
    if let Some(contract) = config.contracts.get(&func.name) {
        let _span = tracing::info_span!("aims_verify").entered();
        super::run_aims_verify(func, contract, "after AIMS emission", config.verify_arc);
    }
    {
        let _span = tracing::info_span!("tail_calls").entered();
        func.tail_calls = crate::tail_call::detect_tail_calls(func);
        crate::tail_call::rewrite_tail_calls(func);
    }
    {
        let _span = tracing::info_span!("merge_blocks").entered();
        crate::block_merge::merge_blocks(func);
    }
}

/// Check FBIP enforcement and auto-FBIP detection (Step 14).
fn check_fbip(func: &ArcFunction, config: &AimsPipelineConfig<'_>) -> Vec<ArcProblem> {
    let mut problems = Vec::new();
    if func.is_fbip {
        let func_name = config.interner.lookup(func.name);
        let func_span = func
            .spans
            .first()
            .and_then(|block_spans| block_spans.first().copied().flatten())
            .unwrap_or(ori_ir::Span::DUMMY);
        if let Some(problem) =
            crate::fbip::check_fbip_enforcement(func, config.classifier, func_name, func_span)
        {
            problems.push(problem);
        }
    }

    if crate::fbip::is_auto_fbip(func) {
        let func_name = config.interner.lookup(func.name);
        tracing::debug!(
            function = func_name,
            cow_ops = func.cow_annotations.len(),
            "auto FBIP: all COW operations are StaticUnique"
        );
    }

    problems
}

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
    // Step 1: interprocedural analysis → MemoryContract per function.
    let mut contracts = {
        let _span = tracing::info_span!("analyze_program").entered();
        crate::aims::interprocedural::analyze_program(functions, classifier, builtins, interner)
    };

    // Step 2: apply ownership to function parameters.
    {
        let _span = tracing::info_span!("apply_ownership").entered();
        apply_aims_ownership(functions, &contracts);
    }

    // Steps 3–14: per-function pipeline.
    let config = AimsPipelineConfig {
        classifier,
        contracts: &contracts,
        pool,
        interner,
        builtins,
        verify_arc,
    };

    let mut all_problems = Vec::new();
    let mut total_rc = super::rc_count::RcOpCount::default();
    // Collect post-emission missed_reuses for second-pass FIP recomputation.
    // Preserves the full count (not just bool) so Bounded(n) contracts can
    // be re-verified with accurate evidence.
    let mut reuse_updates: Vec<(Name, usize)> = Vec::new();
    // Track TRMC-rewritten functions for contract refresh (Bug 2).
    let mut trmc_rewritten: Vec<Name> = Vec::new();
    for func in functions.iter_mut() {
        let result = run_aims_pipeline(func, &config);
        all_problems.extend(result.problems);
        reuse_updates.push((func.name, result.missed_reuses));
        if result.was_trmc_rewritten {
            trmc_rewritten.push(func.name);
        }
        let rc = super::rc_count::count_rc_ops(func);
        total_rc.inc += rc.inc;
        total_rc.dec += rc.dec;
    }

    // Second pass: TRMC contract refresh → may_deallocate → FIP.
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
