//! Frozen and current-run denominator reconciliation.

use crate::contract::{
    DenominatorReconciliation, FrozenCounts, FrozenFrontendStatus, FrozenOracleStatus,
    FrozenVmStatus, OracleCounts, OraclePolicy, PlanCounts, ReconciliationStatus, SummaryRecord,
    SCHEMA_VERSION,
};
use crate::manifest::LoadedManifest;

pub(crate) fn build_summary(
    manifest: &LoadedManifest,
    oracle_policy: OraclePolicy,
    oracle: OracleCounts,
    generic: PlanCounts,
    physical: PlanCounts,
) -> SummaryRecord {
    let frozen = frozen_counts(manifest);
    let expected_oracle = match oracle_policy {
        OraclePolicy::Historical48 => frozen.oracle_eligible,
        OraclePolicy::AllFrontendValid => frozen.frontend_valid,
    };
    let executed_oracle = oracle_total(&oracle).saturating_sub(oracle.not_requested);
    let executable_vm_cases = frozen.vm_full
        + frozen.vm_unsupported
        + frozen.vm_runtime
        + frozen.vm_timeout
        + frozen.vm_verifier
        + frozen.vm_fallback;
    let manifest_cases = manifest.document.cases.len();
    let generic_total = plan_total(&generic);
    let physical_total = plan_total(&physical);
    SummaryRecord {
        schema_version: SCHEMA_VERSION,
        requested_oracle_policy: oracle_policy,
        executed_oracle_policy: oracle_policy,
        denominators: DenominatorReconciliation {
            manifest_cases,
            emitted_cases: manifest_cases,
            oracle_classified_cases: oracle_total(&oracle),
            generic_classified_cases: generic_total,
            physical_classified_cases: physical_total,
            frozen_frontend_cases: frozen.frontend_valid + frozen.frontend_rejected,
            frozen_oracle_cases: frozen.oracle_match + frozen.oracle_mismatch,
            executable_vm_cases,
            requested_oracle_cases: expected_oracle,
            executed_oracle_cases: executed_oracle,
            selected_reconciled: reconciliation(manifest_cases == frozen.selected),
            frozen_frontend_reconciled: reconciliation(
                frozen.frontend_valid + frozen.frontend_rejected == frozen.selected,
            ),
            frozen_oracle_reconciled: reconciliation(
                frozen.oracle_match + frozen.oracle_mismatch == frozen.oracle_eligible,
            ),
            executable_vm_reconciled: reconciliation(executable_vm_cases == frozen.frontend_valid),
            oracle_policy_reconciled: reconciliation(executed_oracle == expected_oracle),
            plan_denominators_reconciled: reconciliation(
                generic_total == manifest_cases && physical_total == manifest_cases,
            ),
        },
        frozen,
        oracle,
        generic,
        physical,
    }
}

const fn reconciliation(condition: bool) -> ReconciliationStatus {
    if condition {
        ReconciliationStatus::Reconciled
    } else {
        ReconciliationStatus::Drift
    }
}

fn frozen_counts(manifest: &LoadedManifest) -> FrozenCounts {
    let mut counts = FrozenCounts {
        selected: manifest.document.cases.len(),
        ..FrozenCounts::default()
    };
    for case in &manifest.document.cases {
        match case.frontend_status {
            FrozenFrontendStatus::Valid => counts.frontend_valid += 1,
            FrozenFrontendStatus::Rejected => counts.frontend_rejected += 1,
        }
        match case.vm_status {
            FrozenVmStatus::NotRunFrontendRejected => {}
            FrozenVmStatus::FullVm => {
                counts.oracle_eligible += 1;
                counts.vm_full += 1;
            }
            FrozenVmStatus::Unsupported => counts.vm_unsupported += 1,
            FrozenVmStatus::Runtime => counts.vm_runtime += 1,
            FrozenVmStatus::Timeout => counts.vm_timeout += 1,
            FrozenVmStatus::Verifier => counts.vm_verifier += 1,
            FrozenVmStatus::Fallback => counts.vm_fallback += 1,
        }
        match case.oracle_status {
            FrozenOracleStatus::Match => counts.oracle_match += 1,
            FrozenOracleStatus::Mismatch => counts.oracle_mismatch += 1,
            FrozenOracleStatus::NotRun => {}
        }
    }
    counts
}

fn oracle_total(counts: &OracleCounts) -> usize {
    counts.not_requested
        + counts.success
        + counts.frontend_rejected
        + counts.runtime_error
        + counts.timeout
        + counts.internal_error
}

fn plan_total(counts: &PlanCounts) -> usize {
    counts.not_run_frontend_rejected
        + counts.not_run_oracle_unavailable
        + counts.frontend_rejected
        + counts.vm_correct
        + counts.mismatch
        + counts.comparison_unavailable
        + counts.unsupported
        + counts.realization_error
        + counts.compile_error
        + counts.runtime_error
        + counts.timeout
        + counts.verifier_error
        + counts.fallback
        + counts.physical_prepare_error
        + counts.internal_error
}
