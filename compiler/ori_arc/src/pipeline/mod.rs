//! ARC pipeline entry points.
//!
//! Contains [`run_arc_pipeline`] (single function), [`run_arc_pipeline_all`]
//! (batch with borrow application), and [`run_uniqueness_analysis`]
//! (interprocedural uniqueness).
//!
//! When the `aims` feature is active, these functions internally dispatch to
//! the unified AIMS pipeline (interprocedural contracts → per-variable state
//! map → RC/reuse emission) instead of the sequential analysis passes.

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::AnnotatedSig;
#[cfg(any(not(feature = "aims"), feature = "aims-shadow"))]
use crate::uniqueness::Uniqueness;
use crate::uniqueness::UniquenessSummary;
use crate::ArcClassification;

/// Run the full ARC optimization pipeline on a single function.
///
/// Pipeline order: var reprs → derived ownership → liveness →
/// **uniqueness + COW annotation** → RC insertion → reset/reuse →
/// expansion → RC identity → RC elimination → tail call detection +
/// loop lowering → block merge → drop hints → FBIP enforcement.
///
/// When the `aims` feature is active, dispatches to the unified AIMS pipeline
/// which replaces the analysis and RC emission steps with a single state-map
/// based approach (see [`aims_pipeline::run_aims_pipeline`]).
///
/// **Prerequisite:** [`annotate_arg_ownership`](crate::rc_insert::annotate_arg_ownership)
/// must be called before this function. It populates per-argument ownership on
/// `Apply`/`Invoke` instructions so the RC insertion pass can read ownership from the IR.
///
/// The `uniqueness_summaries` parameter provides interprocedural uniqueness
/// information computed by [`run_uniqueness_analysis`]. When non-empty,
/// the pipeline annotates each COW operation with a [`CowMode`](crate::CowMode) on the
/// function's [`cow_annotations`](ArcFunction::cow_annotations) field.
///
/// This is the canonical pass ordering. All consumers should call this function
/// instead of manually sequencing passes, which avoids duplicating ordering
/// knowledge across crate boundaries.
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn run_arc_pipeline(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    uniqueness_summaries: &FxHashMap<Name, UniquenessSummary>,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    // Pure AIMS mode (without shadow comparison).
    #[cfg(all(feature = "aims", not(feature = "aims-shadow")))]
    {
        // AIMS pipeline — `sigs` and `uniqueness_summaries` are unused when
        // AIMS is active (contracts are passed via AimsPipelineConfig).
        // However, during the transitional period the caller still provides
        // them. We silence the warnings and delegate to the AIMS per-function
        // pipeline with empty contracts (the batch path fills them properly).
        let _ = (sigs, uniqueness_summaries);
        let contracts = FxHashMap::default();
        let builtins = BuiltinOwnershipSets::new(interner);
        let config = aims_pipeline::AimsPipelineConfig {
            classifier,
            contracts: &contracts,
            pool,
            interner,
            builtins: &builtins,
            verify_arc,
        };
        aims_pipeline::run_aims_pipeline(func, &config)
    }

    // Legacy mode, or shadow mode (shadow comparison runs in the batch
    // path; per-function path uses legacy for consistent output).
    #[cfg(any(not(feature = "aims"), feature = "aims-shadow"))]
    {
        run_legacy_pipeline(
            func,
            classifier,
            sigs,
            pool,
            interner,
            uniqueness_summaries,
            verify_arc,
        )
    }
}

