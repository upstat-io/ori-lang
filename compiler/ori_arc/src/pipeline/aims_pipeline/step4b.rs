//! Step-4b burden emission: the legacy walk invocation, its disable toggle,
//! and the post-emission debug dumps.

use std::sync::LazyLock;

use crate::ir::ArcFunction;

use super::AimsPipelineConfig;

/// `ORI_DISABLE_BURDEN_OPS=1` skips `emit_burden_ops` at Step 4b; the
/// predicate-stack realization path runs as in the pre-burden baseline. Read
/// once at first access; permanent empty-harness parity + bisection flag.
static BURDEN_OPS_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_BURDEN_OPS").as_deref() == Ok("1"));

/// `ORI_DUMP_AFTER_BURDEN=1` dumps each function's ARC IR to stderr immediately
/// after Step 4b `emit_burden_ops`, before any realization. Surfaces the
/// faithful Phase-5 `BurdenInc` / `BurdenDec*` emission for VF-1 residual
/// localization (the post-realize `ORI_DUMP_AFTER_ARC` cannot show pre-realize
/// burden placement). Read once at first access; zero overhead when unset.
static DUMP_AFTER_BURDEN: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DUMP_AFTER_BURDEN").as_deref() == Ok("1"));

/// `ORI_DUMP_AFTER_BURDEN_ELIM=1` dumps each function's ARC IR after running
/// Phase-6 `eliminate_burden_ops` on a CLONE of the post-Step-4b function,
/// before any predicate-stack realization. Surfaces which `BurdenInc` /
/// `BurdenDec*` survive DP-2/DP-3 elimination — the ledger that Phase-7
/// mechanical lowering would turn into real `RcInc`/`RcDec`. Read once at
/// first access; zero overhead when unset.
static DUMP_AFTER_BURDEN_ELIM: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DUMP_AFTER_BURDEN_ELIM").as_deref() == Ok("1"));

/// Whether Step-4b burden emission is enabled (`ORI_DISABLE_BURDEN_OPS` unset).
pub(super) fn legacy_emission_enabled() -> bool {
    !*BURDEN_OPS_DISABLED
}

/// Step 4b: emit `BurdenInc`/`BurdenDec*` ops from the converged state map.
///
/// `ORI_DISABLE_BURDEN_OPS=1` skips emission entirely.
///
/// The live `TypeRegistry` is threaded so `lookup_burden` resolves both the
/// builtin (`BURDEN_TABLE`) partition AND user-side `[T]` / `{K:V}` / `Set<T>` /
/// closure-env / struct burdens (`TypeRegistry::burden(idx)`); the burden path
/// is the sole real-RC emitter, so the field-grain `BurdenDecPartial` /
/// `BurdenDecField` / `BurdenDecVariant` codegen glue is exercised.
pub(super) fn emit_burden_ops_step(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
    derived_ownership: &[crate::ownership::DerivedOwnership],
    // Converged Step-4 state map — supplies `apply_result_aliases` to the
    // §1.9 unified alias-table construction inside the burden walk (the
    // sibling-union cross-block identity). Spec: Annex E §AIMS.
    state_map: &crate::aims::intraprocedural::AimsStateMap,
) {
    if !legacy_emission_enabled() {
        return;
    }
    let _span = tracing::info_span!("emit_burden_ops").entered();
    let immortals = crate::aims::immortal::detect_immortals(func, config.interner);
    let _burden_ctx = crate::lower::burden_lower::emit_burden_ops(
        func,
        config.type_registry,
        derived_ownership,
        &immortals,
        config.contracts,
        state_map.apply_result_aliases(),
        true,
        config.interner,
    );
}

/// Dump the post-Step-4b ARC IR to stderr when `ORI_DUMP_AFTER_BURDEN=1`.
pub(super) fn dump_after_burden(func: &ArcFunction, config: &AimsPipelineConfig<'_>) {
    if !*DUMP_AFTER_BURDEN {
        return;
    }
    eprintln!(
        "=== ARC IR after emit_burden_ops ===\n{}",
        crate::ir::format::format_function(func, config.pool, config.interner)
    );
}

/// Dump the post-`eliminate_burden_ops` ARC IR to stderr when
/// `ORI_DUMP_AFTER_BURDEN_ELIM=1`. Operates on a clone so the live pipeline
/// IR is untouched; no-op when the flag is unset, and for a
/// class-ledger-replaced function (realization never runs Phase-6
/// elimination on the plan, so the dump would misreport the pipeline).
pub(super) fn dump_after_burden_elim(
    func: &ArcFunction,
    state_map: &crate::aims::intraprocedural::AimsStateMap,
    config: &AimsPipelineConfig<'_>,
) {
    if !*DUMP_AFTER_BURDEN_ELIM || func.class_ledger_emission {
        return;
    }
    let mut clone = func.clone();
    let same_alloc_reps =
        crate::aims::emit_rc::compute_same_alloc_reps(&clone, state_map.apply_result_aliases());
    crate::aims::realize::eliminate_burden_ops(
        &mut clone,
        state_map,
        &same_alloc_reps,
        config.contracts,
        config.interner,
    );
    eprintln!(
        "=== ARC IR after eliminate_burden_ops (clone) ===\n{}",
        crate::ir::format::format_function(&clone, config.pool, config.interner)
    );
}
