//! Evaluator and bytecode-worker entry points.

#[path = "worker/error_kinds.rs"]
mod error_kinds;

use std::fs;
use std::path::Path;

use oric::{CompilerDb, Db, SourceFile};

use error_kinds::{
    compile_error_is_unsupported, compile_error_kind, executable_error_kind, execution_error_kind,
    record_error, verify_error_kind,
};

use crate::contract::{
    Availability, ExecutionEntry, FrontendStatus, OracleWorkerDisposition, OracleWorkerRecord,
    Phase, PhysicalPlanMetricsStatus, PlanKind, UnavailableReason, VmWorkerDisposition,
    VmWorkerRecord, SCHEMA_VERSION,
};
use crate::errors::{ErrorKind, ErrorRecord, FrontendErrorKind, HarnessErrorKind};
use crate::metrics::{BytecodeMetricsRecord, ExecutionMetricsRecord, PhysicalPlanMetricsRecord};
use crate::values::{EvaluatorValue, VmValue};

pub(crate) fn evaluator(source_path: &Path, record_path: &Path) -> Result<(), WorkerIoError> {
    let source = match fs::read_to_string(source_path) {
        Ok(source) => source,
        Err(error) => {
            return write_record(
                record_path,
                &oracle_failure(
                    OracleWorkerDisposition::InternalError,
                    Phase::Supervisor,
                    FrontendStatus::Unknown,
                    ErrorRecord::new(
                        ErrorKind::Harness {
                            kind: HarnessErrorKind::SourceRead,
                        },
                        error.to_string(),
                        &format!("{error:?}"),
                    ),
                ),
            );
        }
    };
    let path = source_path.to_string_lossy().into_owned();
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, path.into(), source);
    if let Some((phase, error)) = frontend_error(&db, file) {
        return write_record(
            record_path,
            &oracle_failure(
                OracleWorkerDisposition::FrontendRejected,
                phase,
                FrontendStatus::Rejected,
                error,
            ),
        );
    }

    let evaluated = oric::evaluated(&db, file);
    let record = match (&evaluated.result, &evaluated.error, &evaluated.eval_error) {
        (Some(value), None, None) => OracleWorkerRecord {
            schema_version: SCHEMA_VERSION,
            disposition: OracleWorkerDisposition::Success,
            phase: Phase::Eval,
            frontend_status: FrontendStatus::Valid,
            result: Availability::available(EvaluatorValue::from_output(value, db.interner())),
            error: Availability::unavailable(UnavailableReason::NoErrorForSuccess),
        },
        (None, Some(message), Some(snapshot)) => oracle_failure(
            OracleWorkerDisposition::RuntimeError,
            Phase::Eval,
            FrontendStatus::Valid,
            ErrorRecord::new(
                ErrorKind::EvaluatorRuntime {
                    kind_name: snapshot.kind_name.clone(),
                    error_code: snapshot.error_code.to_string(),
                },
                message.clone(),
                &format!("{snapshot:?}"),
            ),
        ),
        _ => oracle_failure(
            OracleWorkerDisposition::InternalError,
            Phase::Eval,
            FrontendStatus::Valid,
            ErrorRecord::new(
                ErrorKind::Harness {
                    kind: HarnessErrorKind::InconsistentWorkerRecord,
                },
                "evaluator returned an inconsistent result/error tuple".to_owned(),
                &format!("{evaluated:?}"),
            ),
        ),
    };
    write_record(record_path, &record)
}

pub(crate) fn generic(source_path: &Path, record_path: &Path) -> Result<(), WorkerIoError> {
    bytecode_worker(source_path, record_path, PlanKind::Generic)
}

pub(crate) fn physical(source_path: &Path, record_path: &Path) -> Result<(), WorkerIoError> {
    bytecode_worker(source_path, record_path, PlanKind::Physical)
}

fn bytecode_worker(
    source_path: &Path,
    record_path: &Path,
    requested_plan: PlanKind,
) -> Result<(), WorkerIoError> {
    let source = match fs::read_to_string(source_path) {
        Ok(source) => source,
        Err(error) => {
            return write_record(
                record_path,
                &vm_failure(
                    VmWorkerDisposition::InternalError,
                    Phase::Supervisor,
                    requested_plan,
                    ErrorRecord::new(
                        ErrorKind::Harness {
                            kind: HarnessErrorKind::SourceRead,
                        },
                        error.to_string(),
                        &format!("{error:?}"),
                    ),
                ),
            );
        }
    };
    let record = compile_source(source_path, &source, requested_plan);
    write_record(record_path, &record)
}

