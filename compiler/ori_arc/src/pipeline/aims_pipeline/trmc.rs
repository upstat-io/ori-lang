//! TRMC normalization and soundness verification.
//!
//! Owns TRMC normalization, immortal-variable detection, semantic verification,
//! and rollback when a rewrite fails its soundness gates.

use super::AimsPipelineConfig;
use crate::aims::contract::ContractMapExt;
use crate::ir::ArcFunction;

type NormalizationOutput = (
    crate::aims::normalize::NormalizationResult,
    Vec<bool>,
    bool,
    Option<ArcFunction>,
);

/// Computes variable representations and immortals, then normalizes TRMC regions.
///
/// When TRMC rewrite fires, re-run from step 3 because new variables need
/// `ValueRepr` entries and immortal detection. The rewrite is idempotent —
/// at most 2 iterations. `pre_trmc_func` is saved before TRMC rewrite for
/// semantic rollback.
pub(crate) fn normalize_with_trmc(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<NormalizationOutput, Vec<crate::verify::VerifyError>> {
    let contract = config.contracts.get_required(&func.name, "trmc_entry");
    let mut did_trmc_transform = false;
    let mut pre_trmc_func: Option<ArcFunction> = None;
    let mut trmc_iterations: u32 = 0;

    let (norm_result, immortals) = loop {
        {
            let _span = tracing::info_span!("compute_var_reprs").entered();
            validate_or_realize_variable_metadata(func, config.classifier, config.pool)?;
        }
        super::trace_pipeline_checkpoint(
            func,
            "compute_var_reprs",
            config.interner,
            config.observer,
        );

        let immortals = detect_immortals(func, config);
        super::trace_pipeline_checkpoint(
            func,
            "detect_immortals",
            config.interner,
            config.observer,
        );

        // Why: Soundness rollback requires the original function, not an intermediate rewrite.
        let saved = if pre_trmc_func.is_none() {
            Some(func.clone())
        } else {
            None
        };
        let norm_result = {
            let _span = tracing::info_span!("normalize_function").entered();
            crate::aims::normalize::normalize_function(func, Some(contract))
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
            assert!(
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

    Ok((norm_result, immortals, did_trmc_transform, pre_trmc_func))
}

/// Verifies TRMC semantic soundness after analysis converges.
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
    let mut errors = crate::aims::normalize::verify::verify_trmc_soundness(func, &state_map);
    // §PL-10 structural verify + §VF-7 tier (a) — burden-balance is the same
    // tier of structural well-formedness as Uniqueness; failure rolls back
    // the TRMC rewrite through the same path as a Uniqueness failure.
    errors.extend(crate::aims::normalize::verify::verify_trmc_burden_balance(
        func, &state_map,
    ));
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

/// Validate the authoritative metadata tables and perform only a legal
/// forward lifecycle transition. Existing ready or realized data is never
/// silently recomputed over a producer defect.
fn validate_or_realize_variable_metadata(
    func: &mut ArcFunction,
    classifier: &dyn crate::ArcClassification,
    pool: &ori_types::Pool,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    use crate::ir::VariableMetadataState;
    use crate::verify::VerifyError;

    match func.var_metadata_state {
        VariableMetadataState::Unrealized => {
            if !func.var_reprs.is_empty() || !func.var_rc_strategies.is_empty() {
                return Err(vec![VerifyError::VariableMetadataUnrealized]);
            }
            let representations = crate::ir::compute_var_reprs(func, classifier, pool);
            let strategies =
                crate::ir::derive_var_rc_strategies(&representations, &func.var_types, pool);
            func.replace_realized_variable_metadata(representations, strategies);
            Ok(())
        }
        VariableMetadataState::RepresentationsReady => {
            let expected = crate::ir::compute_var_reprs(func, classifier, pool);
            let mut errors = super::representation_metadata_errors(func, &expected);
            if !func.var_rc_strategies.is_empty() {
                errors.push(VerifyError::VariableMetadataUnexpectedEntries {
                    table: "representation-ready RC-strategy",
                    entries: func.var_rc_strategies.len(),
                });
            }
            if !errors.is_empty() {
                return Err(errors);
            }
            let strategies =
                crate::ir::derive_var_rc_strategies(&func.var_reprs, &func.var_types, pool);
            func.complete_variable_metadata(strategies);
            Ok(())
        }
        VariableMetadataState::Realized => {
            super::validate_variable_metadata(func, classifier, pool)
        }
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

#[cfg(test)]
mod tests {
    use super::validate_or_realize_variable_metadata;
    use crate::ir::{ArcFunction, ValueRepr, VariableMetadataState};
    use ori_types::{Idx, Pool};

    #[test]
    fn representation_ready_corruption_fails_without_silent_repair() {
        let pool = Pool::new();
        let classifier = crate::ArcClassifier::new(&pool);
        let mut function = ArcFunction {
            var_types: vec![Idx::STR],
            var_reprs: vec![ValueRepr::Scalar],
            var_metadata_state: VariableMetadataState::RepresentationsReady,
            ..ArcFunction::default()
        };

        let result = validate_or_realize_variable_metadata(&mut function, &classifier, &pool);
        let Err(errors) = result else {
            panic!("corrupt representation-ready metadata must fail");
        };

        assert!(errors.iter().any(|error| matches!(
            error,
            crate::verify::VerifyError::VariableRepresentationMismatch {
                expected: ValueRepr::FatValue,
                found: ValueRepr::Scalar,
                ..
            }
        )));
        assert_eq!(function.var_reprs, [ValueRepr::Scalar]);
        assert!(function.var_rc_strategies.is_empty());
        assert_eq!(
            function.var_metadata_state,
            VariableMetadataState::RepresentationsReady
        );
    }
}
