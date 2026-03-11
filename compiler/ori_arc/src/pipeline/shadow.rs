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
//!
//! # Gate criterion
//!
//! Zero regressions (AIMS weaker than legacy). Improvements are expected and logged.

use ori_ir::{Name, StringInterner};
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::AccessClass;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::{AnnotatedSig, Ownership};
use crate::uniqueness::{CowAnnotations, UniquenessSummary};
use crate::ArcClassification;

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
    Skipped(
        #[expect(
            dead_code,
            reason = "diagnostic detail for tracing; not read in production"
        )]
        String,
    ),
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
}

impl ShadowComparisonReport {
    fn has_regressions(&self) -> bool {
        self.param_regressions > 0 || self.return_regressions > 0 || self.cow_regressions > 0
    }
}

/// AIMS analysis snapshot for a single function (computed pre-mutation).
struct AimsSnapshot {
    contract: Option<MemoryContract>,
    cow_annotations: CowAnnotations,
}

/// Run the shadow comparison pipeline.
///
/// 1. Run AIMS analysis (read-only) on unmodified functions
/// 2. Run legacy pipeline (mutating) — actual output
/// 3. Compare AIMS predictions against legacy results
/// 4. Log via tracing
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

    // Phase 2: AIMS per-function analysis + COW prediction (read-only).
    let mut aims_snapshots: FxHashMap<Name, AimsSnapshot> = FxHashMap::default();
    for func in functions.iter() {
        let state_map =
            crate::aims::intraprocedural::analyze_function(func, classifier, &contracts, &[]);
        let aims_cow =
            crate::aims::emit_rc::cow::compute_aims_cow_annotations(func, &state_map, interner);
        aims_snapshots.insert(
            func.name,
            AimsSnapshot {
                contract: contracts.get(&func.name).cloned(),
                cow_annotations: aims_cow,
            },
        );
    }

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

    // Phase 5: Compare and report.
    let report = compare_all(functions, &aims_snapshots, &uniqueness_summaries, interner);
    log_report(&report);

    all_problems
}

// Comparison logic

fn compare_all(
    functions: &[ArcFunction],
    aims_snapshots: &FxHashMap<Name, AimsSnapshot>,
    old_summaries: &FxHashMap<Name, UniquenessSummary>,
    interner: &StringInterner,
) -> ShadowComparisonReport {
    let mut report = ShadowComparisonReport {
        per_function: Vec::new(),
        total_functions: functions.len(),
        param_matches: 0,
        param_improvements: 0,
        param_regressions: 0,
        return_matches: 0,
        return_improvements: 0,
        return_regressions: 0,
        cow_matches: 0,
        cow_improvements: 0,
        cow_regressions: 0,
    };

    for func in functions {
        let snapshot = aims_snapshots.get(&func.name);
        let old_summary = old_summaries.get(&func.name);
        let name_str = interner.lookup(func.name).to_string();

        let comparison = compare_function(func, snapshot, old_summary, name_str);

        tally_dimension(
            &comparison.param_ownership,
            &mut report.param_matches,
            &mut report.param_improvements,
            &mut report.param_regressions,
        );
        tally_dimension(
            &comparison.return_uniqueness,
            &mut report.return_matches,
            &mut report.return_improvements,
            &mut report.return_regressions,
        );
        tally_dimension(
            &comparison.cow_annotations,
            &mut report.cow_matches,
            &mut report.cow_improvements,
            &mut report.cow_regressions,
        );

        report.per_function.push(comparison);
    }

    report
}

fn tally_dimension(
    result: &DimensionResult,
    matches: &mut usize,
    improvements: &mut usize,
    regressions: &mut usize,
) {
    match result {
        DimensionResult::Match => *matches += 1,
        DimensionResult::Improvement(_) => *improvements += 1,
        DimensionResult::Regression(_) => *regressions += 1,
        DimensionResult::Skipped(_) => {}
    }
}

