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
//! 4. `aims::analyze_function()` — backward dataflow → converged state map
//! 5. `aims::realize_rc_reuse()` — Phase 1: `arg_ownership` + RC + reuse (pre-merge)
//! 6. `verify()` — ARC IR sanity check
//! 7. `run_aims_verify()` — AIMS contract vs IR consistency
//! 8. `detect_tail_calls()` + `rewrite_tail_calls()`
//! 9. `merge_blocks()` — CFG cleanup
//! 10. `aims::realize_annotations()` — Phase 2: COW + drop hints (post-merge)
//! 11. `verify()` — final sanity check
//! 12. FBIP enforcement — read-only diagnostic

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::{MemoryContract, ParamContract};
use crate::aims::lattice::AccessClass;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::Ownership;
use crate::ArcClassification;

/// Configuration for the AIMS per-function pipeline.
///
/// Bundles the shared parameters that `run_aims_pipeline` needs, avoiding
/// the 7-parameter signature anti-pattern from the old pipeline.
pub(crate) struct AimsPipelineConfig<'a> {
    pub classifier: &'a dyn ArcClassification,
    pub contracts: &'a FxHashMap<Name, MemoryContract>,
    pub pool: &'a Pool,
    pub interner: &'a ori_ir::StringInterner,
    pub builtins: &'a BuiltinOwnershipSets,
    pub verify_arc: bool,
}

