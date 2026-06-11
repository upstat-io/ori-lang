//! AIMS pipeline implementation — the unified AIMS analysis + emission
//! pipeline (borrow inference, liveness, uniqueness, RC insertion,
//! reset/reuse, RC elimination as one lattice-driven flow).
//!
//! # Pipeline (unified realization)
//!
//! **Interprocedural** (once across all functions):
//! 1. `aims::analyze_program` — compute `MemoryContract` per function
//! 2. `aims::apply_ownership` — populate `ArcParam.ownership`
//!
//! **Per-function** (steps 3–12):
//! 3. `compute_var_reprs` — fill `ValueRepr` per variable
//! 3a. `aims::normalize_function` — TRMC context region detection
//! 4. `aims::analyze_function` — backward dataflow → converged state map
//! 5. `aims::realize_rc_reuse` — Phase 1: `arg_ownership` + RC + reuse (pre-merge)
//! 5a. `aims::verify::fip::verify_fip_contract` — FIP enforcement verification
//! 6. `verify` — ARC IR sanity check
//! 7. `run_aims_verify` — AIMS contract vs IR consistency
//! 8. `detect_tail_calls` + `rewrite_tail_calls`
//! 9. `merge_blocks` — CFG cleanup
//! 10. `aims::realize_annotations` — Phase 2: COW + drop hints (post-merge)
//! 11. `verify` — final sanity check
//! 12. FBIP enforcement — read-only diagnostic

mod batch;
mod postprocess;
mod trmc;

use std::sync::LazyLock;

use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{ContractMapExt, MemoryContract};
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr};
use crate::lower::ArcProblem;
use crate::ownership::AnnotatedSig;
use crate::pipeline::rc_count;
use crate::ArcClassification;

// Re-export batch entry points used by pipeline/mod.rs.
pub(crate) use batch::{apply_aims_ownership, run_aims_pipeline_all};

/// `ORI_DISABLE_BURDEN_OPS=1` skips `emit_burden_ops` at Step 4b; the
/// predicate-stack realization path runs as in the pre-burden baseline. Read
/// once at first access; permanent empty-harness parity + bisection flag.
static BURDEN_OPS_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_BURDEN_OPS").as_deref() == Ok("1"));

/// Predicate-stack RC emission is RETIRED by default: the burden path is the
/// sole real-RC emitter. The lattice-realized `BurdenInc → RcInc` /
/// `BurdenDec → RcDec` lowering (Phase 7) replaces the predicate-stack
/// `RcInc`/`RcDec` walk (Phase 1 + edge / dead / project-escape cleanup),
/// per the canonical burden RC-emission path (Spec: Annex E §AIMS).
///
/// `ORI_DISABLE_PREDICATE_STACK_RC=0` is a transitional escape hatch that
/// restores the legacy predicate-stack emitter for migration-time validation
/// and bisection; it is removed when the predicate-stack code is deleted. With
/// the flag set to `0`, burden ops revert to codegen no-ops and the predicate
/// stack emits RC as before.
///
/// Seeds the per-pipeline `AimsPipelineConfig.predicate_stack_rc_disabled` at
/// the production entry points; tests set the config field directly (the env
/// read is process-global via `LazyLock`, so a per-test toggle is impossible).
/// Read once at first access.
pub(crate) static PREDICATE_STACK_RC_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_PREDICATE_STACK_RC").as_deref() != Ok("0"));

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

/// Callback invoked at each pipeline checkpoint.
///
/// Receives the current function state and the phase name. Used by
/// snapshot tests to capture ARC IR at pipeline boundaries. Production
/// code passes `None` — zero overhead when not capturing.
pub type CheckpointObserver<'a> = dyn Fn(&ArcFunction, &str /* phase */) + 'a;

