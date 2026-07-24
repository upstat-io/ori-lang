//! Worker-protocol and process-boundary contract tests.

use std::error::Error;
use std::time::Duration;

use crate::contract::{
    Availability, BackendParityStatus, CaseRecord, ExecutionEntry, FrontendStatus,
    OracleObservation, OracleStatus, ParityReason, ParityStatus, Phase, PhysicalPlanMetricsStatus,
    PlanClassification, PlanKind, ProcessObservation, RecordKind, Termination, UnavailableReason,
    VmWorkerDisposition, VmWorkerRecord, SCHEMA_VERSION,
};
use crate::digest::ByteArtifact;
use crate::errors::{ErrorKind, ErrorRecord, HarnessErrorKind};
use crate::manifest;
use crate::supervisor::{Supervised, Supervisor, WorkerKind};

#[test]
fn spawned_worker_entry() -> Result<(), crate::app::AppError> {
    let Some(args) = crate::supervisor::test_worker_args() else {
        return Ok(());
    };
    crate::app::run(args)
}

#[test]
fn physical_prepare_failure_retains_phase_and_classification() {
    let record = failed_record(
        VmWorkerDisposition::PhysicalPrepareError,
        Phase::PhysicalPrepare,
        PlanKind::Physical,
        ErrorKind::PhysicalPrepare,
    );
    let observation =
        crate::runner::physical_observation(&unavailable_oracle(), successful_process(record));
    assert_eq!(observation.requested_plan, PlanKind::Physical);
    assert_eq!(observation.executed_plan, None);
    assert!(!observation.fallback);
    assert_eq!(observation.phase, Phase::PhysicalPrepare);
    assert_eq!(
        observation.classification,
        PlanClassification::PhysicalPrepareError
    );
    assert!(matches!(
        observation.error,
        Availability::Available {
            value: ErrorRecord {
                kind: ErrorKind::PhysicalPrepare,
                ..
            }
        }
    ));
}

#[test]
fn protocol_rejects_plan_identity_without_explicit_fallback() {
    let mut record = failed_record(
        VmWorkerDisposition::InternalError,
        Phase::VmExecute,
        PlanKind::Physical,
        ErrorKind::Harness {
            kind: HarnessErrorKind::InconsistentWorkerRecord,
        },
    );
    record.execution_entry = ExecutionEntry::Entered;
    record.executed_plan = Some(PlanKind::Generic);
    let observation =
        crate::runner::physical_observation(&unavailable_oracle(), successful_process(record));
    assert_eq!(
        observation.classification,
        PlanClassification::InternalError
    );
    assert_eq!(observation.phase, Phase::VmExecute);
    let Availability::Available { value: error } = observation.error else {
        panic!("invalid provenance must publish a typed error");
    };
    assert_eq!(
        error.kind,
        ErrorKind::Harness {
            kind: HarnessErrorKind::InconsistentWorkerRecord
        }
    );
    assert!(error.message.contains("without an explicit fallback"));
}

#[test]
fn runtime_parity_compares_error_message_and_exact_output_prefix() {
    let output = ByteArtifact::complete(b"before failure\n");
    let oracle = runtime_oracle("division by zero", output.clone());
    let mut record = failed_record(
        VmWorkerDisposition::RuntimeError,
        Phase::VmExecute,
        PlanKind::Generic,
        ErrorKind::VmExecution {
            kind: crate::errors::ExecutionErrorKind::IntegerOperation,
        },
    );
    record.error = Availability::available(ErrorRecord::new(
        ErrorKind::VmExecution {
            kind: crate::errors::ExecutionErrorKind::IntegerOperation,
        },
        "division by zero".to_owned(),
        "backend-specific debug payload",
    ));
    record.output = Availability::available(output.clone());
    let parity = crate::runner::classify_runtime(&oracle, &record);
    assert_eq!(parity.status, ParityStatus::Equal);
    assert_eq!(parity.reason, None);

    record.output = Availability::available(ByteArtifact::complete(b"different prefix\n"));
    let parity = crate::runner::classify_runtime(&oracle, &record);
    assert_eq!(parity.reason, Some(ParityReason::OutputDiffers));

    record.error = Availability::available(ErrorRecord::new(
        ErrorKind::VmExecution {
            kind: crate::errors::ExecutionErrorKind::IntegerOperation,
        },
        "different error".to_owned(),
        "backend-specific debug payload",
    ));
    record.output = Availability::available(output);
    let parity = crate::runner::classify_runtime(&oracle, &record);
    assert_eq!(parity.reason, Some(ParityReason::ErrorDiffers));
}

