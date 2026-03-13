//! Shadow comparison between legacy and AIMS pipelines.
//!
//! Runs AIMS analysis (read-only) alongside the legacy pipeline to validate
//! that AIMS produces equivalent or superior analysis results. The actual
//! output comes from the legacy pipeline — AIMS analysis is for comparison
//! only.
//!
//! # Comparison dimensions
//!
//! 1. **Param ownership**: AIMS `ParamContract.access` vs legacy `ArcParam.ownership`
//! 2. **Return uniqueness**: AIMS `ReturnContract.uniqueness` vs legacy `UniquenessSummary.return_val`
//! 3. **COW annotations**: AIMS-predicted `CowMode` counts vs legacy `CowAnnotations`
//! 4. **RC operations**: AIMS `RcInc`/`RcDec` count vs legacy (fewer = better)
//! 5. **Arg ownership**: per-call-site `Apply.arg_ownership` / `Invoke.arg_ownership`
//!
//! # Gate criterion
//!
//! Zero regressions (AIMS weaker than legacy). Improvements are expected and logged.

mod compare;

use ori_ir::{Name, StringInterner};
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::AnnotatedSig;
use crate::uniqueness::CowAnnotations;
use crate::ArcClassification;

use super::aims_pipeline::AimsPipelineConfig;
use super::rc_count::{count_rc_ops, RcOpCount};

// Comparison types

/// Per-dimension comparison result.
#[derive(Debug)]
pub(crate) enum DimensionResult {
    /// AIMS and legacy agree.
    Match,
    /// AIMS is strictly tighter (better optimization).
    Improvement(String),
    /// AIMS is strictly weaker (worse optimization).
    Regression(String),
    /// Comparison not applicable (missing data, param count mismatch, etc.).
    Skipped(#[allow(dead_code, reason = "diagnostic detail for tracing; read in tests")] String),
}

/// Per-function comparison of AIMS vs legacy analysis.
#[derive(Debug)]
pub(crate) struct FunctionComparison {
    #[expect(
        dead_code,
        reason = "reserved for programmatic test access via ShadowComparisonReport"
    )]
    pub name: Name,
    pub name_str: String,
    pub param_ownership: DimensionResult,
    pub return_uniqueness: DimensionResult,
    pub cow_annotations: DimensionResult,
    pub rc_ops: DimensionResult,
    /// Per-call-site `arg_ownership` comparison (Apply/Invoke instructions).
    pub arg_ownership: DimensionResult,
    /// Number of RC operations skipped due to immortal variables in the AIMS
    /// pipeline. When > 0, RC count improvements are partially or fully
    /// attributable to immortal object optimization.
    pub immortal_skips: usize,
}

/// Aggregate shadow comparison report.
#[derive(Debug)]
pub(crate) struct ShadowComparisonReport {
    pub per_function: Vec<FunctionComparison>,
    pub total_functions: usize,
    pub param_matches: usize,
    pub param_improvements: usize,
    pub param_regressions: usize,
    pub return_matches: usize,
    pub return_improvements: usize,
    pub return_regressions: usize,
    pub cow_matches: usize,
    pub cow_improvements: usize,
    pub cow_regressions: usize,
    pub rc_matches: usize,
    pub rc_improvements: usize,
    pub rc_regressions: usize,
    pub arg_ownership_matches: usize,
    pub arg_ownership_improvements: usize,
    pub arg_ownership_regressions: usize,
    /// Total AIMS RC operations across all functions.
    pub aims_rc_total: usize,
    /// Total legacy RC operations across all functions.
    pub legacy_rc_total: usize,
    /// Total immortal variable count across all functions. When > 0, some RC
    /// count improvements are attributable to immortal object optimization.
    pub immortal_skips_total: usize,
}

impl ShadowComparisonReport {
    fn has_regressions(&self) -> bool {
        self.param_regressions > 0
            || self.return_regressions > 0
            || self.cow_regressions > 0
            || self.arg_ownership_regressions > 0
    }
}