/// Emit a pipeline checkpoint event for `bisect-passes.sh` consumption.
///
/// Uses `info` level on the `ori_arc::aims::pipeline` target so it can be
/// captured with `ORI_LOG=ori_arc::aims::pipeline=info` without overwhelming
/// verbosity. This is intentionally different from the existing
/// `trace_phase_snapshot` in `emit_unified.rs` which uses `trace!` on
/// `ori_arc::aims::realize` for finer-grained realization-step snapshots.
///
/// Uses existing `rc_count::count_rc_ops` (SSOT for RC counting) plus
/// structural metrics (`blocks`, `vars`) to detect phases that change
/// CFG structure without altering RC totals.
///
/// When `observer` is `Some`, invokes the callback with the current function
/// and phase name — used by snapshot tests to capture ARC IR at pipeline
/// boundaries. When `None`, zero overhead beyond the existing tracing check.
pub(crate) fn trace_pipeline_checkpoint(
    func: &ArcFunction,
    phase: &str,
    interner: &ori_ir::StringInterner,
    observer: Option<&CheckpointObserver<'_>>,
) {
    // Early exit when the pipeline target is disabled — avoids the cost of
    // count_rc_opsand string lookup when tracing is off.
    if tracing::enabled!(target: "ori_arc::aims::pipeline", tracing::Level::INFO) {
        let fn_name = interner.lookup(func.name);
        let rc = rc_count::count_rc_ops(func);
        let blocks = func.blocks.len();
        let vars = func.var_types.len();
        // Per-block burden-op sites — pairs with the `verify_burden_balance`
        // imbalance trace to localize WHICH pipeline phase relocated/dropped a
        // burden op between checkpoints (merge_blocks / tail-call rewrite).
        let mut burden_sites: Vec<String> = Vec::new();
        for (b, block) in func.blocks.iter().enumerate() {
            for (i, instr) in block.body.iter().enumerate() {
                let kind = match instr {
                    ArcInstr::BurdenInc { var } => format!("bb{b}.{i}:binc%{}", var.index()),
                    ArcInstr::BurdenDec { var }
                    | ArcInstr::BurdenDecPartial { var, .. }
                    | ArcInstr::BurdenDecVariant { var } => {
                        format!("bb{b}.{i}:bdec%{}", var.index())
                    }
                    _ => continue,
                };
                burden_sites.push(kind);
            }
        }
        tracing::info!(
            target: "ori_arc::aims::pipeline",
            function = fn_name,
            phase,
            rc_incs = rc.inc,
            rc_decs = rc.dec,
            blocks,
            vars,
            burden_sites = %burden_sites.join(" "),
            "AIMS phase checkpoint"
        );
    }
    // Invoke observer if present (snapshot capture).
    if let Some(obs) = observer {
        obs(func, phase);
    }
}

/// Configuration for the AIMS per-function pipeline.
///
/// Bundles the shared parameters that `run_aims_pipeline` needs, avoiding
/// the 7-parameter signature anti-pattern from the old pipeline.
pub(crate) struct AimsPipelineConfig<'a> {
    pub classifier: &'a dyn ArcClassification,
    pub contracts: &'a FxHashMap<Name, MemoryContract>,
    /// Names of the functions in this compilation unit (the analyzed set).
    ///
    /// Consumed by Site-8 `is_safe_non_sharing_callee` to distinguish a
    /// local analyzed callee (which IC-1 guarantees has a `MemoryContract`)
    /// from an FFI / external / DCE'd callee (legitimately absent). The
    /// `debug_assert!` fires when a callee in this set is missing from
    /// `contracts` — an IC-1 pipeline-ordering violation.
    pub func_names: &'a FxHashSet<Name>,
    pub pool: &'a ori_types::Pool,
    pub interner: &'a ori_ir::StringInterner,
    pub builtins: &'a BuiltinOwnershipSets,
    pub verify_arc: bool,
    /// Optional checkpoint observer for snapshot capture.
    /// When `Some`, called after each pipeline step with the current
    /// function state and phase name. When `None`, zero overhead.
    pub observer: Option<&'a CheckpointObserver<'a>>,
    /// Annotated function signatures (borrow inference output).
    ///
    /// Consumed by per-variable derived-ownership inference
    /// (`borrow::infer_derived_ownership`).
    pub sigs: &'a FxHashMap<Name, AnnotatedSig>,
    /// Type registry used by the burden-emission walker
    /// (`lower::burden_lower::emit_burden_ops`). Carried per AIMS Invariant 5
    /// ("unified model — new capabilities extend a lattice dimension OR a
    /// contract field OR feed the lattice-driven analysis as a typed pre-pass
    /// input"). Call sites pass either the live module `TypeRegistry` (`oric`
    /// codegen path) or an empty placeholder (`TypeRegistry::default`).
    pub type_registry: &'a TypeRegistry,
    /// Probe flag (per the canonical burden RC-emission path, Spec: Annex E §AIMS):
    /// when `true`, suppress the predicate-stack `RcInc`/`RcDec` emission and
    /// instead mechanically lower surviving `BurdenInc → RcInc` /
    /// `BurdenDec → RcDec` (Phase 7), proving the burden path is a complete
    /// standalone RC emitter. Production entry points seed this from
    /// [`PREDICATE_STACK_RC_DISABLED`] (the `ORI_DISABLE_PREDICATE_STACK_RC`
    /// env `LazyLock`); tests set it directly so the env read stays single
    /// process-global. Default `false` — predicate-stack RC as today.
    pub predicate_stack_rc_disabled: bool,
}

