//! Versioned JSONL and worker protocol contract.

use serde::{Deserialize, Serialize};

use crate::digest::ByteArtifact;
use crate::errors::ErrorRecord;
use crate::metrics::{BytecodeMetricsRecord, ExecutionMetricsRecord, PhysicalPlanMetricsRecord};
use crate::values::{EvaluatorValue, VmValue};

pub(crate) const SCHEMA_VERSION: u32 = 1;

/// Explicit presence state for values whose absence affects interpretation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Availability<T> {
    Available { value: T },
    Unavailable { reason: UnavailableReason },
}

impl<T> Availability<T> {
    pub(crate) fn available(value: T) -> Self {
        Self::Available { value }
    }

    pub(crate) const fn unavailable(reason: UnavailableReason) -> Self {
        Self::Unavailable { reason }
    }
}

/// Closed reasons for an unavailable measurement or artifact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnavailableReason {
    PhaseNotReached,
    FrontendRejected,
    OracleUnavailable,
    ProcessNotSpawned,
    UnsupportedPlatform,
    ObservationMissed,
    VcsIdentityNotEmbedded,
    WorkerRecordMissing,
    WorkerRecordInvalid,
    NoResultForError,
    NoErrorForSuccess,
}

/// Compiler or harness phase owning an observation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    Lex,
    Parse,
    Typecheck,
    Eval,
    Realization,
    BytecodeCompile,
    BytecodeVerify,
    VmExecute,
    PhysicalPrepare,
    PhysicalExecute,
    Supervisor,
}

/// Current frontend status observed by the evaluator worker.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrontendStatus {
    Valid,
    Rejected,
    Unknown,
}

/// Historical frontend status frozen in the corpus manifest.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrozenFrontendStatus {
    Valid,
    Rejected,
}

/// Historical raw VM outcome frozen independently of frontend validity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrozenVmStatus {
    NotRunFrontendRejected,
    FullVm,
    Unsupported,
    Runtime,
    Timeout,
    Verifier,
    Fallback,
}

/// Historical evaluator comparison for the full-VM subset.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrozenOracleStatus {
    Match,
    Mismatch,
    NotRun,
}

/// Evaluator-worker completion state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OracleWorkerDisposition {
    Success,
    FrontendRejected,
    RuntimeError,
    InternalError,
}

/// Record written out-of-band by one evaluator worker.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OracleWorkerRecord {
    pub(crate) schema_version: u32,
    pub(crate) disposition: OracleWorkerDisposition,
    pub(crate) phase: Phase,
    pub(crate) frontend_status: FrontendStatus,
    pub(crate) result: Availability<EvaluatorValue>,
    pub(crate) error: Availability<ErrorRecord>,
}

/// Whether bytecode execution was actually entered.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionEntry {
    Entered,
    NotEntered,
}

/// VM worker completion state before evaluator comparison.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VmWorkerDisposition {
    Success,
    FrontendRejected,
    Unsupported,
    RealizationError,
    CompileError,
    VerifierError,
    PhysicalPrepareError,
    RuntimeError,
    InternalError,
}

/// Record written out-of-band by one bytecode worker.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VmWorkerRecord {
    pub(crate) schema_version: u32,
    pub(crate) disposition: VmWorkerDisposition,
    pub(crate) phase: Phase,
    pub(crate) requested_plan: PlanKind,
    pub(crate) execution_entry: ExecutionEntry,
    pub(crate) executed_plan: Option<PlanKind>,
    pub(crate) fallback: bool,
    pub(crate) result: Availability<VmValue>,
    pub(crate) output: Availability<ByteArtifact>,
    pub(crate) bytecode_metrics: Availability<BytecodeMetricsRecord>,
    pub(crate) execution_metrics: Availability<ExecutionMetricsRecord>,
    pub(crate) physical_plan_metrics: PhysicalPlanMetricsStatus,
    pub(crate) error: Availability<ErrorRecord>,
}

/// A requested VM representation/execution plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanKind {
    Generic,
    Physical,
}

/// Current-run evaluator admission policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum OraclePolicy {
    #[serde(rename = "historical-48")]
    Historical48,
    #[serde(rename = "all-frontend-valid")]
    AllFrontendValid,
}

