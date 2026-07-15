//! AIMS pipeline implementation — backend-neutral ownership calculus and
//! logical ownership-event realization (borrow inference, liveness,
//! uniqueness, release/retain obligations, and storage reuse as one
//! lattice-driven flow).
//!
//! The current IR carrier names `RcInc` and `RcDec` are transitional logical
//! event spellings. They do not require a backend to use a reference-counted
//! heap representation; each physical projection realizes the frozen events
//! through its own representation plan.
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
//! 5. `aims::realize_rc_reuse` — Phase 1: argument ownership + logical
//!    retain/release events + reuse (pre-merge)
//! 5a. `aims::verify::fip::verify_fip_contract` — FIP enforcement verification
//! 6. `verify` — ARC IR sanity check
//! 7. `run_aims_verify` — AIMS contract vs IR consistency
//! 8. `detect_tail_calls` + `rewrite_tail_calls`
//! 9. `merge_blocks` — CFG cleanup
//! 10. `aims::realize_annotations` — Phase 2: COW + drop hints (post-merge)
//! 11. `verify` — final sanity check
//! 12. FBIP enforcement — read-only diagnostic

mod batch;
mod burden_emission;
mod postprocess;
mod trmc;

use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{ContractMapExt, MemoryContract};
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr};
use crate::lower::ArcProblem;
use crate::pipeline::rc_count;
use crate::ArcClassification;

