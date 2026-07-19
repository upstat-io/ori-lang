//! Deterministic corpus runner.

#[path = "runner/output.rs"]
mod output;
#[path = "runner/parity.rs"]
mod parity;
#[path = "runner/process.rs"]
mod process;
#[path = "runner/protocol.rs"]
mod protocol;
#[path = "runner/summary.rs"]
mod summary;

pub(crate) use output::{write_jsonl, RunnerError};
pub(crate) use parity::backend_parity;
use parity::{classify_plan, unavailable_parity};
#[cfg(test)]
pub(crate) use parity::{classify_runtime, parity_for};
use process::{failed_termination_error, termination_succeeded};
use protocol::validate_worker_record;
pub(crate) use summary::build_summary;

use std::io::Write;
use std::time::Duration;

use crate::contract::{
    Availability, CaseRecord, CompilerIdentity, FrontendStatus, FrozenFrontendStatus,
    FrozenVmStatus, OracleCounts, OracleObservation, OraclePolicy, OracleStatus,
    OracleWorkerDisposition, OracleWorkerRecord, ParityReason, Phase, PhysicalPlanMetricsStatus,
    PlanClassification, PlanCounts, PlanKind, PlanObservation, ProcessObservation, RecordKind,
    RunRecord, Termination, UnavailableReason, VmWorkerRecord, SCHEMA_VERSION,
};
use crate::digest::ByteArtifact;
use crate::errors::{ErrorKind, ErrorRecord, HarnessErrorKind};
use crate::manifest::{LoadedManifest, ManifestCase};
use crate::supervisor::{Supervised, Supervisor, WorkerKind};

pub(crate) fn execute<W: Write>(
    manifest: &LoadedManifest,
    timeout: Duration,
    oracle_policy: OraclePolicy,
    writer: &mut W,
) -> Result<(), RunnerError> {
    let supervisor = Supervisor::new(timeout)?;
    let run = RunRecord {
        schema_version: SCHEMA_VERSION,
        corpus_id: manifest.document.corpus_id.clone(),
        manifest_sha256: manifest.raw_sha256.clone(),
        compiler: CompilerIdentity {
            package_version: env!("CARGO_PKG_VERSION").to_owned(),
            executable_sha256: supervisor.executable_sha256().to_owned(),
            vcs_head: Availability::unavailable(UnavailableReason::VcsIdentityNotEmbedded),
            vcs_diff_sha256: Availability::unavailable(UnavailableReason::VcsIdentityNotEmbedded),
        },
        timeout_ms_decimal: timeout.as_millis().to_string(),
        requested_plans: vec![PlanKind::Generic, PlanKind::Physical],
        requested_oracle_policy: oracle_policy,
        executed_oracle_policy: oracle_policy,
    };
    write_jsonl(writer, RecordKind::Run, &run)?;

    let mut oracle_counts = OracleCounts::default();
    let mut generic_counts = PlanCounts::default();
    let mut physical_counts = PlanCounts::default();
    for (manifest_index, case) in manifest.document.cases.iter().enumerate() {
        let source_path = manifest.root.join(&case.path);
        let oracle = if oracle_requested(oracle_policy, case) {
            oracle_observation(
                supervisor.run::<OracleWorkerRecord>(WorkerKind::Evaluator, &source_path),
            )
        } else {
            oracle_not_requested()
        };
        increment_oracle(&mut oracle_counts, oracle.status);
        let generic = if case.frontend_status == FrozenFrontendStatus::Rejected {
            not_run_plan(PlanKind::Generic)
        } else {
            generic_observation(
                &oracle,
                supervisor.run::<VmWorkerRecord>(WorkerKind::Generic, &source_path),
            )
        };
        increment_plan(&mut generic_counts, generic.classification);
        let physical = if case.frontend_status == FrozenFrontendStatus::Rejected {
            not_run_plan(PlanKind::Physical)
        } else {
            physical_observation(
                &oracle,
                supervisor.run::<VmWorkerRecord>(WorkerKind::Physical, &source_path),
            )
        };
        increment_plan(&mut physical_counts, physical.classification);
        let generic_physical_parity = backend_parity(&generic, &physical);
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
        write_jsonl(writer, RecordKind::Case, &record)?;
    }
    let summary = build_summary(
        manifest,
        oracle_policy,
        oracle_counts,
        generic_counts,
        physical_counts,
    );
    write_jsonl(writer, RecordKind::Summary, &summary)
}

