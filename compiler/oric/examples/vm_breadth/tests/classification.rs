//! Per-lane classification-surface pins.
//!
//! The evaluator lane and the VM lane each carry a closed disposition set, and the
//! evaluator-versus-plan parity lane carries a third state the generic-versus-physical
//! backend lane cannot express. These pins fail if any lane is widened, merged with a
//! sibling lane, or re-mapped onto a different downstream classification.

use std::collections::BTreeSet;

use crate::contract::{
    Availability, BackendParityDifference, BackendParityStatus, FrontendStatus, OracleObservation,
    OracleStatus, OracleWorkerDisposition, OracleWorkerRecord, ParityObservation, ParityReason,
    ParityStatus, Phase, PhysicalPlanMetricsStatus, PlanClassification, PlanKind, PlanObservation,
    UnavailableReason, VmWorkerDisposition, VmWorkerRecord, SCHEMA_VERSION,
};
use crate::errors::{ErrorKind, ErrorRecord, HarnessErrorKind};

const VM_DISPOSITIONS: [VmWorkerDisposition; 9] = [
    VmWorkerDisposition::Success,
    VmWorkerDisposition::FrontendRejected,
    VmWorkerDisposition::Unsupported,
    VmWorkerDisposition::RealizationError,
    VmWorkerDisposition::CompileError,
    VmWorkerDisposition::VerifierError,
    VmWorkerDisposition::PhysicalPrepareError,
    VmWorkerDisposition::RuntimeError,
    VmWorkerDisposition::InternalError,
];

const ORACLE_DISPOSITIONS: [OracleWorkerDisposition; 4] = [
    OracleWorkerDisposition::Success,
    OracleWorkerDisposition::FrontendRejected,
    OracleWorkerDisposition::RuntimeError,
    OracleWorkerDisposition::InternalError,
];

const PARITY_STATUSES: [ParityStatus; 3] = [
    ParityStatus::Equal,
    ParityStatus::Different,
    ParityStatus::Unavailable,
];

const BACKEND_PARITY_STATUSES: [BackendParityStatus; 2] =
    [BackendParityStatus::Equal, BackendParityStatus::Different];

/// Phases the evaluator never reaches, so their wire tokens are VM-lane exclusive.
const VM_ONLY_TOKENS: [&str; 5] = [
    "unsupported",
    "realization_error",
    "compile_error",
    "verifier_error",
    "physical_prepare_error",
];

/// Position in `VM_DISPOSITIONS`; a new variant fails to compile here.
const fn vm_disposition_index(disposition: VmWorkerDisposition) -> usize {
    match disposition {
        VmWorkerDisposition::Success => 0,
        VmWorkerDisposition::FrontendRejected => 1,
        VmWorkerDisposition::Unsupported => 2,
        VmWorkerDisposition::RealizationError => 3,
        VmWorkerDisposition::CompileError => 4,
        VmWorkerDisposition::VerifierError => 5,
        VmWorkerDisposition::PhysicalPrepareError => 6,
        VmWorkerDisposition::RuntimeError => 7,
        VmWorkerDisposition::InternalError => 8,
    }
}

/// Position in `ORACLE_DISPOSITIONS`; a new variant fails to compile here.
const fn oracle_disposition_index(disposition: OracleWorkerDisposition) -> usize {
    match disposition {
        OracleWorkerDisposition::Success => 0,
        OracleWorkerDisposition::FrontendRejected => 1,
        OracleWorkerDisposition::RuntimeError => 2,
        OracleWorkerDisposition::InternalError => 3,
    }
}

/// Position in `PARITY_STATUSES`; a new variant fails to compile here.
const fn parity_status_index(status: ParityStatus) -> usize {
    match status {
        ParityStatus::Equal => 0,
        ParityStatus::Different => 1,
        ParityStatus::Unavailable => 2,
    }
}

/// Position in `BACKEND_PARITY_STATUSES`; a new variant fails to compile here.
const fn backend_parity_status_index(status: BackendParityStatus) -> usize {
    match status {
        BackendParityStatus::Equal => 0,
        BackendParityStatus::Different => 1,
    }
}

#[test]
fn classification_enums_enumerate_every_variant_exactly_once() {
    assert_permutation(
        &VM_DISPOSITIONS.map(vm_disposition_index),
        "VmWorkerDisposition",
    );
    assert_permutation(
        &ORACLE_DISPOSITIONS.map(oracle_disposition_index),
        "OracleWorkerDisposition",
    );
    assert_permutation(&PARITY_STATUSES.map(parity_status_index), "ParityStatus");
    assert_permutation(
        &BACKEND_PARITY_STATUSES.map(backend_parity_status_index),
        "BackendParityStatus",
    );
}