fn compile_source(source_path: &Path, source: &str, requested_plan: PlanKind) -> VmWorkerRecord {
    let display_path = source_path.to_string_lossy().into_owned();
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, display_path.clone().into(), source.to_owned());
    if let Some((phase, error)) = frontend_error(&db, file) {
        return vm_failure(
            VmWorkerDisposition::FrontendRejected,
            phase,
            requested_plan,
            error,
        );
    }

    let executable = match oric::test_support::compile_to_executable(
        &display_path,
        source,
        ori_repr::NarrowingPolicy::Aggressive,
    ) {
        Ok(executable) => executable,
        Err(error) => {
            let (disposition, phase, kind) = executable_error_kind(&error);
            return vm_failure(
                disposition,
                phase,
                requested_plan,
                record_error(kind, &error),
            );
        }
    };
    execute_bytecode(&executable, requested_plan)
}

fn execute_bytecode(
    executable: &ori_repr::executable::ExecutableProgram,
    requested_plan: PlanKind,
) -> VmWorkerRecord {
    let bytecode = match ori_vm::compile(executable) {
        Ok(bytecode) => bytecode,
        Err(error) => {
            let disposition = if compile_error_is_unsupported(&error) {
                VmWorkerDisposition::Unsupported
            } else {
                VmWorkerDisposition::CompileError
            };
            let kind = ErrorKind::BytecodeCompile {
                kind: compile_error_kind(&error),
            };
            return vm_failure(
                disposition,
                Phase::BytecodeCompile,
                requested_plan,
                record_error(kind, &error),
            );
        }
    };
    let bytecode_metrics = BytecodeMetricsRecord::from(bytecode.metrics());
    let verified = match ori_vm::verify(bytecode) {
        Ok(verified) => verified,
        Err(error) => {
            let kind = ErrorKind::BytecodeVerify {
                kind: verify_error_kind(&error),
            };
            let mut record = vm_failure(
                VmWorkerDisposition::VerifierError,
                Phase::BytecodeVerify,
                requested_plan,
                record_error(kind, &error),
            );
            record.bytecode_metrics = Availability::available(bytecode_metrics);
            return record;
        }
    };
    let (report, phase, physical_plan_metrics) = match requested_plan {
        PlanKind::Generic => (
            ori_vm::execute_report(&verified, ori_vm::ExecutionConfig::default()),
            Phase::VmExecute,
            PhysicalPlanMetricsStatus::Unavailable {
                reason: UnavailableReason::PhaseNotReached,
            },
        ),
        PlanKind::Physical => {
            let plan = match ori_vm::prepare(&verified, ori_vm::PhysicalOptions::default()) {
                Ok(plan) => plan,
                Err(error) => {
                    let mut record = vm_failure(
                        VmWorkerDisposition::PhysicalPrepareError,
                        Phase::PhysicalPrepare,
                        requested_plan,
                        record_error(ErrorKind::PhysicalPrepare, &error),
                    );
                    record.bytecode_metrics = Availability::available(bytecode_metrics);
                    return record;
                }
            };
            let metrics = PhysicalPlanMetricsStatus::Available {
                value: Box::new(PhysicalPlanMetricsRecord::from(plan.metrics())),
            };
            let report = ori_vm::execute_physical_report(&plan, ori_vm::ExecutionConfig::default());
            (report, Phase::PhysicalExecute, metrics)
        }
    };
    execution_record(
        report,
        phase,
        requested_plan,
        bytecode_metrics,
        physical_plan_metrics,
    )
}

fn execution_record(
    report: ori_vm::ExecutionReport,
    phase: Phase,
    executed_plan: PlanKind,
    bytecode_metrics: BytecodeMetricsRecord,
    physical_plan_metrics: PhysicalPlanMetricsStatus,
) -> VmWorkerRecord {
    let execution_metrics = ExecutionMetricsRecord::from(&report.metrics);
    let output = crate::digest::ByteArtifact::complete(&report.output);
    if let Some(error) = cleanup_error(&report.metrics) {
        return VmWorkerRecord {
            schema_version: SCHEMA_VERSION,
            disposition: VmWorkerDisposition::InternalError,
            phase,
            requested_plan: executed_plan,
            execution_entry: ExecutionEntry::Entered,
            executed_plan: Some(executed_plan),
            fallback: false,
            result: Availability::unavailable(UnavailableReason::NoResultForError),
            output: Availability::available(output),
            bytecode_metrics: Availability::available(bytecode_metrics),
            execution_metrics: Availability::available(execution_metrics),
            physical_plan_metrics,
            error: Availability::available(error),
        };
    }
    match report.result {
        Ok(value) => VmWorkerRecord {
            schema_version: SCHEMA_VERSION,
            disposition: VmWorkerDisposition::Success,
            phase,
            requested_plan: executed_plan,
            execution_entry: ExecutionEntry::Entered,
            executed_plan: Some(executed_plan),
            fallback: false,
            result: Availability::available(VmValue::from(&value)),
            output: Availability::available(output),
            bytecode_metrics: Availability::available(bytecode_metrics),
            execution_metrics: Availability::available(execution_metrics),
            physical_plan_metrics,
            error: Availability::unavailable(UnavailableReason::NoErrorForSuccess),
        },
        Err(error) => VmWorkerRecord {
            schema_version: SCHEMA_VERSION,
            disposition: VmWorkerDisposition::RuntimeError,
            phase,
            requested_plan: executed_plan,
            execution_entry: ExecutionEntry::Entered,
            executed_plan: Some(executed_plan),
            fallback: false,
            result: Availability::unavailable(UnavailableReason::NoResultForError),
            output: Availability::available(output),
            bytecode_metrics: Availability::available(bytecode_metrics),
            execution_metrics: Availability::available(execution_metrics),
            physical_plan_metrics,
            error: Availability::available(record_error(
                ErrorKind::VmExecution {
                    kind: execution_error_kind(&error),
                },
                &error,
            )),
        },
    }
}