/// Result of `run_aims_pipeline` for a single function.
pub(crate) struct AimsPipelineResult {
    pub problems: Vec<ArcProblem>,
    /// Post-emission missed reuse count from `FipEvidence`. Used by the
    /// second pass to compute `may_deallocate` (> 0) and to re-verify
    /// `Bounded(n)` contracts with accurate counts.
    pub missed_reuses: usize,
    /// Whether this function was TRMC-rewritten (and the rewrite survived
    /// both structural and semantic verification). Used by the second pass
    /// to mark `has_unbounded_stack = false` on refreshed contracts.
    pub was_trmc_rewritten: bool,
}

/// Run the AIMS pipeline on a single function (steps 3–12).
///
/// Called from within `run_arc_pipeline` when the `aims` feature is active.
/// Interprocedural contracts must already be computed and passed via `config`.
///
/// Returns `Err` if ARC IR verification fails under explicit verification
/// mode (`ORI_VERIFY_ARC=1`). Verification errors are ICEs — they indicate
/// internal compiler bugs, not user-facing issues.
pub(crate) fn run_aims_pipeline(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<AimsPipelineResult, Vec<crate::verify::VerifyError>> {
    // Steps 3–3a: compute var_reprs, detect immortals, normalize with
    // TRMC rewrite loop (idempotent — at most 2 iterations).
    let (norm_result, immortals, did_trmc_transform, pre_trmc_func) =
        trmc::normalize_with_trmc(func, config);
    trace_pipeline_checkpoint(
        func,
        "normalize_with_trmc_complete",
        config.interner,
        config.observer,
    );

    // Step 3b: compute DerivedOwnership for the burden walker (consumed at Step 4b).
    let derived_ownership = crate::borrow::infer_derived_ownership(func, config.sigs);

    // Intraprocedural analysis → converged state map.
    let state_map = {
        let _span = tracing::info_span!("analyze_function").entered();
        crate::aims::intraprocedural::analyze_function(
            func,
            config.classifier,
            config.contracts,
            &norm_result.context_regions,
            immortals,
        )
    };
    trace_pipeline_checkpoint(func, "analyze_function", config.interner, config.observer);

    // Step 4a: TRMC semantic soundness verification.
    let (state_map, trmc_rewrite_survived) =
        trmc::verify_trmc_soundness(func, state_map, did_trmc_transform, pre_trmc_func, config);
    trace_pipeline_checkpoint(
        func,
        "verify_trmc_soundness",
        config.interner,
        config.observer,
    );

    // Step 4b-prelude — populate arg_ownership AFTER analyze_function (so
    // post-convergence's payload-edge analysis sees empty arg_ownership,
    // preserving class_payload_of computation) but BEFORE emit_burden_ops (so
    // burden_lower observes converged arg_ownership at emission time — closes
    // the VF-1 imbalance per AIMS TF-3 / RL-2). emit_arg_ownership is
    // idempotent; `realize_rc_reuse` does not re-invoke it because arg_ownership
    // is already populated here.
    {
        let _span = tracing::debug_span!("emit_arg_ownership_prelude").entered();
        crate::aims::emit_rc::arg_ownership::emit_arg_ownership(
            func,
            config.contracts,
            config.interner,
            config.builtins,
            config.pool,
        );
    }
    trace_pipeline_checkpoint(
        func,
        "emit_arg_ownership_prelude",
        config.interner,
        config.observer,
    );

    // Step 4b: emit BurdenInc/BurdenDec ops based on converged state map.
    emit_burden_ops_step(func, config, &derived_ownership, &state_map);
    trace_pipeline_checkpoint(func, "emit_burden_ops", config.interner, config.observer);

    if *DUMP_AFTER_BURDEN {
        eprintln!(
            "=== ARC IR after emit_burden_ops ===\n{}",
            crate::ir::format::format_function(func, config.pool, config.interner)
        );
    }

    dump_after_burden_elim(func, &state_map, config);

    // Phase 1: RC + reuse + arg_ownership (pre-merge).
    let mut result = {
        let _span = tracing::info_span!("realize_rc_reuse").entered();
        crate::aims::realize::realize_rc_reuse(
            func,
            &state_map,
            config.contracts,
            config.interner,
            config.builtins,
            config.pool,
            config.predicate_stack_rc_disabled,
        )
    };
    trace_pipeline_checkpoint(func, "realize_rc_reuse", config.interner, config.observer);

    // Post-emission missed_reuses count for the second pass (FP² Theorem 2).
    let missed_reuses = result.fip_evidence.missed_reuses;

    // Step 5a: FIP enforcement pre-check.
    fip_precheck(func, config, &result)?;
    trace_pipeline_checkpoint(
        func,
        "verify_fip_contract",
        config.interner,
        config.observer,
    );

    // Set canonicalize cross-dim fires from converged state analysis.
    result.synergy_metrics.canonicalize_cross_fires = state_map.count_cross_dim_states();

    // Verify, AIMS-verify, tail calls, merge.
    postprocess::verify_and_merge(func, config)?;

    apply_phase_2_annotations(func, &state_map, config, &mut result);

    // Final verification + FBIP.
    let problems = postprocess::emit_postprocess(func, config)?;

    Ok(AimsPipelineResult {
        problems,
        missed_reuses,
        was_trmc_rewritten: trmc_rewrite_survived,
    })
}

/// Step 4b: emit `BurdenInc`/`BurdenDec*` ops from the converged state map.
///
/// `ORI_DISABLE_BURDEN_OPS=1` skips emission entirely so the predicate-stack
/// realization path runs unchanged.
///
/// The DEFAULT path (predicate stack ON) passes an EMPTY `TypeRegistry` so
/// `lookup_burden` resolves only the builtin (`BURDEN_TABLE`) partition;
/// user-side `[T]` / `{K:V}` / `Set<T>` / closure-env / struct burdens
/// (`TypeRegistry::burden(idx)`) return `None`, so no field-grain
/// `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` ops are emitted on
/// the default path. Those field-grain ops have UNCONDITIONAL codegen glue
/// (`instr_dispatch.rs` `struct_gep`s the aggregate) that is unsound on a
/// by-value aggregate phi (Spec: Annex E §AIMS RE / codegen AB-5), so surfacing
/// them on the default path breaks byte-identity. The live registry is threaded
/// ONLY under the predicate-stack-disabled probe, where the burden path is the
/// sole real-RC emitter and the field-grain codegen glue is exercised by the
/// probe corpus.
fn emit_burden_ops_step(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
    derived_ownership: &[crate::ownership::DerivedOwnership],
    // Converged Step-4 state map — supplies `apply_result_aliases` to the
    // §1.9 unified alias-table construction inside the burden walk (the
    // sibling-union cross-block identity). Spec: Annex E §AIMS.
    state_map: &crate::aims::intraprocedural::AimsStateMap,
) {
    if *BURDEN_OPS_DISABLED {
        return;
    }
    let _span = tracing::info_span!("emit_burden_ops").entered();
    let immortals = crate::aims::immortal::detect_immortals(func, config.interner);
    let empty_registry: ori_types::TypeRegistry;
    let burden_registry = if config.predicate_stack_rc_disabled {
        config.type_registry
    } else {
        empty_registry = ori_types::TypeRegistry::default();
        &empty_registry
    };
    let _burden_ctx = crate::lower::burden_lower::emit_burden_ops(
        func,
        burden_registry,
        derived_ownership,
        &immortals,
        config.contracts,
        state_map.apply_result_aliases(),
        config.predicate_stack_rc_disabled,
        config.interner,
    );
}

/// Dump the post-`eliminate_burden_ops` ARC IR to stderr when
/// `ORI_DUMP_AFTER_BURDEN_ELIM=1`. Operates on a clone so the live pipeline
/// IR is untouched; no-op when the flag is unset.
fn dump_after_burden_elim(
    func: &ArcFunction,
    state_map: &crate::aims::intraprocedural::AimsStateMap,
    config: &AimsPipelineConfig<'_>,
) {
    if !*DUMP_AFTER_BURDEN_ELIM {
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
        *PREDICATE_STACK_RC_DISABLED,
    );
    eprintln!(
        "=== ARC IR after eliminate_burden_ops (clone) ===\n{}",
        crate::ir::format::format_function(&clone, config.pool, config.interner)
    );
}

/// Phase 2: COW + drop hints (post-merge) followed by post-realize
/// cleanup of redundant project-alias decs.
fn apply_phase_2_annotations(
    func: &mut ArcFunction,
    state_map: &crate::aims::intraprocedural::AimsStateMap,
    config: &AimsPipelineConfig<'_>,
    result: &mut crate::aims::realize::RealizationResult,
) {
    {
        let _span = tracing::info_span!("realize_annotations").entered();
        let env = crate::aims::realize::AnnotationEnv {
            state_map,
            interner: config.interner,
            pool: config.pool,
            contracts: config.contracts,
            builtins: config.builtins,
            func_names: config.func_names,
        };
        crate::aims::realize::realize_annotations(func, &env, result);
    }
    trace_pipeline_checkpoint(
        func,
        "realize_annotations",
        config.interner,
        config.observer,
    );
    func.cow_annotations = std::mem::take(&mut result.cow_annotations);
    func.drop_hints = std::mem::take(&mut result.drop_hints);

    {
        let _span = tracing::info_span!("cleanup_redundant_project_alias_decs").entered();
        crate::aims::realize::cleanup_redundant_project_alias_decs(
            func,
            state_map,
            config.pool,
            config.interner,
        );
    }
    trace_pipeline_checkpoint(
        func,
        "cleanup_redundant_project_alias_decs",
        config.interner,
        config.observer,
    );
}

/// Step 5a: FIP enforcement pre-check.
///
/// Cross-checks `FipContract` against realization evidence. At this point,
/// the contract has optimistic `may_deallocate=false` from interprocedural
/// analysis — `CertifiedButHasMissedReuses` mismatches are expected and
/// will be corrected by the second pass. But structural violations
/// (`CertifiedButUnboundedStack`, `BoundedExceeded`) are genuine bugs
/// that should be caught immediately.
fn fip_precheck(
    func: &ArcFunction,
    config: &AimsPipelineConfig<'_>,
    result: &crate::aims::realize::RealizationResult,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let contract = config.contracts.get_required(&func.name, "fip_precheck");
    let fip_errors =
        crate::aims::verify::fip::verify_fip_contract(func.name, contract, &result.fip_evidence);
    let mut structural_errors = Vec::new();
    for e in &fip_errors {
        use crate::aims::verify::fip::FipVerificationError;
        match e {
            FipVerificationError::CertifiedButHasMissedReuses { .. } => {
                // Expected: may_deallocate is stale (optimistic default).
                // Second pass will recompute contract.fip and re-verify.
                tracing::debug!("FIP pre-check (will recompute in second pass): {e}");
            }
            FipVerificationError::CertifiedButUnboundedStack { .. }
            | FipVerificationError::BoundedExceeded { .. } => {
                // Genuine bug: structural violations are known at
                // interprocedural analysis time, not post-emission facts.
                tracing::error!("FIP verification failed: {e}");
                if config.verify_arc {
                    structural_errors.push(crate::verify::VerifyError::FipStructural {
                        message: e.to_string(),
                    });
                }
            }
        }
    }
    if structural_errors.is_empty() {
        Ok(())
    } else {
        Err(structural_errors)
    }
}
