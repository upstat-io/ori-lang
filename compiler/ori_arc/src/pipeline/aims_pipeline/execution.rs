//! Per-function AIMS realization.
//!
//! Interprocedural analysis freezes contracts before this module consumes them.
//! Each function normalizes IR, solves the lattice, materializes logical
//! ownership events and reuse, verifies them, rewrites control flow, and derives
//! post-merge COW/drop annotations. Physical projections consume the frozen
//! logical events through their own representation plans.

use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{ContractMapExt, MemoryContract};
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr};
use crate::lower::ArcProblem;
use crate::pipeline::rc_count;
use crate::ArcClassification;

use super::metadata::validate_metadata_checkpoint;
use super::{burden_emission, postprocess, trmc};

/// Receives stable function snapshots at realization checkpoints.
pub type CheckpointObserver<'a> = dyn Fn(&ArcFunction, &str /* phase */) + 'a;

/// Shared immutable inputs for one per-function realization.
pub(crate) struct AimsPipelineConfig<'a> {
    /// ARC type classifier used by the analysis passes.
    pub classifier: &'a dyn ArcClassification,
    /// Closed memory contracts keyed by callable name.
    pub contracts: &'a FxHashMap<Name, MemoryContract>,
    /// Exact local functions whose contracts must be present.
    pub func_names: &'a FxHashSet<Name>,
    /// Local and producer-validated external callables whose exact contracts
    /// take precedence over same-spelled builtin ownership heuristics.
    pub exact_callables: &'a FxHashSet<Name>,
    /// Canonical type pool for structural queries.
    pub pool: &'a ori_types::Pool,
    /// Interner for diagnostics, tracing, and callable identity.
    pub interner: &'a ori_ir::StringInterner,
    /// Preclassified builtin ownership semantics.
    pub builtins: &'a BuiltinOwnershipSets,
    /// Receives each stable checkpoint when snapshot capture is enabled.
    pub observer: Option<&'a CheckpointObserver<'a>>,
    /// Type information required to freeze class-ledger plans.
    pub type_registry: &'a TypeRegistry,
    /// Whether to run the optional ARC verification checkpoints.
    pub verify_arc: bool,
}

/// Result of `run_aims_pipeline` for a single function.
pub(crate) struct AimsPipelineResult {
    /// Verification problems reported during realization.
    pub problems: Vec<ArcProblem>,
    /// Post-emission missed reuse count from `FipEvidence` for refreshed
    /// `may_deallocate` and `Bounded(n)` contract verification.
    pub missed_reuses: usize,
    /// Whether TRMC rewriting survived structural and semantic verification.
    pub was_trmc_rewritten: bool,
}

