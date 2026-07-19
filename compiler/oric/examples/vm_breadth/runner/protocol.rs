//! VM worker-record provenance and availability validation.

use crate::contract::{
    Availability, ExecutionEntry, Phase, PhysicalPlanMetricsStatus, PlanKind, VmWorkerDisposition,
    VmWorkerRecord,
};
use crate::errors::{ErrorKind, ErrorRecord, HarnessErrorKind};

pub(super) fn validate_worker_record(
    record: &VmWorkerRecord,
    requested_plan: PlanKind,
) -> Option<ErrorRecord> {
    let entered = record.execution_entry == ExecutionEntry::Entered;
    let executed = record.executed_plan.is_some();
    let execution_disposition = matches!(
        record.disposition,
        VmWorkerDisposition::Success | VmWorkerDisposition::RuntimeError
    );
    let physical_metrics_available = matches!(
        &record.physical_plan_metrics,
        PhysicalPlanMetricsStatus::Available { .. }
    );
    let result_available = matches!(&record.result, Availability::Available { .. });
    let output_available = matches!(&record.output, Availability::Available { .. });
    let bytecode_metrics_available =
        matches!(&record.bytecode_metrics, Availability::Available { .. });
    let execution_metrics_available =
        matches!(&record.execution_metrics, Availability::Available { .. });
    let error_available = matches!(&record.error, Availability::Available { .. });
    let execution_phase_matches = match record.executed_plan {
        Some(PlanKind::Generic) => record.phase == Phase::VmExecute,
        Some(PlanKind::Physical) => record.phase == Phase::PhysicalExecute,
        None => true,
    };
    let fallback_provenance = record.requested_plan == PlanKind::Physical
        && record.executed_plan == Some(PlanKind::Generic)
        && entered;
    let detail = if record.requested_plan != requested_plan {
        Some("worker requested-plan identity differs from the supervisor request")
    } else if entered != executed {
        Some("execution entry and executed-plan provenance disagree")
    } else if execution_disposition && !entered {
        Some("success or runtime error claims that execution was not entered")
    } else if record.fallback && !fallback_provenance {
        Some("fallback lacks physical-request to generic-execution provenance")
    } else if record.disposition == VmWorkerDisposition::PhysicalPrepareError && entered {
        Some("physical preparation failure claims that execution was entered")
    } else if record
        .executed_plan
        .is_some_and(|plan| plan != requested_plan)
        && !record.fallback
    {
        Some("executed-plan identity differs without an explicit fallback")
    } else if entered && !execution_phase_matches {
        Some("execution phase differs from the executed-plan identity")
    } else if record.executed_plan == Some(PlanKind::Physical) && !physical_metrics_available {
        Some("physical execution omitted prepared-plan metrics")
    } else if record.executed_plan == Some(PlanKind::Generic) && physical_metrics_available {
        Some("generic execution published physical-plan metrics")
    } else if physical_metrics_available && requested_plan != PlanKind::Physical {
        Some("non-physical request published physical-plan metrics")
    } else if entered
        && (!output_available || !bytecode_metrics_available || !execution_metrics_available)
    {
        Some("entered execution omitted output, bytecode, or execution metrics")
    } else if !entered && execution_metrics_available {
        Some("worker published execution metrics without entering execution")
    } else if record.disposition == VmWorkerDisposition::Success
        && (!result_available || error_available)
    {
        Some("successful worker record has inconsistent result/error availability")
    } else if record.disposition == VmWorkerDisposition::RuntimeError
        && (result_available || !error_available)
    {
        Some("runtime-error worker record has inconsistent result/error availability")
    } else if record.disposition != VmWorkerDisposition::Success && !error_available {
        Some("failed worker record omitted its typed error")
    } else {
        None
    };
    detail.map(inconsistent_worker_record)
}

fn inconsistent_worker_record(detail: &str) -> ErrorRecord {
    ErrorRecord::new(
        ErrorKind::Harness {
            kind: HarnessErrorKind::InconsistentWorkerRecord,
        },
        detail.to_owned(),
        detail,
    )
}
