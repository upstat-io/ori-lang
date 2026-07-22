//! FBIP (Functional But In-Place) diagnostic analysis.
//!
//! After logical ownership/reuse analysis runs, this pass catalogs which
//! donor/recipient reuse opportunities the current transitional projection
//! realized (`Reset`/`Reuse` pairs) and which it missed (a logical release
//! followed by a compatible construction). This helps developers understand
//! where a selected physical plan may avoid fresh storage and why.
//!
//! Historical influence: Koka's `CheckFBIP.hs` SHAPE — a read-only diagnostic pass that
//! reports on the effectiveness of Perceus reuse and ownership analysis.
//!
//! # Usage
//!
//! Run after the full ownership pipeline (analyze → realize logical facts →
//! project the current carrier).
//! The report is purely informational and does not modify the IR.

use ori_ir::Span;

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};
use crate::ArcClassification;

/// Summary of FBIP analysis for a single function.
pub(crate) struct FbipReport {
    /// Number of successfully paired donor/recipient operations.
    pub(crate) achieved_count: usize,
    /// Number of unpaired logical releases.
    pub(crate) missed_count: usize,
}

impl FbipReport {
    #[cfg(test)]
    fn is_fbip(&self) -> bool {
        self.missed_count == 0 && self.achieved_count > 0
    }
}

/// Analyze a function for FBIP properties after the ARC pipeline has run.
///
/// Catalogs achieved reuse (Reset/Reuse pairs) and missed opportunities
/// (unpaired `RcDec` + `Construct`). This is a **read-only** pass — no IR
/// modifications.
///
/// # Arguments
///
/// * `func` — the ARC IR function (post-pipeline).
/// * `classifier` — type classifier for RC checks.
pub(crate) fn analyze_fbip(func: &ArcFunction, classifier: &dyn ArcClassification) -> FbipReport {
    FbipReport {
        achieved_count: count_achieved_reuse(func),
        missed_count: count_missed_reuse(func, classifier),
    }
}

fn count_achieved_reuse(func: &ArcFunction) -> usize {
    func.blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter(|instr| matches!(instr, ArcInstr::Reuse { .. }))
        .count()
}

fn count_missed_reuse(func: &ArcFunction, classifier: &dyn ArcClassification) -> usize {
    let mut missed_count = 0;
    for block in &func.blocks {
        let is_shared_vars: rustc_hash::FxHashSet<ArcVarId> = block
            .body
            .iter()
            .filter_map(|i| match i {
                ArcInstr::IsShared { var, .. } => Some(*var),
                _ => None,
            })
            .collect();

        for instr in &block.body {
            if let ArcInstr::RcDec { var, .. } = instr {
                if !is_shared_vars.contains(var)
                    && classifier.has_managed_ownership_obligation(func.var_type(*var))
                {
                    missed_count += 1;
                }
            }
        }
    }
    missed_count
}

/// Check FBIP enforcement for a post-pipeline ARC function.
///
/// Returns an `ArcProblem::FbipViolation` if the function has missed reuse
/// opportunities. Only called for functions annotated `#fbip`.
///
pub fn check_fbip_enforcement(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    func_name: &str,
    func_span: Span,
) -> Option<crate::lower::ArcProblem> {
    let report = analyze_fbip(func, classifier);

    if report.missed_count == 0 {
        // Fully compliant (or nothing to reuse) — no violation.
        None
    } else {
        Some(crate::lower::ArcProblem::FbipViolation {
            func_name: func_name.to_string(),
            missed_count: report.missed_count,
            achieved_count: report.achieved_count,
            span: func_span,
        })
    }
}

/// Check whether a function achieves automatic FBIP through static uniqueness.
///
/// A function is "auto FBIP" if **all** of its COW operations are annotated
/// as [`CowMode::StaticUnique`](crate::uniqueness::CowMode::StaticUnique) —
/// meaning every collection mutation is provably in-place without runtime
/// RC checks. This is stronger than `#fbip` enforcement (which checks
/// reset/reuse pairing): auto FBIP means zero COW overhead, period.
///
/// Returns `true` if the function has at least one COW operation and all
/// are `StaticUnique`. Functions with no COW operations return `false`
/// (they're trivially allocation-free, but "FBIP" specifically refers to
/// functional-but-in-place mutation patterns).
pub fn is_auto_fbip(func: &ArcFunction) -> bool {
    let annotations = &func.cow_annotations;
    if annotations.is_empty() {
        return false;
    }
    annotations.static_unique_count() == annotations.len()
}

#[cfg(test)]
mod tests;