/// Run the full ARC pipeline on all functions, including borrow application.
///
/// This is the batch entry point for the entire ARC optimization pass:
/// 1. Apply borrow inference results to function parameters
/// 2. Run interprocedural uniqueness analysis (COW check elimination)
/// 3. Annotate per-argument ownership on call instructions
/// 4. Run the per-function pipeline on each function (with uniqueness)
///
/// When the `aims` feature is active, uses AIMS interprocedural analysis to
/// compute `MemoryContract`s, then applies ownership and runs the AIMS
/// per-function pipeline for each function.
///
/// Consumers should call this instead of manually calling [`apply_borrows`](crate::borrow::apply_borrows)
/// followed by a per-function loop over [`run_arc_pipeline`].
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn run_arc_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    builtins: &BuiltinOwnershipSets,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    // Shadow mode: run AIMS analysis alongside legacy pipeline for comparison.
    #[cfg(feature = "aims-shadow")]
    {
        shadow::run_shadow_pipeline_all(
            functions, classifier, sigs, interner, pool, builtins, verify_arc,
        )
    }

    // Pure AIMS mode (without shadow comparison).
    #[cfg(all(feature = "aims", not(feature = "aims-shadow")))]
    {
        let _ = sigs;
        aims_pipeline::run_aims_pipeline_all(
            functions, classifier, interner, pool, builtins, verify_arc,
        )
    }

    // Legacy mode (no AIMS features active).
    #[cfg(not(feature = "aims"))]
    {
        run_legacy_pipeline_all(
            functions, classifier, sigs, interner, pool, builtins, verify_arc,
        )
    }
}

/// Run interprocedural uniqueness analysis on all functions.
///
/// Computes a [`UniquenessSummary`] for each function by:
/// 1. Building hardcoded summaries for COW builtins (push → `Unique`, etc.)
/// 2. Running SCC-based fixpoint analysis across all user functions
///
/// When the `aims` feature is active, this still computes old-pipeline
/// summaries for callers that need them (e.g., `ori_llvm` `FunctionCompiler`
/// which passes summaries to `run_arc_pipeline`). The AIMS pipeline internally
/// computes its own contracts and does not consume these summaries.
///
/// The returned summaries should be passed to [`run_arc_pipeline`] so each
/// function's COW operations are annotated with the correct [`CowMode`](crate::CowMode).
pub fn run_uniqueness_analysis(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    interner: &ori_ir::StringInterner,
) -> FxHashMap<Name, UniquenessSummary> {
    // When AIMS is fully active, interprocedural uniqueness is computed
    // internally by aims::analyze_program(). Callers still call this function
    // but the result is discarded by run_arc_pipeline. Return an empty map
    // to avoid running the legacy analysis.
    #[cfg(all(feature = "aims", not(feature = "aims-shadow")))]
    {
        let _ = (functions, classifier, interner);
        FxHashMap::default()
    }

    #[cfg(any(not(feature = "aims"), feature = "aims-shadow"))]
    {
        let cow_names = crate::borrow::all_cow_method_names(interner);
        let sharing_names = crate::borrow::sharing_builtin_names(interner);
        let builtin_summaries =
            crate::uniqueness::inter::build_cow_summaries(&cow_names, &sharing_names);

        tracing::debug!(
            function_count = functions.len(),
            cow_builtins = cow_names.len(),
            sharing_builtins = sharing_names.len(),
            "starting interprocedural uniqueness analysis"
        );

        let summaries =
            crate::uniqueness::inter::analyze_program(functions, classifier, &builtin_summaries);

        if tracing::enabled!(tracing::Level::DEBUG) {
            let unique_returns = summaries
                .values()
                .filter(|s| s.return_val == Uniqueness::Unique)
                .count();
            tracing::debug!(
                total_summaries = summaries.len(),
                unique_returns,
                "interprocedural uniqueness analysis complete"
            );
        }

        summaries
    }
}

// Legacy pipeline (active when `aims` feature is NOT enabled, or when
// `aims-shadow` is active — shadow uses legacy output while comparing)