fn cleanup_error(metrics: &ori_vm::ExecutionMetrics) -> Option<ErrorRecord> {
    let clean = metrics.final_live_heap_objects == 0
        && metrics.final_live_heap_payload_bytes == 0
        && metrics.final_value_arena_entries == 0
        && metrics.final_value_arena_slots == 0
        && metrics.final_value_arena_owned_bytes == 0;
    (!clean).then(|| {
        let message = format!(
            "VM execution returned success with live cleanup state: heap_objects={}, heap_bytes={}, arena_entries={}, arena_slots={}, arena_bytes={}",
            metrics.final_live_heap_objects,
            metrics.final_live_heap_payload_bytes,
            metrics.final_value_arena_entries,
            metrics.final_value_arena_slots,
            metrics.final_value_arena_owned_bytes,
        );
        ErrorRecord::new(
            ErrorKind::Harness {
                kind: HarnessErrorKind::InconsistentWorkerRecord,
            },
            message.clone(),
            &message,
        )
    })
}

fn frontend_error(db: &CompilerDb, file: SourceFile) -> Option<(Phase, ErrorRecord)> {
    let lex_errors = oric::query::lex_errors(db, file);
    if !lex_errors.is_empty() {
        return Some((
            Phase::Lex,
            ErrorRecord::new(
                ErrorKind::Frontend {
                    kind: FrontendErrorKind::Lex,
                },
                format!("{} lexer error(s)", lex_errors.len()),
                &format!("{lex_errors:?}"),
            ),
        ));
    }
    let parsed = oric::query::parsed(db, file);
    if parsed.has_errors() {
        return Some((
            Phase::Parse,
            ErrorRecord::new(
                ErrorKind::Frontend {
                    kind: FrontendErrorKind::Parse,
                },
                format!("{} parser error(s)", parsed.errors.len()),
                &format!("{:?}", parsed.errors),
            ),
        ));
    }
    let typed = oric::query::typed(db, file);
    if typed.has_errors() {
        return Some((
            Phase::Typecheck,
            ErrorRecord::new(
                ErrorKind::Frontend {
                    kind: FrontendErrorKind::Typecheck,
                },
                format!("{} type error(s)", typed.errors().len()),
                &format!("{:?}", typed.errors()),
            ),
        ));
    }
    None
}

fn oracle_failure(
    disposition: OracleWorkerDisposition,
    phase: Phase,
    frontend_status: FrontendStatus,
    error: ErrorRecord,
) -> OracleWorkerRecord {
    OracleWorkerRecord {
        schema_version: SCHEMA_VERSION,
        disposition,
        phase,
        frontend_status,
        result: Availability::unavailable(UnavailableReason::NoResultForError),
        error: Availability::available(error),
    }
}

fn vm_failure(
    disposition: VmWorkerDisposition,
    phase: Phase,
    requested_plan: PlanKind,
    error: ErrorRecord,
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
        error: Availability::available(error),
    }
}

fn write_record<T: serde::Serialize>(path: &Path, record: &T) -> Result<(), WorkerIoError> {
    let encoded = serde_json::to_vec(record).map_err(WorkerIoError::Serialize)?;
    fs::write(path, encoded).map_err(|source| WorkerIoError::Write {
        path: path.to_owned(),
        source,
    })
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum WorkerIoError {
    #[error("cannot serialize worker record: {0}")]
    Serialize(serde_json::Error),
    #[error("cannot write worker record `{path}`: {source}")]
    Write {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}