// Re-export batch entry points used by pipeline/mod.rs.
pub(crate) use batch::{
    run_aims_pipeline_all_with_external_contracts, run_aims_pipeline_all_with_observer,
};

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
    // count_rc_ops and string lookup when tracing is off.
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
    /// Local and producer-validated external callables whose exact contracts
    /// take precedence over same-spelled builtin ownership heuristics.
    pub exact_callables: &'a FxHashSet<Name>,
    pub pool: &'a ori_types::Pool,
    pub interner: &'a ori_ir::StringInterner,
    pub builtins: &'a BuiltinOwnershipSets,
    pub verify_arc: bool,
    /// Optional checkpoint observer for snapshot capture.
    /// When `Some`, called after each pipeline step with the current
    /// function state and phase name. When `None`, zero overhead.
    pub observer: Option<&'a CheckpointObserver<'a>>,
    /// Type registry used by class-ledger burden-op replacement
    /// (`aims::class_ledger::apply_class_ledger_replacement`). Carried per
    /// AIMS Invariant 5 ("unified model — new capabilities extend a lattice
    /// dimension OR a contract field OR feed the lattice-driven analysis as a
    /// typed pre-pass input"). Call sites pass either the live module
    /// `TypeRegistry` (`oric` codegen path) or an empty placeholder
    /// (`TypeRegistry::default`).
    pub type_registry: &'a TypeRegistry,
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
/// Called only by the closed-program batch owner after interprocedural facts
/// are complete and passed via `config`.
///
/// Returns `Err` if ARC IR verification fails under explicit verification
/// mode (`ORI_VERIFY_ARC=1`). Verification errors are ICEs — they indicate
/// internal compiler bugs, not user-facing issues.
pub(crate) fn run_aims_pipeline(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<AimsPipelineResult, Vec<crate::verify::VerifyError>> {
    // Single-function users may bypass the batch entry. Resolve exactly once
    // for a new artifact, or validate the already-frozen AIMS input table.
    crate::aims::primitive::ensure_primitive_facts(func, config.classifier)?;

    // Steps 3–3a: compute var_reprs, detect immortals, normalize with
    // TRMC rewrite loop (idempotent — at most 2 iterations).
    let (norm_result, immortals, did_trmc_transform, pre_trmc_func) =
        trmc::normalize_with_trmc(func, config)?;
    trace_pipeline_checkpoint(
        func,
        "normalize_with_trmc_complete",
        config.interner,
        config.observer,
    );
    // Normalization preserves primitive destinations. Re-check exact coverage
    // so a future structural rewrite cannot silently orphan frozen facts.
    crate::aims::primitive::ensure_primitive_facts(func, config.classifier)?;

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
    let (mut state_map, trmc_rewrite_survived) =
        trmc::verify_trmc_soundness(func, state_map, did_trmc_transform, pre_trmc_func, config);
    trace_pipeline_checkpoint(
        func,
        "verify_trmc_soundness",
        config.interner,
        config.observer,
    );

    // Populate the birth-site partition side table on the converged state map
    // (after Step 4a: IR + state map are final; before Step 4b burden
    // emission). Read-only downstream per PL-5; admission per the T1
    // partition calculus (AimsProof.Partition).
    let birth_site_partition =
        crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition(
            func, &state_map,
        );
    state_map.set_birth_site_partition(birth_site_partition);

    // Step 4b-prelude — populate arg_ownership AFTER analyze_function (so
    // post-convergence's payload-edge analysis sees empty arg_ownership,
    // preserving its transitive-drop edge computation) but BEFORE class-ledger
    // replacement (so the replacement plan observes converged arg_ownership at
    // emission time — closes the VF-1 imbalance per AIMS TF-3 / RL-2).
    // emit_arg_ownership is idempotent; `realize_rc_reuse` does not re-invoke
    // it because arg_ownership is already populated here.
    {
        let _span = tracing::debug_span!("emit_arg_ownership_prelude").entered();
        crate::aims::emit_rc::arg_ownership::emit_arg_ownership_with_exact_callables(
            func,
            config.contracts,
            config.interner,
            config.builtins,
            config.pool,
            config.exact_callables,
        )?;
    }
    trace_pipeline_checkpoint(
        func,
        "emit_arg_ownership_prelude",
        config.interner,
        config.observer,
    );

    // Step 4b: class-ledger replacement is the sole production Step-4b RC
    // emitter. The analysis reads the pre-burden IR + converged state map
    // (before any burden ops enter the class event streams); the applied plan
    // lowers mechanically at Phase 7, so every planned inc is a surviving RC
    // op — emit the observability remarks unconditionally at the disposition
    // seam.
    crate::aims::class_ledger::apply_class_ledger_replacement(
        func,
        &state_map,
        config.contracts,
        config.type_registry,
        config.interner,
        burden_emission::burden_ops_enabled(),
    );
    crate::aims::realize::emit_survivor_remarks_all_kept(func, &state_map, config.interner);
    trace_pipeline_checkpoint(
        func,
        "class_ledger_emission",
        config.interner,
        config.observer,
    );

    burden_emission::dump_after_burden(func, config);
    burden_emission::dump_after_class_ledger_emission_compat(func, config);

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
            config.type_registry,
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

    // Tail-call lowering, unwind cleanup, reuse expansion, and block merging
    // may introduce aliases after the initial metadata computation. Their
    // allocation APIs preserve metadata eagerly; this owner-seam validation
    // recomputes expected values without mutating the authoritative tables, so
    // a missed allocator cannot be silently repaired before backend handoff.
    validate_metadata_checkpoint(func, config)?;

    apply_phase_2_annotations(func, &state_map, config, &mut result);

    // Final verification + FBIP.
    let problems = postprocess::emit_postprocess(func, config)?;

    Ok(AimsPipelineResult {
        problems,
        missed_reuses,
        was_trmc_rewritten: trmc_rewrite_survived,
    })
}

pub(super) fn validate_variable_metadata(
    func: &ArcFunction,
    classifier: &dyn crate::ArcClassification,
    pool: &ori_types::Pool,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let mut errors = Vec::new();
    if func.var_metadata_state != crate::ir::VariableMetadataState::Realized {
        errors.push(crate::verify::VerifyError::VariableMetadataUnrealized);
    }
    let expected_representations = crate::ir::compute_var_reprs(func, classifier, pool);
    errors.extend(representation_metadata_errors(
        func,
        &expected_representations,
    ));

    let expected_strategies =
        crate::ir::derive_var_rc_strategies(&expected_representations, &func.var_types, pool);
    errors.extend(rc_strategy_metadata_errors(func, &expected_strategies));

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_metadata_checkpoint(
    func: &ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    validate_variable_metadata(func, config.classifier, config.pool)?;
    trace_pipeline_checkpoint(
        func,
        "validate_variable_metadata",
        config.interner,
        config.observer,
    );
    Ok(())
}

fn representation_metadata_errors(
    func: &ArcFunction,
    expected: &[crate::ir::ValueRepr],
) -> Vec<crate::verify::VerifyError> {
    use crate::verify::VerifyError;

    if func.var_reprs.len() == func.var_types.len() {
        expected
            .iter()
            .zip(&func.var_reprs)
            .enumerate()
            .filter(|(_, (expected, found))| expected != found)
            .map(
                |(index, (&expected, &found))| VerifyError::VariableRepresentationMismatch {
                    var: variable_id(index),
                    expected,
                    found,
                },
            )
            .collect()
    } else {
        vec![VerifyError::VariableMetadataLength {
            table: "representation",
            variables: func.var_types.len(),
            entries: func.var_reprs.len(),
        }]
    }
}

fn rc_strategy_metadata_errors(
    func: &ArcFunction,
    expected: &[Option<crate::ir::RcStrategy>],
) -> Vec<crate::verify::VerifyError> {
    use crate::verify::VerifyError;

    if func.var_rc_strategies.len() == func.var_types.len() {
        expected
            .iter()
            .zip(&func.var_rc_strategies)
            .enumerate()
            .filter(|(_, (expected, found))| expected != found)
            .map(
                |(index, (&expected, &found))| VerifyError::VariableRcStrategyMismatch {
                    var: variable_id(index),
                    expected,
                    found,
                },
            )
            .collect()
    } else {
        vec![VerifyError::VariableMetadataLength {
            table: "RC-strategy",
            variables: func.var_types.len(),
            entries: func.var_rc_strategies.len(),
        }]
    }
}

fn variable_id(index: usize) -> crate::ir::ArcVarId {
    crate::ir::ArcVarId::new(
        u32::try_from(index).unwrap_or_else(|_| panic!("variable index exceeds u32::MAX")),
    )
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
