//! ARC pipeline entry points.
//!
//! Contains [`run_arc_pipeline`] (single function), [`run_arc_pipeline_all`]
//! (batch with borrow application), and [`run_uniqueness_analysis`]
//! (interprocedural uniqueness).

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::AnnotatedSig;
use crate::uniqueness::{Uniqueness, UniquenessSummary};
use crate::ArcClassification;

/// Run the full ARC optimization pipeline on a single function.
///
/// Pipeline order: var reprs → derived ownership → liveness →
/// **uniqueness + COW annotation** → RC insertion → reset/reuse →
/// expansion → RC identity → RC elimination → tail call detection +
/// loop lowering → block merge → drop hints → FBIP enforcement.
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
) -> Vec<ArcProblem> {
    // Compute value representations before any passes modify the function.
    func.var_reprs = crate::ir::compute_var_reprs(func, classifier, pool);

    let ownership = crate::borrow::infer_derived_ownership(func, sigs);
    let (_, liveness) = crate::liveness::compute_refined_liveness(func, classifier);

    // Uniqueness analysis: determine CowMode for each COW operation.
    // Runs BEFORE RC insertion because the analysis needs the pre-RC form
    // (RC ops are handled defensively but add noise). Uses the liveness data
    // already computed above.
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

    // ARC IR verification after RC insertion.
    run_verify(func, "after RC insertion");

    // Build dom/post-dom trees AFTER RC insertion. Edge cleanup can split
    // edges and append trampoline blocks, which invalidates any earlier
    // dominator analysis. Refined liveness is also recomputed so cross-block
    // detection sees the post-insertion CFG.
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

    // Normalize RC identities before elimination: rewrite RcInc/RcDec on
    // projected variables to target their canonical root, enabling the
    // pair-matching eliminator to find more Inc/Dec cancellations.
    let identity_map = crate::rc_identity::RcIdentityMap::build(func, &ownership);
    crate::rc_identity::propagate_rc_identity(func, &identity_map, pool);

    crate::rc_elim::eliminate_rc_ops_dataflow(func, &ownership);

    // ARC IR verification after RC elimination.
    run_verify(func, "after RC elimination");

    // Tail call detection + loop lowering: identify self-recursive tail calls
    // and rewrite them as loop back-edges. Runs AFTER RC elimination (all RC
    // ops are in final positions — we can verify RcDec hoisting safety) and
    // BEFORE block merge (which cleans up dead merge blocks left by the rewrite
    // and renumbers blocks).
    func.tail_calls = crate::tail_call::detect_tail_calls(func);
    crate::tail_call::rewrite_tail_calls(func);

    // Block merge: eliminate redundant blocks created by invoke splitting.
    // Runs AFTER RC elimination (all RC ops are final) but BEFORE drop hints
    // (which store block_idx/instr_idx coordinates that merge invalidates).
    crate::block_merge::merge_blocks(func);

    // Drop hints: identify RcDec instructions on provably unique collections.
    // Runs AFTER block merge (indices are final). The LLVM emitter uses
    // these hints to call ori_buffer_drop_unique instead of ori_buffer_rc_dec.
    func.drop_hints = crate::uniqueness::compute_drop_hints(func, pool);

    // FBIP enforcement: check #fbip-annotated functions for missed reuse.
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

    // Auto FBIP detection: functions with all COW operations proven
    // StaticUnique achieve FBIP without the `#fbip` attribute.
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

/// Run the full ARC pipeline on all functions, including borrow application.
///
/// This is the batch entry point for the entire ARC optimization pass:
/// 1. Apply borrow inference results to function parameters
/// 2. Run interprocedural uniqueness analysis (COW check elimination)
/// 3. Annotate per-argument ownership on call instructions
/// 4. Run the per-function pipeline on each function (with uniqueness)
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
) -> Vec<ArcProblem> {
    crate::borrow::apply_borrows(functions, sigs);

    // Interprocedural uniqueness analysis: compute per-function summaries
    // AFTER borrow application (uses ownership annotations) but BEFORE
    // per-function RC insertion (which modifies the IR).
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
        );
        all_problems.extend(problems);
    }
    all_problems
}

/// Run interprocedural uniqueness analysis on all functions.
///
/// Computes a [`UniquenessSummary`] for each function by:
/// 1. Building hardcoded summaries for COW builtins (push → `Unique`, etc.)
/// 2. Running SCC-based fixpoint analysis across all user functions
///
/// The returned summaries should be passed to [`run_arc_pipeline`] so each
/// function's COW operations are annotated with the correct [`CowMode`](crate::CowMode).
///
/// This is the interprocedural counterpart to the per-function uniqueness
/// analysis in [`run_arc_pipeline`]. It runs once across all functions to
/// determine which function return values are provably unique.
pub fn run_uniqueness_analysis(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    interner: &ori_ir::StringInterner,
) -> FxHashMap<Name, UniquenessSummary> {
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

/// Run ARC IR verification if enabled.
///
/// Active under `debug_assertions` or when `ORI_VERIFY_ARC=1` is set.
/// Logs warnings for each error but does not panic — this is diagnostic,
/// not blocking.
fn run_verify(func: &ArcFunction, phase: &str) {
    let enabled = cfg!(debug_assertions) || std::env::var("ORI_VERIFY_ARC").is_ok();
    if !enabled {
        return;
    }

    let errors = crate::verify::check_function(func);
    for e in &errors {
        tracing::warn!(phase, "ARC IR verification: {e}");
    }
}