/// Complete process termination identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Termination {
    Exited {
        code_decimal: String,
    },
    Signaled {
        signal_decimal: String,
    },
    TimedOut {
        limit_ms_decimal: String,
        kill_error: Option<String>,
    },
    SpawnFailed {
        message: String,
    },
    WaitFailed {
        message: String,
    },
}

/// Parent-observed child-process resources and exact streams.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProcessObservation {
    pub(crate) termination: Termination,
    pub(crate) elapsed_ns_decimal: Availability<String>,
    pub(crate) sampled_peak_rss_kib_lower_bound_decimal: Availability<String>,
    pub(crate) stdout: ByteArtifact,
    pub(crate) stderr: ByteArtifact,
}

/// Evaluator oracle after process supervision and protocol validation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OracleStatus {
    NotRequested,
    Success,
    FrontendRejected,
    RuntimeError,
    Timeout,
    InternalError,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OracleObservation {
    pub(crate) status: OracleStatus,
    pub(crate) frontend_status: FrontendStatus,
    pub(crate) phase: Phase,
    pub(crate) result: Availability<EvaluatorValue>,
    pub(crate) output: Availability<ByteArtifact>,
    pub(crate) error: Availability<ErrorRecord>,
    pub(crate) process: Availability<ProcessObservation>,
}

