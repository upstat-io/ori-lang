//! AIMS pipeline implementation.
//!
//! Replaces the sequential analysis passes (borrow inference, liveness,
//! uniqueness, RC insertion, reset/reuse, RC elimination) with the unified
//! AIMS analysis + emission pipeline.
//!
//! # Pipeline (Section 10 — unified realization)
//!
//! **Interprocedural** (once across all functions):
//! 1. `aims::analyze_program()` — compute `MemoryContract` per function
//! 2. `aims::apply_ownership()` — populate `ArcParam.ownership`
//!
//! **Per-function** (steps 3–12):
//! 3. `compute_var_reprs()` — fill `ValueRepr` per variable
//! 3a. `aims::normalize_function()` — TRMC context region detection
//! 4. `aims::analyze_function()` — backward dataflow → converged state map
//! 5. `aims::realize_rc_reuse()` — Phase 1: `arg_ownership` + RC + reuse (pre-merge)
//! 5a. `aims::verify::fip::verify_fip_contract()` — FIP enforcement verification
//! 6. `verify()` — ARC IR sanity check
//! 7. `run_aims_verify()` — AIMS contract vs IR consistency
//! 8. `detect_tail_calls()` + `rewrite_tail_calls()`
//! 9. `merge_blocks()` — CFG cleanup
//! 10. `aims::realize_annotations()` — Phase 2: COW + drop hints (post-merge)
//! 11. `verify()` — final sanity check
//! 12. FBIP enforcement — read-only diagnostic

mod batch;
mod postprocess;
mod trmc;

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::pipeline::rc_count;
use crate::ArcClassification;

// Re-export batch entry points used by pipeline/mod.rs.
pub(crate) use batch::{apply_aims_ownership, run_aims_pipeline_all};

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
/// Uses existing `rc_count::count_rc_ops()` (SSOT for RC counting) plus
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
    // count_rc_ops() and string lookup when tracing is off.
    if tracing::enabled!(target: "ori_arc::aims::pipeline", tracing::Level::INFO) {
        let fn_name = interner.lookup(func.name);
        let rc = rc_count::count_rc_ops(func);
        let blocks = func.blocks.len();
        let vars = func.var_types.len();
        tracing::info!(
            target: "ori_arc::aims::pipeline",
            function = fn_name,
            phase,
            rc_incs = rc.inc,
            rc_decs = rc.dec,
            blocks,
            vars,
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
    pub pool: &'a ori_types::Pool,
    pub interner: &'a ori_ir::StringInterner,
    pub builtins: &'a BuiltinOwnershipSets,
    pub verify_arc: bool,
    /// Optional checkpoint observer for snapshot capture.
    /// When `Some`, called after each pipeline step with the current
    /// function state and phase name. When `None`, zero overhead.
    pub observer: Option<&'a CheckpointObserver<'a>>,
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

    // Phase 2: COW + drop hints (post-merge).
    {
        let _span = tracing::info_span!("realize_annotations").entered();
        crate::aims::realize::realize_annotations(
            func,
            &state_map,
            config.interner,
            config.pool,
            config.contracts,
            config.builtins,
            &mut result,
        );
    }
    trace_pipeline_checkpoint(
        func,
        "realize_annotations",
        config.interner,
        config.observer,
    );
    func.cow_annotations = result.cow_annotations;
    func.drop_hints = result.drop_hints;

    // BUG-04-118: Post-realize cleanup of redundant project-alias decs.
    // Removes (N - K) decs where K = explicit RcInc and N = explicit
    // RcDec on a project-only class whose source's drop chain already
    // provides one type-driven dec.
    {
        let _span = tracing::info_span!("cleanup_redundant_project_alias_decs").entered();
        crate::aims::realize::cleanup_redundant_project_alias_decs(
            func,
            &state_map,
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

    // Final verification + FBIP.
    let problems = postprocess::emit_postprocess(func, config)?;

    Ok(AimsPipelineResult {
        problems,
        missed_reuses,
        was_trmc_rewritten: trmc_rewrite_survived,
    })
}

/// Step 5a: FIP enforcement pre-check (Section 12.3).
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
    let Some(contract) = config.contracts.get(&func.name) else {
        return Ok(());
    };
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
