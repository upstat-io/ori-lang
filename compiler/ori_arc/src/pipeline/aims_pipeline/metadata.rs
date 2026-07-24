//! Metadata checkpoints reject stored variable facts that disagree with fresh derivation.

use crate::ir::ArcFunction;

use super::{trace_pipeline_checkpoint, AimsPipelineConfig};

/// Validate stored variable representations and RC strategies against their derivations.
pub(crate) fn validate_variable_metadata(
    func: &ArcFunction,
    classifier: &dyn crate::ArcClassification,
    pool: &ori_types::Pool,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let mut errors = Vec::new();
    if func.var_metadata_state != crate::ir::VariableMetadataState::Realized {
        errors.push(crate::verify::VerifyError::VariableMetadataUnrealized);
    }
    let expected_representations = crate::ir::compute_var_reprs(func, classifier, pool);
    errors.extend(representation_metadata_errors(
        func,
        &expected_representations,
    ));

    let expected_strategies =
        crate::ir::derive_var_rc_strategies(&expected_representations, &func.var_types, pool);
    errors.extend(rc_strategy_metadata_errors(func, &expected_strategies));

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate variable metadata and notify the configured pipeline observer.
pub(super) fn validate_metadata_checkpoint(
    func: &ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    validate_variable_metadata(func, config.classifier, config.pool)?;
    trace_pipeline_checkpoint(
        func,
        "validate_variable_metadata",
        config.interner,
        config.observer,
    );
    Ok(())
}

/// Report representation-table length and per-variable value mismatches.
pub(crate) fn representation_metadata_errors(
    func: &ArcFunction,
    expected: &[crate::ir::ValueRepr],
) -> Vec<crate::verify::VerifyError> {
    use crate::verify::VerifyError;

    if func.var_reprs.len() == func.var_types.len() {
        expected
            .iter()
            .zip(&func.var_reprs)
            .enumerate()
            .filter(|(_, (expected, found))| expected != found)
            .map(
                |(index, (&expected, &found))| VerifyError::VariableRepresentationMismatch {
                    var: variable_id(index),
                    expected,
                    found,
                },
            )
            .collect()
    } else {
        vec![VerifyError::VariableMetadataLength {
            table: "representation",
            variables: func.var_types.len(),
            entries: func.var_reprs.len(),
        }]
    }
}

fn rc_strategy_metadata_errors(
    func: &ArcFunction,
    expected: &[Option<crate::ir::RcStrategy>],
) -> Vec<crate::verify::VerifyError> {
    use crate::verify::VerifyError;

    if func.var_rc_strategies.len() == func.var_types.len() {
        expected
            .iter()
            .zip(&func.var_rc_strategies)
            .enumerate()
            .filter(|(_, (expected, found))| expected != found)
            .map(
                |(index, (&expected, &found))| VerifyError::VariableRcStrategyMismatch {
                    var: variable_id(index),
                    expected,
                    found,
                },
            )
            .collect()
    } else {
        vec![VerifyError::VariableMetadataLength {
            table: "RC-strategy",
            variables: func.var_types.len(),
            entries: func.var_rc_strategies.len(),
        }]
    }
}

fn variable_id(index: usize) -> crate::ir::ArcVarId {
    crate::ir::ArcVarId::new(
        u32::try_from(index).unwrap_or_else(|_| panic!("variable index exceeds u32::MAX")),
    )
}
