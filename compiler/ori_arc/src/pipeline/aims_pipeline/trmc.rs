//! TRMC normalization and soundness verification.
//!
//! Contains the TRMC rewrite loop (steps 3–3a), semantic soundness
//! verification (step 4a), and immortal variable detection (step 3.5).

use super::AimsPipelineConfig;
use crate::ir::ArcFunction;

/// Steps 3–3a: compute `var_reprs`, detect immortals, normalize with TRMC.
///
/// When TRMC rewrite fires, re-run from step 3 because new variables need
/// `ValueRepr` entries and immortal detection. The rewrite is idempotent —
/// at most 2 iterations. `pre_trmc_func` is saved before TRMC rewrite for
/// semantic rollback.
pub(crate) fn normalize_with_trmc(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> (
    crate::aims::normalize::NormalizationResult,
    Vec<bool>,
    bool,
    Option<ArcFunction>,
) {
    let contract = config.contracts.get(&func.name);
    let mut did_trmc_transform = false;
    let mut pre_trmc_func: Option<ArcFunction> = None;
    let mut trmc_iterations: u32 = 0;

    let (norm_result, immortals) = loop {
        // Step 3: compute value representations.
        {
            let _span = tracing::info_span!("compute_var_reprs").entered();
            func.var_reprs = crate::ir::compute_var_reprs(func, config.classifier, config.pool);
            // BUG-04-104: cache RcStrategy alongside var_reprs so the AIMS
            // pre-walk PIN-6 `class_payload_of` population in
            // `intraprocedural::ssa_alias_classes` can classify a var's
            // strategy without holding a `&Pool` reference at analyze-time.
            func.var_rc_strategies = crate::ir::compute_var_rc_strategies(func, config.pool);
        }
        super::trace_pipeline_checkpoint(
            func,
            "compute_var_reprs",
            config.interner,
            config.observer,
        );

        // Step 3.5: detect immortal variables.
        let immortals = detect_immortals(func, config);
        super::trace_pipeline_checkpoint(
            func,
            "detect_immortals",
            config.interner,
            config.observer,
        );

        // Step 3a: normalize — detect + rewrite TRMC context regions.
        // Save pre-rewrite state for semantic rollback. Only clone on
        // the first iteration — subsequent iterations already have the
        // pre-TRMC state saved.
        let saved = if pre_trmc_func.is_none() {
            Some(func.clone())
        } else {
            None
        };
        let norm_result = {
            let _span = tracing::info_span!("normalize_function").entered();
            crate::aims::normalize::normalize_function(func, contract)
        };
        super::trace_pipeline_checkpoint(
            func,
            "normalize_function",
            config.interner,
            config.observer,
        );

        if norm_result.was_transformed {
            if let Some(saved) = saved {
                pre_trmc_func = Some(saved);
            }
            did_trmc_transform = true;
            trmc_iterations += 1;
            debug_assert!(
                trmc_iterations <= 2,
                "TRMC rewrite loop exceeded 2 iterations for {:?} — \
                 idempotency invariant violated",
                func.name,
            );
            tracing::debug!(
                func = func.name.raw(),
                iteration = trmc_iterations,
                "TRMC rewrite applied, re-running var_reprs and immortals"
            );
            continue;
        }

        break (norm_result, immortals);
    };

    (norm_result, immortals, did_trmc_transform, pre_trmc_func)
}

/// Step 4a: TRMC semantic soundness verification.
///
/// After analysis converges, verify that context variables are Unique
/// at all Set sites. On failure, roll back to pre-rewrite function and
/// re-run analysis on the restored version.
///
/// Returns `(state_map, trmc_rewrite_survived)`.
pub(crate) fn verify_trmc_soundness(
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
        // BUG-04-104: refresh PIN-6 strategy cache after TRMC rollback.
        func.var_rc_strategies = crate::ir::compute_var_rc_strategies(func, config.pool);
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
pub(crate) fn detect_immortals(func: &ArcFunction, config: &AimsPipelineConfig<'_>) -> Vec<bool> {
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