/// Final classification of one requested plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanClassification {
    NotRunFrontendRejected,
    NotRunOracleUnavailable,
    FrontendRejected,
    VmCorrect,
    Mismatch,
    ComparisonUnavailable,
    Unsupported,
    RealizationError,
    CompileError,
    RuntimeError,
    Timeout,
    VerifierError,
    Fallback,
    PhysicalPrepareError,
    InternalError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ParityStatus {
    Equal,
    Different,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ParityReason {
    ValuesDiffer,
    OutputDiffers,
    ValueAndOutputDiffer,
    EvaluatorValueNotLosslesslyComparable,
    OracleDidNotSucceed,
    PlanDidNotSucceed,
    OracleSucceededPlanErrored,
    OracleErroredPlanSucceeded,
    ErrorDiffers,
    ErrorAndOutputDiffer,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParityObservation {
    pub(crate) status: ParityStatus,
    pub(crate) reason: Option<ParityReason>,
}

/// Physical planning metrics remain explicit across every worker outcome.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum PhysicalPlanMetricsStatus {
    Available {
        value: Box<PhysicalPlanMetricsRecord>,
    },
    Unavailable {
        reason: UnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanObservation {
    pub(crate) requested_plan: PlanKind,
    pub(crate) executed_plan: Option<PlanKind>,
    pub(crate) fallback: bool,
    pub(crate) phase: Phase,
    pub(crate) classification: PlanClassification,
    pub(crate) parity: ParityObservation,
    pub(crate) result: Availability<VmValue>,
    pub(crate) output: Availability<ByteArtifact>,
    pub(crate) bytecode_metrics: Availability<BytecodeMetricsRecord>,
    pub(crate) execution_metrics: Availability<ExecutionMetricsRecord>,
    pub(crate) physical_plan_metrics: PhysicalPlanMetricsStatus,
    pub(crate) error: Availability<ErrorRecord>,
    pub(crate) process: Availability<ProcessObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompilerIdentity {
    pub(crate) package_version: String,
    pub(crate) executable_sha256: String,
    pub(crate) vcs_head: Availability<String>,
    pub(crate) vcs_diff_sha256: Availability<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunRecord {
    pub(crate) schema_version: u32,
    pub(crate) corpus_id: String,
    pub(crate) manifest_sha256: String,
    pub(crate) compiler: CompilerIdentity,
    pub(crate) timeout_ms_decimal: String,
    pub(crate) requested_plans: Vec<PlanKind>,
    pub(crate) requested_oracle_policy: OraclePolicy,
    pub(crate) executed_oracle_policy: OraclePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaseRecord {
    pub(crate) schema_version: u32,
    pub(crate) manifest_index: usize,
    pub(crate) path: String,
    pub(crate) source_sha256: String,
    pub(crate) frozen_frontend_status: FrozenFrontendStatus,
    pub(crate) frozen_vm_status: FrozenVmStatus,
    pub(crate) frozen_oracle_status: FrozenOracleStatus,
    pub(crate) oracle: OracleObservation,
    pub(crate) plans: Vec<PlanObservation>,
    pub(crate) generic_physical_parity: BackendParityObservation,
}

/// Exact generic-versus-physical equality over shared observation fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackendParityObservation {
    pub(crate) status: BackendParityStatus,
    pub(crate) differences: Vec<BackendParityDifference>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackendParityStatus {
    Equal,
    Different,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackendParityDifference {
    Classification,
    EvaluatorParity,
    Fallback,
    Result,
    Output,
    BytecodeMetrics,
    ExecutionMetrics,
    Error,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrozenCounts {
    pub(crate) selected: usize,
    pub(crate) frontend_valid: usize,
    pub(crate) frontend_rejected: usize,
    pub(crate) oracle_eligible: usize,
    pub(crate) oracle_match: usize,
    pub(crate) oracle_mismatch: usize,
    pub(crate) vm_full: usize,
    pub(crate) vm_unsupported: usize,
    pub(crate) vm_runtime: usize,
    pub(crate) vm_timeout: usize,
    pub(crate) vm_verifier: usize,
    pub(crate) vm_fallback: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OracleCounts {
    pub(crate) not_requested: usize,
    pub(crate) success: usize,
    pub(crate) frontend_rejected: usize,
    pub(crate) runtime_error: usize,
    pub(crate) timeout: usize,
    pub(crate) internal_error: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanCounts {
    pub(crate) not_run_frontend_rejected: usize,
    pub(crate) not_run_oracle_unavailable: usize,
    pub(crate) frontend_rejected: usize,
    pub(crate) vm_correct: usize,
    pub(crate) mismatch: usize,
    pub(crate) comparison_unavailable: usize,
    pub(crate) unsupported: usize,
    pub(crate) realization_error: usize,
    pub(crate) compile_error: usize,
    pub(crate) runtime_error: usize,
    pub(crate) timeout: usize,
    pub(crate) verifier_error: usize,
    pub(crate) fallback: usize,
    pub(crate) physical_prepare_error: usize,
    pub(crate) internal_error: usize,
}

/// Typed result of one denominator equality check.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReconciliationStatus {
    Reconciled,
    Drift,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DenominatorReconciliation {
    pub(crate) manifest_cases: usize,
    pub(crate) emitted_cases: usize,
    pub(crate) oracle_classified_cases: usize,
    pub(crate) generic_classified_cases: usize,
    pub(crate) physical_classified_cases: usize,
    pub(crate) frozen_frontend_cases: usize,
    pub(crate) frozen_oracle_cases: usize,
    pub(crate) executable_vm_cases: usize,
    pub(crate) requested_oracle_cases: usize,
    pub(crate) executed_oracle_cases: usize,
    pub(crate) selected_reconciled: ReconciliationStatus,
    pub(crate) frozen_frontend_reconciled: ReconciliationStatus,
    pub(crate) frozen_oracle_reconciled: ReconciliationStatus,
    pub(crate) executable_vm_reconciled: ReconciliationStatus,
    pub(crate) oracle_policy_reconciled: ReconciliationStatus,
    pub(crate) plan_denominators_reconciled: ReconciliationStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SummaryRecord {
    pub(crate) schema_version: u32,
    pub(crate) requested_oracle_policy: OraclePolicy,
    pub(crate) executed_oracle_policy: OraclePolicy,
    pub(crate) frozen: FrozenCounts,
    pub(crate) oracle: OracleCounts,
    pub(crate) generic: PlanCounts,
    pub(crate) physical: PlanCounts,
    pub(crate) denominators: DenominatorReconciliation,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordKind {
    Run,
    Case,
    Summary,
    ManifestValidation,
}

#[derive(Serialize)]
pub(crate) struct RecordEnvelope<'record, T> {
    pub(crate) record: RecordKind,
    #[serde(flatten)]
    pub(crate) payload: &'record T,
}