#[test]
fn oracle_lane_cannot_express_vm_phase_dispositions() {
    let vm_tokens = wire_tokens(&VM_DISPOSITIONS);
    let oracle_tokens = wire_tokens(&ORACLE_DISPOSITIONS);
    assert_eq!(vm_tokens.len(), VM_DISPOSITIONS.len());
    assert_eq!(oracle_tokens.len(), ORACLE_DISPOSITIONS.len());
    assert!(
        oracle_tokens.is_subset(&vm_tokens),
        "the evaluator lane must name its shared outcomes identically to the VM lane"
    );

    let vm_only: BTreeSet<String> = vm_tokens.difference(&oracle_tokens).cloned().collect();
    let expected: BTreeSet<String> = VM_ONLY_TOKENS.iter().map(|&t| t.to_owned()).collect();
    assert_eq!(
        vm_only, expected,
        "the VM lane classifies exactly the phases the evaluator never reaches"
    );
}

#[test]
fn oracle_worker_record_rejects_vm_lane_disposition_tokens() {
    let record = OracleWorkerRecord {
        schema_version: SCHEMA_VERSION,
        disposition: OracleWorkerDisposition::Success,
        phase: Phase::Eval,
        frontend_status: FrontendStatus::Valid,
        result: Availability::unavailable(UnavailableReason::NoResultForError),
        error: Availability::unavailable(UnavailableReason::NoErrorForSuccess),
    };
    let mut encoded = match serde_json::to_value(&record) {
        Ok(value) => value,
        Err(error) => panic!("oracle worker record must serialize: {error}"),
    };
    assert!(
        serde_json::from_value::<OracleWorkerRecord>(encoded.clone()).is_ok(),
        "the evaluator lane must accept its own disposition tokens"
    );

    for token in VM_ONLY_TOKENS {
        let Some(object) = encoded.as_object_mut() else {
            panic!("oracle worker record must serialize as a JSON object");
        };
        object.insert(
            "disposition".to_owned(),
            serde_json::Value::String(token.to_owned()),
        );
        assert!(
            serde_json::from_value::<OracleWorkerRecord>(encoded.clone()).is_err(),
            "the evaluator lane must reject the VM-only disposition `{token}`"
        );
    }
}

#[test]
fn oracle_disposition_status_map_is_total_and_injective() {
    let expected = [
        (OracleWorkerDisposition::Success, OracleStatus::Success),
        (
            OracleWorkerDisposition::FrontendRejected,
            OracleStatus::FrontendRejected,
        ),
        (
            OracleWorkerDisposition::RuntimeError,
            OracleStatus::RuntimeError,
        ),
        (
            OracleWorkerDisposition::InternalError,
            OracleStatus::InternalError,
        ),
    ];
    assert_eq!(expected.len(), ORACLE_DISPOSITIONS.len());
    let mut statuses = BTreeSet::new();
    for (disposition, status) in expected {
        assert_eq!(
            crate::runner::oracle_status_for(disposition),
            status,
            "evaluator disposition {disposition:?} must classify as {status:?}"
        );
        assert!(
            statuses.insert(wire_token(&status)),
            "two evaluator dispositions collapsed onto {status:?}"
        );
    }
}

#[test]
fn vm_disposition_classification_map_is_total_and_injective() {
    let oracle = unavailable_oracle();
    let expected = [
        (
            VmWorkerDisposition::Success,
            PlanClassification::ComparisonUnavailable,
        ),
        (
            VmWorkerDisposition::FrontendRejected,
            PlanClassification::FrontendRejected,
        ),
        (
            VmWorkerDisposition::Unsupported,
            PlanClassification::Unsupported,
        ),
        (
            VmWorkerDisposition::RealizationError,
            PlanClassification::RealizationError,
        ),
        (
            VmWorkerDisposition::CompileError,
            PlanClassification::CompileError,
        ),
        (
            VmWorkerDisposition::VerifierError,
            PlanClassification::VerifierError,
        ),
        (
            VmWorkerDisposition::PhysicalPrepareError,
            PlanClassification::PhysicalPrepareError,
        ),
        (
            VmWorkerDisposition::RuntimeError,
            PlanClassification::RuntimeError,
        ),
        (
            VmWorkerDisposition::InternalError,
            PlanClassification::InternalError,
        ),
    ];
    assert_eq!(expected.len(), VM_DISPOSITIONS.len());
    let mut classifications = BTreeSet::new();
    for (disposition, classification) in expected {
        let (observed, _) = crate::runner::classify_plan(&oracle, &vm_record(disposition));
        assert_eq!(
            observed, classification,
            "VM disposition {disposition:?} must classify as {classification:?}"
        );
        assert!(
            classifications.insert(wire_token(&classification)),
            "two VM dispositions collapsed onto {classification:?}"
        );
    }
}