#[test]
fn crashed_worker_uses_process_identity_when_record_is_missing() {
    for termination in [
        Termination::Exited {
            code_decimal: "101".to_owned(),
        },
        Termination::Signaled {
            signal_decimal: "6".to_owned(),
        },
    ] {
        let process = process_observation(termination, b"compiler invariant panic");
        let observation = crate::runner::generic_observation(
            &unavailable_oracle(),
            Supervised {
                process: process.clone(),
                record: None,
                protocol_error: Some(worker_record_error(HarnessErrorKind::WorkerRecordMissing)),
            },
        );
        assert_eq!(
            observation.classification,
            PlanClassification::InternalError
        );
        assert_eq!(observation.executed_plan, None);
        let Availability::Available { value: error } = &observation.error else {
            panic!("process crash must carry a typed error record");
        };
        assert_eq!(
            error.kind,
            ErrorKind::Harness {
                kind: HarnessErrorKind::ProcessCrash
            }
        );
        assert!(error.message.contains("captured stderr artifact"));
        assert_eq!(observation.process, Availability::available(process));
    }
}

#[test]
fn successful_worker_without_record_is_a_protocol_failure() {
    let process = process_observation(
        Termination::Exited {
            code_decimal: "0".to_owned(),
        },
        b"",
    );
    let observation = crate::runner::generic_observation(
        &unavailable_oracle(),
        Supervised {
            process: process.clone(),
            record: None,
            protocol_error: Some(worker_record_error(HarnessErrorKind::WorkerRecordMissing)),
        },
    );
    assert_eq!(
        observation.classification,
        PlanClassification::InternalError
    );
    assert_eq!(observation.executed_plan, None);
    assert!(matches!(
        observation.error,
        Availability::Available {
            value: ErrorRecord {
                kind: ErrorKind::Harness {
                    kind: HarnessErrorKind::WorkerRecordMissing,
                },
                ..
            }
        }
    ));
    assert_eq!(observation.process, Availability::available(process));
}

#[test]
fn spawn_and_wait_failures_keep_distinct_process_identities() {
    for (termination, expected) in [
        (
            Termination::SpawnFailed {
                message: "permission denied".to_owned(),
            },
            HarnessErrorKind::ProcessSpawn,
        ),
        (
            Termination::WaitFailed {
                message: "waitpid failed".to_owned(),
            },
            HarnessErrorKind::ProcessWait,
        ),
    ] {
        let process = process_observation(termination, b"");
        let observation = crate::runner::generic_observation(
            &unavailable_oracle(),
            Supervised {
                process: process.clone(),
                record: None,
                protocol_error: Some(worker_record_error(HarnessErrorKind::WorkerRecordMissing)),
            },
        );
        let Availability::Available { value: error } = &observation.error else {
            panic!("process failure must carry a typed error record");
        };
        assert_eq!(error.kind, ErrorKind::Harness { kind: expected });
        assert_eq!(observation.process, Availability::available(process));
    }
}