#[cfg(any(not(feature = "aims"), feature = "aims-shadow"))]
fn run_legacy_pipeline(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    uniqueness_summaries: &FxHashMap<Name, UniquenessSummary>,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    func.var_reprs = crate::ir::compute_var_reprs(func, classifier, pool);

    let ownership = crate::borrow::infer_derived_ownership(func, sigs);
    let (_, liveness) = crate::liveness::compute_refined_liveness(func, classifier);

    if !uniqueness_summaries.is_empty() {
        let cow_names = crate::borrow::all_cow_method_names(interner);
        func.cow_annotations = crate::uniqueness::compute_cow_annotations(
            func,
            classifier,
            &liveness,
            uniqueness_summaries,
            &cow_names,
        );
    }

    crate::rc_insert::insert_rc_ops_with_ownership(
        func, classifier, &liveness, &ownership, sigs, pool,
    );
    crate::rc_insert::insert_external_invoke_cleanup(func, classifier, &liveness, pool);

    run_verify(func, "after RC insertion", verify_arc);

    let dom_tree = crate::graph::DominatorTree::build(func);
    let post_dom_tree = crate::graph::PostDominatorTree::build(func);
    let (refined_post_rc, _) = crate::liveness::compute_refined_liveness(func, classifier);
    crate::reset_reuse::detect_reset_reuse_cfg(
        func,
        classifier,
        &dom_tree,
        &post_dom_tree,
        &refined_post_rc,
        pool,
    );
    crate::expand_reuse::expand_reset_reuse(func, classifier, Some(pool));

    let identity_map = crate::rc_identity::RcIdentityMap::build(func, &ownership);
    crate::rc_identity::propagate_rc_identity(func, &identity_map, pool);

    crate::rc_elim::eliminate_rc_ops_dataflow(func, &ownership);

    run_verify(func, "after RC elimination", verify_arc);

    func.tail_calls = crate::tail_call::detect_tail_calls(func);
    crate::tail_call::rewrite_tail_calls(func);

    crate::block_merge::merge_blocks(func);

    func.drop_hints = crate::uniqueness::compute_drop_hints(func, pool);

    let mut problems = Vec::new();
    if func.is_fbip {
        let func_name = interner.lookup(func.name);
        let func_span = func
            .spans
            .first()
            .and_then(|block_spans| block_spans.first().copied().flatten())
            .unwrap_or(ori_ir::Span::DUMMY);
        if let Some(problem) =
            crate::fbip::check_fbip_enforcement(func, classifier, func_name, func_span)
        {
            problems.push(problem);
        }
    }

    if crate::fbip::is_auto_fbip(func) {
        let func_name = interner.lookup(func.name);
        tracing::debug!(
            function = func_name,
            cow_ops = func.cow_annotations.len(),
            "auto FBIP: all COW operations are StaticUnique"
        );
    }
    problems
}

// Legacy batch pipeline — only used in non-AIMS dispatch (shadow
// inlines these steps to capture intermediates for comparison).
#[cfg(not(feature = "aims"))]
fn run_legacy_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    builtins: &BuiltinOwnershipSets,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    crate::borrow::apply_borrows(functions, sigs);

    let uniqueness_summaries = run_uniqueness_analysis(functions, classifier, interner);

    let mut all_problems = Vec::new();
    for func in functions {
        crate::rc_insert::annotate_arg_ownership(func, sigs, interner, builtins, pool);
        let problems = run_arc_pipeline(
            func,
            classifier,
            sigs,
            pool,
            interner,
            &uniqueness_summaries,
            verify_arc,
        );
        all_problems.extend(problems);
    }
    all_problems
}

// AIMS pipeline (active when `aims` feature IS enabled)

// AIMS pipeline (active when `aims` feature IS enabled — both pure and shadow)
#[cfg(feature = "aims")]
mod aims_pipeline;

// RC operation counting for pipeline comparison
#[cfg(feature = "aims")]
pub(crate) mod rc_count;

// Shadow comparison (active when `aims-shadow` feature IS enabled)
#[cfg(feature = "aims-shadow")]
mod shadow;

/// Run ARC IR verification if enabled.
///
/// Active under `debug_assertions` or when the caller passes `verify: true`
/// (typically from `ORI_VERIFY_ARC=1` read in `oric`).
/// Logs warnings for each error but does not panic — this is diagnostic,
/// not blocking.
fn run_verify(func: &ArcFunction, phase: &str, verify: bool) {
    let enabled = verify || cfg!(debug_assertions);
    if !enabled {
        return;
    }

    let errors = crate::verify::check_function(func);
    for e in &errors {
        tracing::warn!(phase, "ARC IR verification: {e}");
    }
}