#[test]
fn parity_lane_carries_an_unavailable_state_the_backend_lane_omits() {
    let parity_tokens = wire_tokens(&PARITY_STATUSES);
    let backend_tokens = wire_tokens(&BACKEND_PARITY_STATUSES);
    assert!(parity_tokens.contains("unavailable"));
    assert!(
        !backend_tokens.contains("unavailable"),
        "generic-versus-physical parity is always computable and has no unavailable state"
    );
    assert_eq!(
        parity_tokens.difference(&backend_tokens).count(),
        1,
        "the evaluator parity lane adds exactly the unavailable state"
    );

    let (_, parity) = crate::runner::classify_plan(
        &unavailable_oracle(),
        &vm_record(VmWorkerDisposition::Unsupported),
    );
    assert_eq!(
        parity.status,
        ParityStatus::Unavailable,
        "an unsupported plan yields a parity state the backend lane cannot represent"
    );
}

#[test]
fn backend_parity_reports_every_differing_field() {
    let generic = plan_observation(PlanKind::Generic);
    let equal = crate::runner::backend_parity(&generic, &generic);
    assert_eq!(equal.status, BackendParityStatus::Equal);
    assert!(equal.differences.is_empty());

    let mut physical = plan_observation(PlanKind::Physical);
    physical.classification = PlanClassification::Unsupported;
    physical.fallback = true;
    let different = crate::runner::backend_parity(&generic, &physical);
    assert_eq!(different.status, BackendParityStatus::Different);
    assert_eq!(
        different.differences,
        vec![
            BackendParityDifference::Classification,
            BackendParityDifference::Fallback,
        ],
        "the backend lane enumerates every differing field, not one reason"
    );
}

fn assert_permutation(indices: &[usize], label: &str) {
    let mut sorted = indices.to_vec();
    sorted.sort_unstable();
    let expected: Vec<usize> = (0..indices.len()).collect();
    assert_eq!(
        sorted, expected,
        "{label} must enumerate every variant exactly once"
    );
}

fn wire_token<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(token)) => token,
        other => panic!("closed classification enums serialize as strings, got {other:?}"),
    }
}

fn wire_tokens<T: serde::Serialize>(values: &[T]) -> BTreeSet<String> {
    values.iter().map(wire_token).collect()
}

fn unavailable_oracle() -> OracleObservation {
    OracleObservation {
        status: OracleStatus::NotRequested,
        frontend_status: FrontendStatus::Unknown,
        phase: Phase::Supervisor,
        result: Availability::unavailable(UnavailableReason::OracleUnavailable),
        output: Availability::unavailable(UnavailableReason::OracleUnavailable),
        error: Availability::unavailable(UnavailableReason::OracleUnavailable),
        process: Availability::unavailable(UnavailableReason::ProcessNotSpawned),
    }
}

fn vm_record(disposition: VmWorkerDisposition) -> VmWorkerRecord {
    VmWorkerRecord {
        schema_version: SCHEMA_VERSION,
        disposition,
        phase: Phase::VmExecute,
        requested_plan: PlanKind::Generic,
        execution_entry: crate::contract::ExecutionEntry::NotEntered,
        executed_plan: None,
        fallback: false,
        result: Availability::unavailable(UnavailableReason::PhaseNotReached),
        output: Availability::unavailable(UnavailableReason::PhaseNotReached),
        bytecode_metrics: Availability::unavailable(UnavailableReason::PhaseNotReached),
        execution_metrics: Availability::unavailable(UnavailableReason::PhaseNotReached),
        physical_plan_metrics: PhysicalPlanMetricsStatus::Unavailable {
            reason: UnavailableReason::PhaseNotReached,
        },
        error: Availability::available(ErrorRecord::new(
            ErrorKind::Harness {
                kind: HarnessErrorKind::InconsistentWorkerRecord,
            },
            "worker failure".to_owned(),
            "worker failure",
        )),
    }
}

fn plan_observation(requested_plan: PlanKind) -> PlanObservation {
    PlanObservation {
        requested_plan,
        executed_plan: None,
        fallback: false,
        phase: Phase::VmExecute,
        classification: PlanClassification::ComparisonUnavailable,
        parity: ParityObservation {
            status: ParityStatus::Unavailable,
            reason: Some(ParityReason::OracleDidNotSucceed),
        },
        result: Availability::unavailable(UnavailableReason::PhaseNotReached),
        output: Availability::unavailable(UnavailableReason::PhaseNotReached),
        bytecode_metrics: Availability::unavailable(UnavailableReason::PhaseNotReached),
        execution_metrics: Availability::unavailable(UnavailableReason::PhaseNotReached),
        physical_plan_metrics: PhysicalPlanMetricsStatus::Unavailable {
            reason: UnavailableReason::PhaseNotReached,
        },
        error: Availability::unavailable(UnavailableReason::PhaseNotReached),
        process: Availability::unavailable(UnavailableReason::ProcessNotSpawned),
    }
}