fn oracle_requested(policy: OraclePolicy, case: &ManifestCase) -> bool {
    case.frontend_status == FrozenFrontendStatus::Valid
        && match policy {
            OraclePolicy::Historical48 => case.vm_status == FrozenVmStatus::FullVm,
            OraclePolicy::AllFrontendValid => true,
        }
}

fn oracle_not_requested() -> OracleObservation {
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

fn oracle_observation(supervised: Supervised<OracleWorkerRecord>) -> OracleObservation {
    let Supervised {
        process,
        record,
        protocol_error,
    } = supervised;
    let output = Availability::available(process.stdout.clone());
    if matches!(process.termination, Termination::TimedOut { .. }) {
        return OracleObservation {
            status: OracleStatus::Timeout,
            frontend_status: FrontendStatus::Unknown,
            phase: Phase::Supervisor,
            result: Availability::unavailable(UnavailableReason::PhaseNotReached),
            output,
            error: protocol_error.map_or_else(
                || Availability::unavailable(UnavailableReason::WorkerRecordMissing),
                Availability::available,
            ),
            process: Availability::available(process),
        };
    }
    if !termination_succeeded(&process.termination) {
        let error = failed_termination_error(&process.termination).or(protocol_error);
        return internal_oracle(process, output, error);
    }
    let Some(record) = record else {
        return internal_oracle(process, output, protocol_error);
    };
    if record.schema_version != SCHEMA_VERSION {
        return internal_oracle(process, output, Some(schema_error(record.schema_version)));
    }
    let status = match record.disposition {
        OracleWorkerDisposition::Success => OracleStatus::Success,
        OracleWorkerDisposition::FrontendRejected => OracleStatus::FrontendRejected,
        OracleWorkerDisposition::RuntimeError => OracleStatus::RuntimeError,
        OracleWorkerDisposition::InternalError => OracleStatus::InternalError,
    };
    OracleObservation {
        status,
        frontend_status: record.frontend_status,
        phase: record.phase,
        result: record.result,
        output,
        error: record.error,
        process: Availability::available(process),
    }
}

fn internal_oracle(
    process: ProcessObservation,
    output: Availability<ByteArtifact>,
    error: Option<ErrorRecord>,
) -> OracleObservation {
    OracleObservation {
        status: OracleStatus::InternalError,
        frontend_status: FrontendStatus::Unknown,
        phase: Phase::Supervisor,
        result: Availability::unavailable(UnavailableReason::WorkerRecordMissing),
        output,
        error: error.map_or_else(
            || Availability::unavailable(UnavailableReason::WorkerRecordMissing),
            Availability::available,
        ),
        process: Availability::available(process),
    }
}

pub(crate) fn generic_observation(
    oracle: &OracleObservation,
    supervised: Supervised<VmWorkerRecord>,
) -> PlanObservation {
    plan_observation(oracle, supervised, PlanKind::Generic)
}

pub(crate) fn physical_observation(
    oracle: &OracleObservation,
    supervised: Supervised<VmWorkerRecord>,
) -> PlanObservation {
    plan_observation(oracle, supervised, PlanKind::Physical)
}

fn plan_observation(
    oracle: &OracleObservation,
    supervised: Supervised<VmWorkerRecord>,
    requested_plan: PlanKind,
) -> PlanObservation {
    let Supervised {
        process,
        record,
        protocol_error,
    } = supervised;
    if matches!(process.termination, Termination::TimedOut { .. }) {
        return failed_plan(
            requested_plan,
            None,
            PlanClassification::Timeout,
            Phase::Supervisor,
            protocol_error,
            process,
        );
    }
    if !termination_succeeded(&process.termination) {
        let error = failed_termination_error(&process.termination).or(protocol_error);
        return failed_plan(
            requested_plan,
            None,
            PlanClassification::InternalError,
            Phase::Supervisor,
            error,
            process,
        );
    }
    let Some(record) = record else {
        return failed_plan(
            requested_plan,
            None,
            PlanClassification::InternalError,
            Phase::Supervisor,
            protocol_error,
            process,
        );
    };
    if record.schema_version != SCHEMA_VERSION {
        return failed_plan(
            requested_plan,
            None,
            PlanClassification::InternalError,
            Phase::Supervisor,
            Some(schema_error(record.schema_version)),
            process,
        );
    }
    if let Some(error) = validate_worker_record(&record, requested_plan) {
        return failed_plan(
            requested_plan,
            record.executed_plan,
            PlanClassification::InternalError,
            record.phase,
            Some(error),
            process,
        );
    }
    let (classification, parity) = classify_plan(oracle, &record);
    PlanObservation {
        requested_plan,
        executed_plan: record.executed_plan,
        fallback: record.fallback,
        phase: record.phase,
        classification,
        parity,
        result: record.result,
        output: record.output,
        bytecode_metrics: record.bytecode_metrics,
        execution_metrics: record.execution_metrics,
        physical_plan_metrics: record.physical_plan_metrics,
        error: record.error,
        process: Availability::available(process),
    }
}

fn failed_plan(
    requested_plan: PlanKind,
    executed_plan: Option<PlanKind>,
    classification: PlanClassification,
    phase: Phase,
    error: Option<ErrorRecord>,
    process: ProcessObservation,
) -> PlanObservation {
    PlanObservation {
        requested_plan,
        executed_plan,
        fallback: false,
        phase,
        classification,
        parity: unavailable_parity(ParityReason::PlanDidNotSucceed),
        result: Availability::unavailable(UnavailableReason::PhaseNotReached),
        output: Availability::unavailable(UnavailableReason::PhaseNotReached),
        bytecode_metrics: Availability::unavailable(UnavailableReason::PhaseNotReached),
        execution_metrics: Availability::unavailable(UnavailableReason::PhaseNotReached),
        physical_plan_metrics: PhysicalPlanMetricsStatus::Unavailable {
            reason: UnavailableReason::PhaseNotReached,
        },
        error: error.map_or_else(
            || Availability::unavailable(UnavailableReason::WorkerRecordMissing),
            Availability::available,
        ),
        process: Availability::available(process),
    }
}

fn not_run_plan(plan: PlanKind) -> PlanObservation {
    PlanObservation {
        requested_plan: plan,
        executed_plan: None,
        fallback: false,
        phase: Phase::Supervisor,
        classification: PlanClassification::NotRunFrontendRejected,
        parity: unavailable_parity(ParityReason::PlanDidNotSucceed),
        result: Availability::unavailable(UnavailableReason::FrontendRejected),
        output: Availability::unavailable(UnavailableReason::FrontendRejected),
        bytecode_metrics: Availability::unavailable(UnavailableReason::FrontendRejected),
        execution_metrics: Availability::unavailable(UnavailableReason::FrontendRejected),
        physical_plan_metrics: PhysicalPlanMetricsStatus::Unavailable {
            reason: UnavailableReason::FrontendRejected,
        },
        error: Availability::unavailable(UnavailableReason::FrontendRejected),
        process: Availability::unavailable(UnavailableReason::ProcessNotSpawned),
    }
}

fn increment_oracle(counts: &mut OracleCounts, status: OracleStatus) {
    match status {
        OracleStatus::NotRequested => counts.not_requested += 1,
        OracleStatus::Success => counts.success += 1,
        OracleStatus::FrontendRejected => counts.frontend_rejected += 1,
        OracleStatus::RuntimeError => counts.runtime_error += 1,
        OracleStatus::Timeout => counts.timeout += 1,
        OracleStatus::InternalError => counts.internal_error += 1,
    }
}

fn increment_plan(counts: &mut PlanCounts, classification: PlanClassification) {
    match classification {
        PlanClassification::NotRunFrontendRejected => counts.not_run_frontend_rejected += 1,
        PlanClassification::NotRunOracleUnavailable => counts.not_run_oracle_unavailable += 1,
        PlanClassification::FrontendRejected => counts.frontend_rejected += 1,
        PlanClassification::VmCorrect => counts.vm_correct += 1,
        PlanClassification::Mismatch => counts.mismatch += 1,
        PlanClassification::ComparisonUnavailable => counts.comparison_unavailable += 1,
        PlanClassification::Unsupported => counts.unsupported += 1,
        PlanClassification::RealizationError => counts.realization_error += 1,
        PlanClassification::CompileError => counts.compile_error += 1,
        PlanClassification::RuntimeError => counts.runtime_error += 1,
        PlanClassification::Timeout => counts.timeout += 1,
        PlanClassification::VerifierError => counts.verifier_error += 1,
        PlanClassification::Fallback => counts.fallback += 1,
        PlanClassification::PhysicalPrepareError => counts.physical_prepare_error += 1,
        PlanClassification::InternalError => counts.internal_error += 1,
    }
}

fn schema_error(found: u32) -> ErrorRecord {
    let message = format!("worker schema version {found}, expected {SCHEMA_VERSION}");
    ErrorRecord::new(
        ErrorKind::Harness {
            kind: HarnessErrorKind::WorkerRecordInvalid,
        },
        message.clone(),
        &message,
    )
}
