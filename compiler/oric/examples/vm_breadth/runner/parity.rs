//! Evaluator and backend-pair parity classification.

use crate::contract::{
    Availability, BackendParityDifference, BackendParityObservation, BackendParityStatus,
    OracleObservation, OracleStatus, ParityObservation, ParityReason, ParityStatus,
    PlanClassification, PlanObservation, VmWorkerDisposition, VmWorkerRecord,
};
use crate::digest::ByteArtifact;
use crate::values::{self, ValueComparison};

pub(super) fn classify_success(
    oracle: &OracleObservation,
    record: &VmWorkerRecord,
) -> (PlanClassification, ParityObservation) {
    if oracle.status == OracleStatus::RuntimeError {
        return (
            PlanClassification::Mismatch,
            different_parity(ParityReason::OracleErroredPlanSucceeded),
        );
    }
    if oracle.status != OracleStatus::Success {
        return (
            PlanClassification::ComparisonUnavailable,
            unavailable_parity(ParityReason::OracleDidNotSucceed),
        );
    }
    let (
        Availability::Available { value: eval_value },
        Availability::Available { value: vm_value },
        Availability::Available { value: eval_output },
        Availability::Available { value: vm_output },
    ) = (
        &oracle.result,
        &record.result,
        &oracle.output,
        &record.output,
    )
    else {
        return (
            PlanClassification::InternalError,
            unavailable_parity(ParityReason::PlanDidNotSucceed),
        );
    };
    parity_for(eval_value, vm_value, eval_output, vm_output)
}

pub(crate) fn classify_plan(
    oracle: &OracleObservation,
    record: &VmWorkerRecord,
) -> (PlanClassification, ParityObservation) {
    if record.fallback {
        return failed(PlanClassification::Fallback);
    }
    match record.disposition {
        VmWorkerDisposition::Success => classify_success(oracle, record),
        VmWorkerDisposition::FrontendRejected => failed(PlanClassification::FrontendRejected),
        VmWorkerDisposition::Unsupported => failed(PlanClassification::Unsupported),
        VmWorkerDisposition::RealizationError => failed(PlanClassification::RealizationError),
        VmWorkerDisposition::CompileError => failed(PlanClassification::CompileError),
        VmWorkerDisposition::VerifierError => failed(PlanClassification::VerifierError),
        VmWorkerDisposition::PhysicalPrepareError => {
            failed(PlanClassification::PhysicalPrepareError)
        }
        VmWorkerDisposition::RuntimeError => (
            PlanClassification::RuntimeError,
            classify_runtime(oracle, record),
        ),
        VmWorkerDisposition::InternalError => failed(PlanClassification::InternalError),
    }
}

pub(crate) fn classify_runtime(
    oracle: &OracleObservation,
    record: &VmWorkerRecord,
) -> ParityObservation {
    if oracle.status == OracleStatus::Success {
        return different_parity(ParityReason::OracleSucceededPlanErrored);
    }
    if oracle.status != OracleStatus::RuntimeError {
        return unavailable_parity(ParityReason::OracleDidNotSucceed);
    }
    let (
        Availability::Available {
            value: oracle_error,
        },
        Availability::Available { value: plan_error },
        Availability::Available {
            value: oracle_output,
        },
        Availability::Available { value: plan_output },
    ) = (&oracle.error, &record.error, &oracle.output, &record.output)
    else {
        return unavailable_parity(ParityReason::PlanDidNotSucceed);
    };
    match (
        oracle_error.message == plan_error.message,
        artifact_identity_equal(oracle_output, plan_output),
    ) {
        (true, true) => ParityObservation {
            status: ParityStatus::Equal,
            reason: None,
        },
        (true, false) => different_parity(ParityReason::OutputDiffers),
        (false, true) => different_parity(ParityReason::ErrorDiffers),
        (false, false) => different_parity(ParityReason::ErrorAndOutputDiffer),
    }
}

pub(crate) fn parity_for(
    eval_value: &crate::values::EvaluatorValue,
    vm_value: &crate::values::VmValue,
    eval_output: &ByteArtifact,
    vm_output: &ByteArtifact,
) -> (PlanClassification, ParityObservation) {
    let output_equal = artifact_identity_equal(eval_output, vm_output);
    match (values::compare(eval_value, vm_value), output_equal) {
        (ValueComparison::Equal, true) => (
            PlanClassification::VmCorrect,
            ParityObservation {
                status: ParityStatus::Equal,
                reason: None,
            },
        ),
        (ValueComparison::Equal, false) => different_classification(ParityReason::OutputDiffers),
        (ValueComparison::Different, true) => different_classification(ParityReason::ValuesDiffer),
        (ValueComparison::Different, false) => {
            different_classification(ParityReason::ValueAndOutputDiffer)
        }
        (ValueComparison::Unavailable, _) => (
            PlanClassification::ComparisonUnavailable,
            unavailable_parity(ParityReason::EvaluatorValueNotLosslesslyComparable),
        ),
    }
}

pub(crate) fn backend_parity(
    generic: &PlanObservation,
    physical: &PlanObservation,
) -> BackendParityObservation {
    let mut differences = Vec::new();
    record_difference(
        generic.classification != physical.classification,
        BackendParityDifference::Classification,
        &mut differences,
    );
    record_difference(
        generic.parity != physical.parity,
        BackendParityDifference::EvaluatorParity,
        &mut differences,
    );
    record_difference(
        generic.fallback != physical.fallback,
        BackendParityDifference::Fallback,
        &mut differences,
    );
    record_difference(
        generic.result != physical.result,
        BackendParityDifference::Result,
        &mut differences,
    );
    record_difference(
        generic.output != physical.output,
        BackendParityDifference::Output,
        &mut differences,
    );
    record_difference(
        generic.bytecode_metrics != physical.bytecode_metrics,
        BackendParityDifference::BytecodeMetrics,
        &mut differences,
    );
    record_difference(
        generic.execution_metrics != physical.execution_metrics,
        BackendParityDifference::ExecutionMetrics,
        &mut differences,
    );
    record_difference(
        generic.error != physical.error,
        BackendParityDifference::Error,
        &mut differences,
    );
    BackendParityObservation {
        status: if differences.is_empty() {
            BackendParityStatus::Equal
        } else {
            BackendParityStatus::Different
        },
        differences,
    }
}

fn record_difference(
    differs: bool,
    field: BackendParityDifference,
    differences: &mut Vec<BackendParityDifference>,
) {
    if differs {
        differences.push(field);
    }
}

fn artifact_identity_equal(left: &ByteArtifact, right: &ByteArtifact) -> bool {
    left.length_decimal == right.length_decimal && left.sha256 == right.sha256
}

fn different_classification(reason: ParityReason) -> (PlanClassification, ParityObservation) {
    (PlanClassification::Mismatch, different_parity(reason))
}

fn failed(classification: PlanClassification) -> (PlanClassification, ParityObservation) {
    (
        classification,
        unavailable_parity(ParityReason::PlanDidNotSucceed),
    )
}

fn different_parity(reason: ParityReason) -> ParityObservation {
    ParityObservation {
        status: ParityStatus::Different,
        reason: Some(reason),
    }
}

pub(super) fn unavailable_parity(reason: ParityReason) -> ParityObservation {
    ParityObservation {
        status: ParityStatus::Unavailable,
        reason: Some(reason),
    }
}