fn compare_function(
    func: &ArcFunction,
    snapshot: Option<&AimsSnapshot>,
    old_summary: Option<&UniquenessSummary>,
    name_str: String,
) -> FunctionComparison {
    let Some(snapshot) = snapshot else {
        return FunctionComparison {
            name: func.name,
            name_str,
            param_ownership: DimensionResult::Skipped("no AIMS contract".into()),
            return_uniqueness: DimensionResult::Skipped("no AIMS contract".into()),
            cow_annotations: DimensionResult::Skipped("no AIMS snapshot".into()),
        };
    };

    FunctionComparison {
        name: func.name,
        param_ownership: compare_param_ownership(func, snapshot),
        return_uniqueness: compare_return_uniqueness(snapshot, old_summary),
        cow_annotations: compare_cow_annotations(&snapshot.cow_annotations, &func.cow_annotations),
        name_str,
    }
}

/// Compare parameter ownership: AIMS contracts vs legacy borrow inference.
///
/// AIMS `Borrowed` where legacy says `Owned` = improvement (fewer RC ops).
/// AIMS `Owned` where legacy says `Borrowed` = regression (more RC ops).
fn compare_param_ownership(func: &ArcFunction, snapshot: &AimsSnapshot) -> DimensionResult {
    let Some(contract) = &snapshot.contract else {
        return DimensionResult::Skipped("no AIMS contract".into());
    };

    if func.params.len() != contract.params.len() {
        return DimensionResult::Skipped(format!(
            "param count mismatch: legacy={}, AIMS={}",
            func.params.len(),
            contract.params.len()
        ));
    }

    let mut improvements = Vec::new();
    let mut regressions = Vec::new();

    for (i, (param, pc)) in func.params.iter().zip(&contract.params).enumerate() {
        let aims_ownership = match pc.access {
            AccessClass::Borrowed => Ownership::Borrowed,
            AccessClass::Owned => Ownership::Owned,
        };

        if aims_ownership == param.ownership {
            continue;
        }

        if aims_ownership == Ownership::Borrowed {
            improvements.push(format!("param {i}: AIMS=Borrowed, legacy=Owned"));
        } else {
            regressions.push(format!("param {i}: AIMS=Owned, legacy=Borrowed"));
        }
    }

    if !regressions.is_empty() {
        DimensionResult::Regression(regressions.join("; "))
    } else if !improvements.is_empty() {
        DimensionResult::Improvement(improvements.join("; "))
    } else {
        DimensionResult::Match
    }
}

/// Compare return uniqueness: AIMS contract vs legacy `UniquenessSummary`.
///
/// AIMS `Unique` where legacy says `MaybeShared` = improvement.
/// AIMS `MaybeShared` where legacy says `Unique` = regression.
fn compare_return_uniqueness(
    snapshot: &AimsSnapshot,
    old_summary: Option<&UniquenessSummary>,
) -> DimensionResult {
    let Some(contract) = &snapshot.contract else {
        return DimensionResult::Skipped("no AIMS contract".into());
    };
    let Some(old) = old_summary else {
        return DimensionResult::Skipped("no legacy summary".into());
    };

    let aims_rank = uniqueness_rank_aims(contract.return_info.uniqueness);
    let legacy_rank = uniqueness_rank_legacy(old.return_val);

    match aims_rank.cmp(&legacy_rank) {
        std::cmp::Ordering::Equal => DimensionResult::Match,
        // Lower rank = tighter (more optimistic) = improvement
        std::cmp::Ordering::Less => DimensionResult::Improvement(format!(
            "AIMS={}, legacy={}",
            aims_uniqueness_name(contract.return_info.uniqueness),
            old.return_val,
        )),
        std::cmp::Ordering::Greater => DimensionResult::Regression(format!(
            "AIMS={}, legacy={}",
            aims_uniqueness_name(contract.return_info.uniqueness),
            old.return_val,
        )),
    }
}

