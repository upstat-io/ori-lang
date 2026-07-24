//! Breadth-harness contract tests.

use std::error::Error;
use std::ffi::OsString;
use std::fs;

use crate::app::Command;
use crate::contract::{
    Availability, FrozenFrontendStatus, FrozenVmStatus, OracleCounts, OraclePolicy, ParityReason,
    ParityStatus, Phase, PhysicalPlanMetricsStatus, PlanClassification, PlanCounts,
    ReconciliationStatus, VmWorkerDisposition, SCHEMA_VERSION,
};
use crate::digest::{ByteArtifact, EncodedBytes};
use crate::manifest::{self, ManifestError};
use crate::values::{EvaluatorValue, VmValue};

#[path = "tests/classification.rs"]
mod classification;
#[path = "tests/protocol.rs"]
mod protocol;

#[test]
fn frozen_manifest_reconciles_all_independent_denominators() -> Result<(), Box<dyn Error>> {
    let manifest = manifest::load(
        &manifest::default_manifest_path(),
        &manifest::repository_root(),
    )?;
    assert_eq!(manifest.document.schema_version, SCHEMA_VERSION);
    assert_eq!(manifest.document.cases.len(), 207);
    assert_eq!(count_frontend(&manifest, FrozenFrontendStatus::Valid), 194);
    assert_eq!(
        count_frontend(&manifest, FrozenFrontendStatus::Rejected),
        13
    );
    assert_eq!(
        count_vm(&manifest, FrozenVmStatus::NotRunFrontendRejected),
        13
    );
    assert_eq!(count_vm(&manifest, FrozenVmStatus::FullVm), 48);
    assert_eq!(count_vm(&manifest, FrozenVmStatus::Unsupported), 122);
    assert_eq!(count_vm(&manifest, FrozenVmStatus::Runtime), 21);
    assert_eq!(count_vm(&manifest, FrozenVmStatus::Timeout), 3);
    assert_eq!(count_vm(&manifest, FrozenVmStatus::Verifier), 0);
    assert_eq!(count_vm(&manifest, FrozenVmStatus::Fallback), 0);
    Ok(())
}

#[test]
fn manifest_rejects_unknown_fields_and_source_digest_drift() -> Result<(), Box<dyn Error>> {
    let raw = fs::read(manifest::default_manifest_path())?;
    let mut unknown: serde_json::Value = serde_json::from_slice(&raw)?;
    let Some(object) = unknown.as_object_mut() else {
        panic!("fixture manifest must be a JSON object");
    };
    object.insert("uncontracted".to_owned(), serde_json::Value::Bool(true));
    let unknown_file = tempfile::NamedTempFile::new()?;
    fs::write(unknown_file.path(), serde_json::to_vec(&unknown)?)?;
    assert!(matches!(
        manifest::load(unknown_file.path(), &manifest::repository_root()),
        Err(ManifestError::Json { .. })
    ));

    let mut drifted: serde_json::Value = serde_json::from_slice(&raw)?;
    let Some(first_case) = drifted
        .get_mut("cases")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|cases| cases.first_mut())
        .and_then(serde_json::Value::as_object_mut)
    else {
        panic!("fixture manifest must contain a first case object");
    };
    first_case.insert(
        "source_sha256".to_owned(),
        serde_json::Value::String("0".repeat(64)),
    );
    let drifted_file = tempfile::NamedTempFile::new()?;
    fs::write(drifted_file.path(), serde_json::to_vec(&drifted)?)?;
    assert!(matches!(
        manifest::load(drifted_file.path(), &manifest::repository_root()),
        Err(ManifestError::Contract { .. })
    ));
    Ok(())
}

#[test]
fn byte_artifact_preserves_exact_bytes_and_digest() {
    let artifact = ByteArtifact::complete(&[0, 0xff, b'A']);
    assert_eq!(artifact.length_decimal, "3");
    assert_eq!(
        artifact.sha256,
        "a90a10503fbfc95789ff38a1bb5039cb71869ab9c0eb1cb51c4a9099f2933c6b"
    );
    assert_eq!(
        artifact.bytes,
        EncodedBytes::CompleteHex {
            hex: "00ff41".to_owned()
        }
    );
}