/// Emits structural and ownership metrics and invokes `observer`.
///
/// Disabled tracing avoids metric and name computation.
pub(crate) fn trace_pipeline_checkpoint(
    func: &ArcFunction,
    phase: &str,
    interner: &ori_ir::StringInterner,
    observer: Option<&CheckpointObserver<'_>>,
) {
    // Why: Disabled tracing must not pay for count and name computation.
    if tracing::enabled!(tracing::Level::INFO) {
        let fn_name = interner.lookup(func.name);
        let rc = rc_count::count_rc_ops(func);
        let blocks = func.blocks.len();
        let vars = func.var_types.len();
        // INVARIANT: Burden sites retain coordinates for pass bisection.
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
    if let Some(obs) = observer {
        obs(func, phase);
    }
}

/// Realizes one function after closed-program contracts are frozen.
///
/// Returns verification failures only when explicit ARC verification is enabled;
/// such failures represent compiler invariants, not user diagnostics.
pub(crate) fn run_aims_pipeline(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<AimsPipelineResult, Vec<crate::verify::VerifyError>> {
    // INVARIANT: Primitive facts are resolved once and otherwise validated unchanged.
    crate::aims::primitive::ensure_primitive_facts(func, config.classifier)?;

    let (norm_result, immortals, did_trmc_transform, pre_trmc_func) =
        trmc::normalize_with_trmc(func, config)?;
    trace_pipeline_checkpoint(
        func,
        "normalize_with_trmc_complete",
        config.interner,
        config.observer,
    );
    // INVARIANT: Structural rewrites preserve exact primitive-destination coverage.
    crate::aims::primitive::ensure_primitive_facts(func, config.classifier)?;

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

    let (mut state_map, trmc_rewrite_survived) =
        trmc::verify_trmc_soundness(func, state_map, did_trmc_transform, pre_trmc_func, config);
    trace_pipeline_checkpoint(
        func,
        "verify_trmc_soundness",
        config.interner,
        config.observer,
    );

    freeze_yield_allocation_locality(func, &state_map);

    // INVARIANT: Converged state fixes the birth-site partition before burden emission.
    let birth_site_partition =
        crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition(
            func, &state_map,
        );
    state_map.set_birth_site_partition(birth_site_partition);

    // INVARIANT: Payload analysis reads unannotated IR; class planning reads converged ownership.
    {
        let _span = tracing::debug_span!("emit_arg_ownership_prelude").entered();
        crate::aims::emit_rc::arg_ownership::emit_arg_ownership(
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

    // INVARIANT: Class-ledger planning is the sole producer of ownership events.
    apply_class_ledger(func, &state_map, config);
    crate::aims::realize::emit_survivor_remarks_all_kept(func, &state_map, config.interner);
    trace_pipeline_checkpoint(
        func,
        "class_ledger_emission",
        config.interner,
        config.observer,
    );

    burden_emission::dump_after_burden(func, config);
    burden_emission::dump_after_class_ledger_emission_compat(func, config);

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

    let missed_reuses = result.fip_evidence.missed_reuses;

    fip_precheck(func, config, &result)?;
    trace_pipeline_checkpoint(
        func,
        "verify_fip_contract",
        config.interner,
        config.observer,
    );

    result.synergy_metrics.canonicalize_cross_fires = state_map.count_cross_dim_states();

    postprocess::verify_and_merge(func, config)?;

    // INVARIANT: Rewrites preserve authoritative variable metadata through handoff.
    validate_metadata_checkpoint(func, config)?;

    install_post_merge_annotations(func, &state_map, config, &mut result);

    let problems = finish_postprocess(func, config)?;

    Ok(AimsPipelineResult {
        problems,
        missed_reuses,
        was_trmc_rewritten: trmc_rewrite_survived,
    })
}

fn apply_class_ledger(
    func: &mut ArcFunction,
    state_map: &crate::aims::intraprocedural::AimsStateMap,
    config: &AimsPipelineConfig<'_>,
) {
    crate::aims::class_ledger::apply_class_ledger_replacement_with_exact(
        func,
        state_map,
        config.contracts,
        config.exact_callables,
        config.type_registry,
        config.interner,
        burden_emission::burden_ops_enabled(),
    );
}

fn finish_postprocess(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<Vec<ArcProblem>, Vec<crate::verify::VerifyError>> {
    postprocess::emit_postprocess(func, config)
}

/// Freeze AIMS placement eligibility onto stable yield-allocation identities.
fn freeze_yield_allocation_locality(
    func: &mut ArcFunction,
    state_map: &crate::aims::intraprocedural::AimsStateMap,
) {
    let yield_lineages = crate::YieldLineageIndex::for_function(func);
    let returned_yield_results: FxHashSet<_> = func
        .blocks
        .iter()
        .filter_map(|block| match block.terminator {
            crate::ir::ArcTerminator::Return { value } => Some(value),
            _ => None,
        })
        .filter_map(|returned| yield_lineages.result_for_receiver(returned))
        .collect();
    let mut eligible = FxHashSet::default();
    for block in &func.blocks {
        for event in state_map.events_in_block(block.id) {
            if let crate::aims::intraprocedural::AimsEvent::PlacementEligibilityCandidate {
                var,
                ..
            } = event
            {
                eligible.insert(*var);
            }
        }
    }
    for fact in &mut func.yield_allocations {
        fact.locality =
            if eligible.contains(&fact.result) && !returned_yield_results.contains(&fact.result) {
                crate::ir::YieldAllocationLocality::Local
            } else {
                crate::ir::YieldAllocationLocality::Escaping
            };
    }
}

/// Installs post-merge COW and drop annotations on `func`.
fn install_post_merge_annotations(
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

/// Rejects structural FIP violations before the contract refresh.
///
/// Missed reuses may change `may_deallocate` and are rechecked after refresh;
/// unbounded stack or exceeded bounds are already final here.
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
                // Why: Missed reuses feed the contract refresh before final verification.
                tracing::debug!("FIP pre-check (will recompute in second pass): {e}");
            }
            FipVerificationError::CertifiedButUnboundedStack { .. }
            | FipVerificationError::BoundedExceeded { .. } => {
                // INVARIANT: Stack and bound facts are final before emission.
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