/// Run the AIMS pipeline on a single function (steps 3–12).
///
/// Called from within `run_arc_pipeline` when the `aims` feature is active.
/// Interprocedural contracts must already be computed and passed via `config`.
pub(crate) fn run_aims_pipeline(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Vec<ArcProblem> {
    // Step 3: compute value representations.
    {
        let _span = tracing::info_span!("compute_var_reprs").entered();
        func.var_reprs = crate::ir::compute_var_reprs(func, config.classifier, config.pool);
    }

    // Step 3.5: detect immortal variables.
    let immortals = detect_immortals(func, config);

    // Intraprocedural analysis → converged state map.
    let state_map = {
        let _span = tracing::info_span!("analyze_function").entered();
        crate::aims::intraprocedural::analyze_function(
            func,
            config.classifier,
            config.contracts,
            &[], // context_regions: empty in Stage 1
            immortals,
        )
    };

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

    // Set canonicalize cross-dim fires from converged state analysis.
    result.synergy_metrics.canonicalize_cross_fires = state_map.count_cross_dim_states();

    // Verify, AIMS-verify, tail calls, merge.
    verify_and_merge(func, config);

    // Phase 2: COW + drop hints (post-merge).
    {
        let _span = tracing::info_span!("realize_annotations").entered();
        crate::aims::realize::realize_annotations(
            func,
            &state_map,
            config.interner,
            config.pool,
            &mut result,
        );
    }
    func.cow_annotations = result.cow_annotations;
    func.drop_hints = result.drop_hints;

    // Final verification + FBIP.
    emit_postprocess(func, config)
}

/// Detect immortal variables (heap-allocated constants with `MAX_REFCOUNT`).
fn detect_immortals(func: &ArcFunction, config: &AimsPipelineConfig<'_>) -> Vec<bool> {
    let _span = tracing::info_span!("detect_immortals").entered();
    let imm = crate::aims::immortal::detect_immortals(func, config.interner);
    let immortal_count = crate::aims::immortal::count_immortals(&imm);
    if immortal_count > 0 {
        tracing::debug!(
            function = func.name.raw(),
            immortal_count,
            "immortal variables detected"
        );
    }
    imm
}

/// Post-emission steps: final verify + FBIP.
fn emit_postprocess(func: &mut ArcFunction, config: &AimsPipelineConfig<'_>) -> Vec<ArcProblem> {
    {
        let _span = tracing::info_span!("verify_final").entered();
        super::run_verify(func, "after AIMS pipeline", config.verify_arc);
    }

    check_fbip(func, config)
}

/// Verify, AIMS-verify, detect tail calls, merge blocks.
fn verify_and_merge(func: &mut ArcFunction, config: &AimsPipelineConfig<'_>) {
    {
        let _span = tracing::info_span!("verify_post_emission").entered();
        super::run_verify(func, "after AIMS emission", config.verify_arc);
    }
    if let Some(contract) = config.contracts.get(&func.name) {
        let _span = tracing::info_span!("aims_verify").entered();
        super::run_aims_verify(func, contract, "after AIMS emission", config.verify_arc);
    }
    {
        let _span = tracing::info_span!("tail_calls").entered();
        func.tail_calls = crate::tail_call::detect_tail_calls(func);
        crate::tail_call::rewrite_tail_calls(func);
    }
    {
        let _span = tracing::info_span!("merge_blocks").entered();
        crate::block_merge::merge_blocks(func);
    }
}

/// Check FBIP enforcement and auto-FBIP detection (Step 14).
fn check_fbip(func: &ArcFunction, config: &AimsPipelineConfig<'_>) -> Vec<ArcProblem> {
    let mut problems = Vec::new();
    if func.is_fbip {
        let func_name = config.interner.lookup(func.name);
        let func_span = func
            .spans
            .first()
            .and_then(|block_spans| block_spans.first().copied().flatten())
            .unwrap_or(ori_ir::Span::DUMMY);
        if let Some(problem) =
            crate::fbip::check_fbip_enforcement(func, config.classifier, func_name, func_span)
        {
            problems.push(problem);
        }
    }

    if crate::fbip::is_auto_fbip(func) {
        let func_name = config.interner.lookup(func.name);
        tracing::debug!(
            function = func_name,
            cow_ops = func.cow_annotations.len(),
            "auto FBIP: all COW operations are StaticUnique"
        );
    }

    problems
}

/// Run the AIMS pipeline on all functions (batch entry point).
///
/// Called from within `run_arc_pipeline_all` when the `aims` feature is active.
///
/// 1. Compute interprocedural contracts via `aims::analyze_program()`
/// 2. Apply ownership to function parameters
/// 3. Run per-function pipeline for each function
#[cfg_attr(
    feature = "aims-shadow",
    expect(dead_code, reason = "used in pure AIMS mode, not shadow mode")
)]
pub(crate) fn run_aims_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    builtins: &BuiltinOwnershipSets,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    // Step 1: interprocedural analysis → MemoryContract per function.
    let contracts = {
        let _span = tracing::info_span!("analyze_program").entered();
        crate::aims::interprocedural::analyze_program(functions, classifier, builtins, interner)
    };

    // Step 2: apply ownership to function parameters.
    {
        let _span = tracing::info_span!("apply_ownership").entered();
        apply_aims_ownership(functions, &contracts);
    }

    // Steps 3–14: per-function pipeline.
    let config = AimsPipelineConfig {
        classifier,
        contracts: &contracts,
        pool,
        interner,
        builtins,
        verify_arc,
    };

    let mut all_problems = Vec::new();
    let mut total_rc = super::rc_count::RcOpCount::default();
    for func in functions.iter_mut() {
        let problems = run_aims_pipeline(func, &config);
        all_problems.extend(problems);
        let rc = super::rc_count::count_rc_ops(func);
        total_rc.inc += rc.inc;
        total_rc.dec += rc.dec;
    }

    tracing::debug!(
        functions = functions.len(),
        rc_inc = total_rc.inc,
        rc_dec = total_rc.dec,
        rc_total = total_rc.total(),
        "AIMS pipeline RC operation totals"
    );

    all_problems
}

/// Apply AIMS ownership annotations to function parameters.
///
/// Sets `ArcParam.ownership` on each function from its `MemoryContract`.
/// Replaces `borrow::apply_borrows()` in the old pipeline.
pub(crate) fn apply_aims_ownership(
    functions: &mut [ArcFunction],
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    for func in functions {
        let Some(contract) = contracts.get(&func.name) else {
            continue;
        };
        for (param, pc) in func.params.iter_mut().zip(&contract.params) {
            param.ownership = param_contract_to_ownership(*pc);
        }
    }
}

/// Convert a `ParamContract` access class to the `Ownership` enum used by
/// `ArcParam`. This bridges the AIMS contract representation with the
/// existing ARC IR parameter ownership field.
fn param_contract_to_ownership(pc: ParamContract) -> Ownership {
    match pc.access {
        AccessClass::Borrowed => Ownership::Borrowed,
        AccessClass::Owned => Ownership::Owned,
    }
}