/// Compare COW annotations by counting `StaticUnique` optimizations.
///
/// More `StaticUnique` = better (more COW checks eliminated).
/// Compares counts rather than per-operation matching because the two
/// pipelines may use different block numbering.
fn compare_cow_annotations(
    aims_cow: &CowAnnotations,
    legacy_cow: &CowAnnotations,
) -> DimensionResult {
    let aims_su = aims_cow.static_unique_count();
    let legacy_su = legacy_cow.static_unique_count();
    let aims_total = aims_cow.len();
    let legacy_total = legacy_cow.len();

    if aims_su == legacy_su && aims_total == legacy_total {
        DimensionResult::Match
    } else if aims_su > legacy_su {
        DimensionResult::Improvement(format!(
            "StaticUnique: AIMS={aims_su}, legacy={legacy_su} (total: AIMS={aims_total}, legacy={legacy_total})"
        ))
    } else if aims_su < legacy_su {
        DimensionResult::Regression(format!(
            "StaticUnique: AIMS={aims_su}, legacy={legacy_su} (total: AIMS={aims_total}, legacy={legacy_total})"
        ))
    } else {
        // Same StaticUnique count, different totals — not a meaningful regression
        DimensionResult::Match
    }
}

// Helpers

fn uniqueness_rank_aims(u: crate::aims::lattice::Uniqueness) -> u8 {
    match u {
        crate::aims::lattice::Uniqueness::Unique => 0,
        crate::aims::lattice::Uniqueness::MaybeShared => 1,
        crate::aims::lattice::Uniqueness::Shared => 2,
    }
}

fn uniqueness_rank_legacy(u: crate::uniqueness::Uniqueness) -> u8 {
    match u {
        crate::uniqueness::Uniqueness::Unique => 0,
        crate::uniqueness::Uniqueness::MaybeShared => 1,
        crate::uniqueness::Uniqueness::Shared => 2,
    }
}

fn aims_uniqueness_name(u: crate::aims::lattice::Uniqueness) -> &'static str {
    match u {
        crate::aims::lattice::Uniqueness::Unique => "Unique",
        crate::aims::lattice::Uniqueness::MaybeShared => "MaybeShared",
        crate::aims::lattice::Uniqueness::Shared => "Shared",
    }
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
        "AIMS shadow comparison summary"
    );

    // Log individual regressions as warnings.
    for fc in &report.per_function {
        if let DimensionResult::Regression(detail) = &fc.param_ownership {
            tracing::warn!(function = %fc.name_str, dimension = "param_ownership", %detail,
                "AIMS REGRESSION: weaker than legacy");
        }
        if let DimensionResult::Regression(detail) = &fc.return_uniqueness {
            tracing::warn!(function = %fc.name_str, dimension = "return_uniqueness", %detail,
                "AIMS REGRESSION: weaker than legacy");
        }
        if let DimensionResult::Regression(detail) = &fc.cow_annotations {
            tracing::warn!(function = %fc.name_str, dimension = "cow_annotations", %detail,
                "AIMS REGRESSION: weaker than legacy");
        }
    }

    // Log improvements at info level.
    if report.param_improvements > 0
        || report.return_improvements > 0
        || report.cow_improvements > 0
    {
        for fc in &report.per_function {
            if let DimensionResult::Improvement(detail) = &fc.param_ownership {
                tracing::info!(function = %fc.name_str, dimension = "param_ownership", %detail,
                    "AIMS improvement over legacy");
            }
            if let DimensionResult::Improvement(detail) = &fc.return_uniqueness {
                tracing::info!(function = %fc.name_str, dimension = "return_uniqueness", %detail,
                    "AIMS improvement over legacy");
            }
            if let DimensionResult::Improvement(detail) = &fc.cow_annotations {
                tracing::info!(function = %fc.name_str, dimension = "cow_annotations", %detail,
                    "AIMS improvement over legacy");
            }
        }
    }

    if report.has_regressions() {
        tracing::warn!(
            param = report.param_regressions,
            return_uniq = report.return_regressions,
            cow = report.cow_regressions,
            "AIMS shadow comparison: REGRESSIONS DETECTED — Stage 1A gate FAILED"
        );
    } else {
        tracing::info!("AIMS shadow comparison: no regressions — Stage 1A gate PASSED");
    }
}

#[cfg(test)]
mod tests;