#[test]
fn parity_requires_lossless_value_and_output_identity() {
    let evaluator = EvaluatorValue::Int {
        decimal: "7".to_owned(),
    };
    let vm = VmValue::Int {
        decimal: "7".to_owned(),
    };
    let empty = ByteArtifact::complete(&[]);
    let (classification, parity) = crate::runner::parity_for(&evaluator, &vm, &empty, &empty);
    assert_eq!(classification, PlanClassification::VmCorrect);
    assert_eq!(parity.status, ParityStatus::Equal);
    assert_eq!(parity.reason, None);

    let different = ByteArtifact::complete(b"different");
    let (classification, parity) = crate::runner::parity_for(&evaluator, &vm, &empty, &different);
    assert_eq!(classification, PlanClassification::Mismatch);
    assert_eq!(parity.reason, Some(ParityReason::OutputDiffers));

    let semantic_tag = EvaluatorValue::Tuple {
        elements: vec![evaluator],
    };
    let erased_tag = VmValue::Aggregate {
        variant_decimal: "0".to_owned(),
        fields: vec![vm],
    };
    let (classification, parity) =
        crate::runner::parity_for(&semantic_tag, &erased_tag, &empty, &empty);
    assert_eq!(classification, PlanClassification::ComparisonUnavailable);
    assert_eq!(
        parity.reason,
        Some(ParityReason::EvaluatorValueNotLosslesslyComparable)
    );
}

#[test]
fn summaries_name_policy_and_reconcile_every_denominator() -> Result<(), Box<dyn Error>> {
    let manifest = manifest::load(
        &manifest::default_manifest_path(),
        &manifest::repository_root(),
    )?;
    let oracle = OracleCounts {
        not_requested: 159,
        success: 48,
        ..OracleCounts::default()
    };
    let generic = PlanCounts {
        not_run_frontend_rejected: 13,
        vm_correct: 48,
        comparison_unavailable: 146,
        ..PlanCounts::default()
    };
    let physical = PlanCounts {
        not_run_frontend_rejected: 13,
        vm_correct: 48,
        comparison_unavailable: 146,
        ..PlanCounts::default()
    };
    let summary = crate::runner::build_summary(
        &manifest,
        OraclePolicy::Historical48,
        oracle,
        generic,
        physical,
    );
    assert_eq!(summary.requested_oracle_policy, OraclePolicy::Historical48);
    assert_eq!(summary.executed_oracle_policy, OraclePolicy::Historical48);
    assert_eq!(summary.denominators.manifest_cases, 207);
    assert_eq!(summary.denominators.executable_vm_cases, 194);
    assert_eq!(summary.denominators.frozen_oracle_cases, 48);
    assert_eq!(summary.denominators.requested_oracle_cases, 48);
    assert_eq!(summary.denominators.executed_oracle_cases, 48);
    assert_eq!(
        summary.denominators.selected_reconciled,
        ReconciliationStatus::Reconciled
    );
    assert_eq!(
        summary.denominators.frozen_frontend_reconciled,
        ReconciliationStatus::Reconciled
    );
    assert_eq!(
        summary.denominators.frozen_oracle_reconciled,
        ReconciliationStatus::Reconciled
    );
    assert_eq!(
        summary.denominators.executable_vm_reconciled,
        ReconciliationStatus::Reconciled
    );
    assert_eq!(
        summary.denominators.oracle_policy_reconciled,
        ReconciliationStatus::Reconciled
    );
    assert_eq!(
        summary.denominators.plan_denominators_reconciled,
        ReconciliationStatus::Reconciled
    );
    Ok(())
}

#[test]
fn cli_has_distinct_fast_and_admission_oracle_policies() -> Result<(), Box<dyn Error>> {
    let fast = crate::app::parse([OsString::from("vm_breadth"), OsString::from("run")])?;
    assert!(matches!(
        fast,
        Command::Run {
            oracle_policy: OraclePolicy::Historical48,
            ..
        }
    ));
    let admission = crate::app::parse([
        OsString::from("vm_breadth"),
        OsString::from("run"),
        OsString::from("--oracle-policy"),
        OsString::from("all-frontend-valid"),
        OsString::from("--timeout-ms"),
        OsString::from("123"),
    ])?;
    assert!(matches!(
        admission,
        Command::Run {
            oracle_policy: OraclePolicy::AllFrontendValid,
            timeout,
            ..
        } if timeout.as_millis() == 123
    ));
    assert_eq!(
        serde_json::to_string(&OraclePolicy::Historical48)?,
        "\"historical-48\""
    );
    assert_eq!(
        serde_json::to_string(&OraclePolicy::AllFrontendValid)?,
        "\"all-frontend-valid\""
    );
    assert!(crate::app::parse([
        OsString::from("vm_breadth"),
        OsString::from("run"),
        OsString::from("--oracle-policy"),
        OsString::from("historical"),
    ])
    .is_err());
    Ok(())
}

