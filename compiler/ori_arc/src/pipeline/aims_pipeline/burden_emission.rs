//! Step-4b burden-op disable toggle and the post-emission debug dumps.

use std::sync::LazyLock;

use crate::ir::ArcFunction;

use super::AimsPipelineConfig;

/// `ORI_DISABLE_BURDEN_OPS=1` declines class-ledger replacement's burden-op
/// emission at Step 4b. Class-ledger replacement is the sole production
/// RC emitter, so setting this leaves the function unreplaced and realization
/// fails loud rather than synthesizing fallback RC. This is a negative-pin
/// probe, never a build mode. Read once at first access.
static BURDEN_OPS_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    report_burden_ops_toggle(std::env::var("ORI_DISABLE_BURDEN_OPS").as_deref() == Ok("1"))
});

fn report_burden_ops_toggle(disabled: bool) -> bool {
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_BURDEN_OPS",
            effect = "decline class-ledger burden-op emission",
            "ablation toggle fired"
        );
    }
    disabled
}

/// `ORI_DUMP_AFTER_BURDEN=1` dumps each function's ARC IR to stderr immediately
/// after Step 4b burden emission, before any realization. Surfaces the
/// faithful class-ledger `BurdenInc` / `BurdenDec*` emission for VF-1 residual
/// localization (the post-realize `ORI_DUMP_AFTER_ARC` cannot show pre-realize
/// burden placement). Read once at first access; zero overhead when unset.
static DUMP_AFTER_BURDEN: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DUMP_AFTER_BURDEN").as_deref() == Ok("1"));

/// `ORI_DUMP_AFTER_BURDEN_ELIM=1` dumps each function's live post-Step-4b ARC
/// IR to stderr. Class-ledger replacement is the sole production Step-4b RC
/// emitter and lowers mechanically at Phase 7 with no separate elimination
/// pass, so this observes the same live state as `dump_after_burden`; the
/// flag name is retained for CLI continuity. Read once at first access; zero
/// overhead when unset.
static DUMP_AFTER_CLASS_LEDGER_EMISSION_COMPAT: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DUMP_AFTER_BURDEN_ELIM").as_deref() == Ok("1"));

/// Whether Step-4b burden emission is enabled (`ORI_DISABLE_BURDEN_OPS` unset).
pub(super) fn burden_ops_enabled() -> bool {
    !*BURDEN_OPS_DISABLED
}

/// Dump the post-Step-4b ARC IR to stderr when `ORI_DUMP_AFTER_BURDEN=1`.
pub(super) fn dump_after_burden(func: &ArcFunction, config: &AimsPipelineConfig<'_>) {
    if !*DUMP_AFTER_BURDEN {
        return;
    }
    eprintln!(
        "=== ARC IR after class_ledger_emission ===\n{}",
        crate::ir::format::format_function(func, config.pool, config.interner)
    );
}

/// Dump the live post-Step-4b ARC IR to stderr when
/// `ORI_DUMP_AFTER_BURDEN_ELIM=1`. Pure observer: no clone, no elimination
/// execution — class-ledger replacement's plan is the live function state.
pub(super) fn dump_after_class_ledger_emission_compat(
    func: &ArcFunction,
    config: &AimsPipelineConfig<'_>,
) {
    if !*DUMP_AFTER_CLASS_LEDGER_EMISSION_COMPAT {
        return;
    }
    eprintln!(
        "=== ARC IR after class_ledger_emission (ORI_DUMP_AFTER_BURDEN_ELIM compatibility) ===\n{}",
        crate::ir::format::format_function(func, config.pool, config.interner)
    );
}

#[cfg(test)]
mod toggle_tests {
    crate::test_helpers::ablation_env_event_test!(
        burden_ops_toggle_reports_effect,
        "ORI_DISABLE_BURDEN_OPS",
        "decline class-ledger burden-op emission",
        || !super::burden_ops_enabled(),
    );
}