#[test]
fn real_cli_process_protocol_emits_backend_parity_jsonl() -> Result<(), Box<dyn Error>> {
    let manifest = manifest::load(
        &manifest::default_manifest_path(),
        &manifest::repository_root(),
    )?;
    let (manifest_index, case) = manifest
        .document
        .cases
        .iter()
        .enumerate()
        .find(|(_, case)| case.path == "tests/benchmarks/bench_hello.ori")
        .ok_or("bench_hello.ori missing from frozen manifest")?;
    let source = manifest.root.join(&case.path);
    let supervisor = Supervisor::new(Duration::from_secs(30))?;
    let oracle = unavailable_oracle();
    let generic = crate::runner::generic_observation(
        &oracle,
        supervisor.run::<VmWorkerRecord>(WorkerKind::Generic, &source),
    );
    let physical = crate::runner::physical_observation(
        &oracle,
        supervisor.run::<VmWorkerRecord>(WorkerKind::Physical, &source),
    );
    assert_eq!(generic.executed_plan, Some(PlanKind::Generic));
    assert_eq!(physical.executed_plan, Some(PlanKind::Physical));
    assert_eq!(generic.phase, Phase::VmExecute);
    assert_eq!(physical.phase, Phase::PhysicalExecute);
    let generic_physical_parity = crate::runner::backend_parity(&generic, &physical);
    assert_eq!(generic_physical_parity.status, BackendParityStatus::Equal);

    let record = CaseRecord {
        schema_version: SCHEMA_VERSION,
        manifest_index,
        path: case.path.clone(),
        source_sha256: case.source_sha256.clone(),
        frozen_frontend_status: case.frontend_status,
        frozen_vm_status: case.vm_status,
        frozen_oracle_status: case.oracle_status,
        oracle,
        plans: vec![generic, physical],
        generic_physical_parity,
    };
    let mut jsonl = Vec::new();
    crate::runner::write_jsonl(&mut jsonl, RecordKind::Case, &record)?;
    let decoded: serde_json::Value = serde_json::from_slice(&jsonl)?;
    assert_eq!(decoded["record"], "case");
    assert_eq!(decoded["plans"][0]["fallback"], false);
    assert_eq!(decoded["plans"][1]["phase"], "physical_execute");
    assert_eq!(decoded["generic_physical_parity"]["status"], "equal");
    assert!(decoded["plans"][1]["process"]
        .get("value")
        .and_then(|value| value.get("sampled_peak_rss_kib_lower_bound_decimal"))
        .is_some());
    Ok(())
}

fn failed_record(
    disposition: VmWorkerDisposition,
    phase: Phase,
    requested_plan: PlanKind,
    kind: ErrorKind,
) -> VmWorkerRecord {
    VmWorkerRecord {
        schema_version: SCHEMA_VERSION,
        disposition,
        phase,
        requested_plan,
        execution_entry: ExecutionEntry::NotEntered,
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
            kind,
            "worker failure".to_owned(),
            "worker failure",
        )),
    }
}

fn successful_process<T>(record: T) -> Supervised<T> {
    Supervised {
        process: process_observation(
            Termination::Exited {
                code_decimal: "0".to_owned(),
            },
            b"",
        ),
        record: Some(record),
        protocol_error: None,
    }
}

fn process_observation(termination: Termination, stderr: &[u8]) -> ProcessObservation {
    ProcessObservation {
        termination,
        elapsed_ns_decimal: Availability::available("1".to_owned()),
        sampled_peak_rss_kib_lower_bound_decimal: Availability::unavailable(
            UnavailableReason::ObservationMissed,
        ),
        stdout: ByteArtifact::complete(&[]),
        stderr: ByteArtifact::complete(stderr),
    }
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

fn runtime_oracle(message: &str, output: ByteArtifact) -> OracleObservation {
    OracleObservation {
        status: OracleStatus::RuntimeError,
        frontend_status: FrontendStatus::Valid,
        phase: Phase::Eval,
        result: Availability::unavailable(UnavailableReason::NoResultForError),
        output: Availability::available(output),
        error: Availability::available(ErrorRecord::new(
            ErrorKind::EvaluatorRuntime {
                kind_name: "ArithmeticError".to_owned(),
                error_code: "1".to_owned(),
            },
            message.to_owned(),
            "evaluator-specific debug payload",
        )),
        process: Availability::unavailable(UnavailableReason::ProcessNotSpawned),
    }
}

fn worker_record_error(kind: HarnessErrorKind) -> ErrorRecord {
    ErrorRecord::new(
        ErrorKind::Harness { kind },
        "worker record unavailable".to_owned(),
        "worker record unavailable",
    )
}