#[test]
fn bounded_workers_emit_comparable_success_and_metrics() -> Result<(), Box<dyn Error>> {
    let evaluator_record = tempfile::NamedTempFile::new()?;
    let vm_record = tempfile::NamedTempFile::new()?;
    let physical_record = tempfile::NamedTempFile::new()?;
    let source = manifest::repository_root().join("tests/benchmarks/bench_hello.ori");
    crate::worker::evaluator(&source, evaluator_record.path())?;
    crate::worker::generic(&source, vm_record.path())?;
    crate::worker::physical(&source, physical_record.path())?;
    let evaluator: crate::contract::OracleWorkerRecord =
        serde_json::from_slice(&fs::read(evaluator_record.path())?)?;
    let worker: crate::contract::VmWorkerRecord =
        serde_json::from_slice(&fs::read(vm_record.path())?)?;
    let physical: crate::contract::VmWorkerRecord =
        serde_json::from_slice(&fs::read(physical_record.path())?)?;
    assert_eq!(
        evaluator.disposition,
        crate::contract::OracleWorkerDisposition::Success
    );
    assert_eq!(worker.schema_version, SCHEMA_VERSION);
    assert_eq!(
        worker.disposition,
        crate::contract::VmWorkerDisposition::Success
    );
    assert_eq!(physical.disposition, VmWorkerDisposition::Success);
    assert_eq!(physical.phase, Phase::PhysicalExecute);
    assert_eq!(worker.result, physical.result);
    assert_eq!(worker.output, physical.output);
    assert_eq!(worker.bytecode_metrics, physical.bytecode_metrics);
    assert_eq!(worker.execution_metrics, physical.execution_metrics);
    assert!(matches!(
        worker.execution_metrics,
        Availability::Available { .. }
    ));
    assert!(matches!(
        worker.bytecode_metrics,
        Availability::Available { .. }
    ));
    assert!(matches!(
        &worker.physical_plan_metrics,
        PhysicalPlanMetricsStatus::Unavailable { .. }
    ));
    let PhysicalPlanMetricsStatus::Available {
        value: plan_metrics,
    } = &physical.physical_plan_metrics
    else {
        panic!("physical execution must publish its prepared-plan metrics");
    };
    assert_eq!(plan_metrics.logical_lanes, plan_metrics.physical_lanes);
    assert_eq!(plan_metrics.canonical_ops, plan_metrics.physical_ops);
    assert_eq!(plan_metrics.planning_scratch_current_payload_bytes, "0");
    assert_eq!(plan_metrics.validation_scratch_current_payload_bytes, "0");
    let (
        Availability::Available {
            value: evaluator_value,
        },
        Availability::Available { value: vm_value },
        Availability::Available { value: vm_output },
    ) = (&evaluator.result, &worker.result, &worker.output)
    else {
        panic!("successful bounded workers must publish result and output artifacts");
    };
    let (classification, parity) = crate::runner::parity_for(
        evaluator_value,
        vm_value,
        &ByteArtifact::complete(&[]),
        vm_output,
    );
    assert_eq!(classification, PlanClassification::VmCorrect);
    assert_eq!(parity.status, ParityStatus::Equal);
    Ok(())
}

fn count_frontend(manifest: &manifest::LoadedManifest, status: FrozenFrontendStatus) -> usize {
    manifest
        .document
        .cases
        .iter()
        .filter(|case| case.frontend_status == status)
        .count()
}

fn count_vm(manifest: &manifest::LoadedManifest, status: FrozenVmStatus) -> usize {
    manifest
        .document
        .cases
        .iter()
        .filter(|case| case.vm_status == status)
        .count()
}