/// AIMS analysis snapshot for a single function (computed pre-mutation).
struct AimsSnapshot {
    contract: Option<MemoryContract>,
    cow_annotations: CowAnnotations,
    rc_ops: RcOpCount,
    /// Per-call-site `arg_ownership` vectors extracted from the AIMS-processed
    /// clone. Each entry is `(call_target, arg_ownership)` where `call_target`
    /// is the function name for Apply/Invoke. Collected after AIMS pipeline
    /// populates `arg_ownership` but before block-altering passes.
    arg_ownership_sites: Vec<(Name, Vec<crate::ir::ArgOwnership>)>,
    /// Number of immortal variables detected (heap-allocated constants that
    /// skip all RC operations). Used to attribute RC count improvements in the
    /// shadow comparison — when AIMS has fewer RC ops than legacy, immortal
    /// skips explain part of the delta.
    immortal_count: usize,
}

/// Run the shadow comparison pipeline.
///
/// 1. Run AIMS analysis (read-only) on unmodified functions
/// 2. Run full AIMS pipeline on cloned functions (RC op counting)
/// 3. Run legacy pipeline (mutating) — actual output
/// 4. Compare AIMS predictions and RC counts against legacy results
/// 5. Log via tracing
pub(crate) fn run_shadow_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    interner: &StringInterner,
    pool: &Pool,
    builtins: &BuiltinOwnershipSets,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    tracing::info!(
        "AIMS shadow comparison: starting analysis on {} functions",
        functions.len()
    );

    // Phase 1: AIMS interprocedural analysis (read-only, on unmodified functions).
    let contracts =
        crate::aims::interprocedural::analyze_program(functions, classifier, builtins, interner);

    // Phase 2: Run full AIMS pipeline on cloned functions.
    // The realize pipeline populates COW annotations, RC ops, and arg ownership.
    let mut aims_snapshots: FxHashMap<Name, AimsSnapshot> = FxHashMap::default();
    let mut aims_clones: Vec<ArcFunction> = functions.to_vec();
    super::aims_pipeline::apply_aims_ownership(&mut aims_clones, &contracts);

    // Detect immortals per function before the pipeline mutates the clones.
    // The pipeline internally detects immortals too, but we capture the count
    // here to attribute RC improvements in the comparison.
    let immortal_counts: FxHashMap<Name, usize> = aims_clones
        .iter()
        .map(|f| {
            let immortals = crate::aims::immortal::detect_immortals(f, interner);
            (f.name, crate::aims::immortal::count_immortals(&immortals))
        })
        .collect();

    let aims_config = AimsPipelineConfig {
        classifier,
        contracts: &contracts,
        pool,
        interner,
        builtins,
        verify_arc,
    };
    for clone in &mut aims_clones {
        let _ = super::aims_pipeline::run_aims_pipeline(clone, &aims_config);
    }
    for clone in &aims_clones {
        aims_snapshots.insert(
            clone.name,
            AimsSnapshot {
                contract: contracts.get(&clone.name).cloned(),
                cow_annotations: clone.cow_annotations.clone(),
                rc_ops: count_rc_ops(clone),
                arg_ownership_sites: compare::extract_arg_ownership_sites(clone),
                immortal_count: immortal_counts.get(&clone.name).copied().unwrap_or(0),
            },
        );
    }
    drop(aims_clones);

    // Phase 3: Legacy interprocedural passes (mutating).
    crate::borrow::apply_borrows(functions, sigs);
    let uniqueness_summaries = super::run_uniqueness_analysis(functions, classifier, interner);

    // Phase 4: Legacy per-function pipeline (mutating).
    let mut all_problems = Vec::new();
    for func in functions.iter_mut() {
        crate::rc_insert::annotate_arg_ownership(func, sigs, interner, builtins, pool);
        let problems = super::run_legacy_pipeline(
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

    // Phase 4.5: Count legacy RC ops (after legacy pipeline has mutated functions).
    let legacy_rc_counts: FxHashMap<Name, RcOpCount> = functions
        .iter()
        .map(|f| (f.name, count_rc_ops(f)))
        .collect();

    // Phase 5: Compare and report.
    let report = compare::compare_all(
        functions,
        &aims_snapshots,
        &legacy_rc_counts,
        &uniqueness_summaries,
        interner,
    );
    log_report(&report);

    all_problems
}

// Reporting

fn log_report(report: &ShadowComparisonReport) {
    tracing::info!(
        total = report.total_functions,
        param_match = report.param_matches,
        param_improve = report.param_improvements,
        param_regress = report.param_regressions,
        return_match = report.return_matches,
        return_improve = report.return_improvements,
        return_regress = report.return_regressions,
        cow_match = report.cow_matches,
        cow_improve = report.cow_improvements,
        cow_regress = report.cow_regressions,
        rc_match = report.rc_matches,
        rc_improve = report.rc_improvements,
        rc_regress = report.rc_regressions,
        arg_own_match = report.arg_ownership_matches,
        arg_own_improve = report.arg_ownership_improvements,
        arg_own_regress = report.arg_ownership_regressions,
        aims_rc_total = report.aims_rc_total,
        legacy_rc_total = report.legacy_rc_total,
        immortal_skips = report.immortal_skips_total,
        "AIMS shadow comparison summary"
    );

    // Log per-function details.
    for fc in &report.per_function {
        log_function_details(fc);
    }

    // RC totals summary.
    if report.aims_rc_total != report.legacy_rc_total {
        let saved = report.legacy_rc_total.saturating_sub(report.aims_rc_total);
        let excess = report.aims_rc_total.saturating_sub(report.legacy_rc_total);
        tracing::info!(
            aims = report.aims_rc_total,
            legacy = report.legacy_rc_total,
            saved,
            excess,
            "AIMS RC operation total comparison"
        );
    }

    if report.has_regressions() {
        tracing::warn!(
            param = report.param_regressions,
            return_uniq = report.return_regressions,
            cow = report.cow_regressions,
            arg_own = report.arg_ownership_regressions,
            "AIMS shadow comparison: REGRESSIONS DETECTED — Stage 1A gate FAILED"
        );
    } else {
        tracing::info!("AIMS shadow comparison: no regressions — Stage 1A gate PASSED");
    }

    // RC regressions are tracked but don't fail the Stage 1A gate
    // (Stage 1C accepts correctness-first with RC count regressions investigated).
    if report.rc_regressions > 0 {
        tracing::warn!(
            rc_regressions = report.rc_regressions,
            aims_total = report.aims_rc_total,
            legacy_total = report.legacy_rc_total,
            "AIMS RC count regressions detected — investigate per-function details above"
        );
    }
}

/// Log regressions and improvements for a single function comparison.
fn log_function_details(fc: &FunctionComparison) {
    let dimensions: &[(&str, &DimensionResult)] = &[
        ("param_ownership", &fc.param_ownership),
        ("return_uniqueness", &fc.return_uniqueness),
        ("cow_annotations", &fc.cow_annotations),
        ("rc_ops", &fc.rc_ops),
        ("arg_ownership", &fc.arg_ownership),
    ];

    for &(dim, result) in dimensions {
        match result {
            DimensionResult::Regression(detail) => {
                tracing::warn!(function = %fc.name_str, dimension = dim, %detail,
                    "AIMS REGRESSION");
            }
            DimensionResult::Improvement(detail) => {
                tracing::info!(function = %fc.name_str, dimension = dim, %detail,
                    "AIMS improvement over legacy");
            }
            DimensionResult::Match | DimensionResult::Skipped(_) => {}
        }
    }

    if fc.immortal_skips > 0 {
        tracing::info!(
            function = %fc.name_str,
            immortal_vars = fc.immortal_skips,
            "AIMS immortal RC skips (RC improvement partially attributable to immortal objects)"
        );
    }
}

#[cfg(test)]
mod tests;
