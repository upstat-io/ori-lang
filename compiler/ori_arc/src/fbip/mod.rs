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
use ori_types::Idx;

use crate::graph::DominatorTree;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId};
use crate::liveness::RefinedLiveness;
use crate::ArcClassification;

/// Summary of FBIP analysis for a single function.
pub(crate) struct FbipReport {
    /// Successfully paired donor/recipient in the current Reset/Reuse adapter.
    pub(crate) achieved: Vec<ReuseOpportunity>,
    /// Unpaired logical release + `Construct` that could have been reused.
    pub(crate) missed: Vec<MissedReuse>,
    /// `true` if the function achieves full FBIP (all allocations reused).
    #[cfg_attr(not(test), expect(dead_code, reason = "read only in tests"))]
    pub(crate) is_fbip: bool,
}

/// A successfully achieved reuse opportunity.
#[expect(
    dead_code,
    reason = "diagnostic output — inner fields for future detailed FBIP reporting"
)]
pub(crate) struct ReuseOpportunity {
    /// The variable whose allocation is recycled.
    pub(crate) reset_var: ArcVarId,
    /// The constructor that reuses the allocation.
    pub(crate) reuse_dst: ArcVarId,
    /// The type being reused.
    pub(crate) ty: Idx,
    /// Block where the reuse occurs.
    pub(crate) block: ArcBlockId,
}

/// A missed reuse opportunity.
#[expect(
    dead_code,
    reason = "diagnostic output — inner fields for future detailed FBIP reporting"
)]
pub(crate) struct MissedReuse {
    /// The variable being decremented (potential allocation to reuse).
    pub(crate) dec_var: ArcVarId,
    /// Block where the `RcDec` occurs.
    pub(crate) dec_block: ArcBlockId,
    /// The Construct destination that could have reused the allocation.
    pub(crate) construct_dst: Option<ArcVarId>,
    /// Block where the Construct occurs.
    pub(crate) construct_block: Option<ArcBlockId>,
    /// Why the reuse couldn't be achieved.
    pub(crate) reason: MissedReuseReason,
}

/// Reasons why an allocation reuse opportunity was missed.
#[expect(
    dead_code,
    reason = "diagnostic output — variant fields for future detailed FBIP reporting"
)]
pub(crate) enum MissedReuseReason {
    /// The decrement and construct have different types.
    TypeMismatch { dec_type: Idx, construct_type: Idx },
    /// The decremented variable is still used between the Dec and Construct.
    IntermediateUse { use_span: Option<Span> },
    /// The `Construct` is not dominated by the `RcDec`.
    NoDominance,
    /// The variable may have competing logical owners, so reuse is unsafe.
    PossiblyShared,
    /// No matching Construct of the same type exists.
    NoMatchingConstruct,
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
/// * `dom_tree` — dominator tree for dominance queries.
/// * `refined` — refined liveness for aliasing checks.
pub(crate) fn analyze_fbip(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    dom_tree: &DominatorTree,
    refined: &[RefinedLiveness],
) -> FbipReport {
    let achieved = collect_achieved_reuse(func);
    let (constructs, unpaired_decs) = collect_reuse_candidates(func, classifier);
    let missed = classify_missed_reuse(&constructs, &unpaired_decs, dom_tree, refined);
    let is_fbip = missed.is_empty() && !achieved.is_empty();

    FbipReport {
        achieved,
        missed,
        is_fbip,
    }
}

type ReuseCandidate = (ArcBlockId, ArcVarId, Idx);

fn collect_achieved_reuse(func: &ArcFunction) -> Vec<ReuseOpportunity> {
    let mut achieved = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Reuse { token, dst, ty, .. } = instr {
                achieved.push(ReuseOpportunity {
                    reset_var: *token,
                    reuse_dst: *dst,
                    ty: *ty,
                    block: block.id,
                });
            }
        }
    }
    achieved
}

fn collect_reuse_candidates(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
) -> (Vec<ReuseCandidate>, Vec<ReuseCandidate>) {
    let mut constructs = Vec::new();
    let mut unpaired_decs = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, ty, .. } = instr {
                if classifier.has_managed_ownership_obligation(*ty) {
                    constructs.push((block.id, *dst, *ty));
                }
            }
        }
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
                    unpaired_decs.push((block.id, *var, func.var_type(*var)));
                }
            }
        }
    }
    (constructs, unpaired_decs)
}

fn classify_missed_reuse(
    constructs: &[ReuseCandidate],
    unpaired_decs: &[ReuseCandidate],
    dom_tree: &DominatorTree,
    refined: &[RefinedLiveness],
) -> Vec<MissedReuse> {
    let mut missed = Vec::new();
    for &(dec_block, dec_var, dec_type) in unpaired_decs {
        // Find a Construct of the same type in a dominated block.
        let matching = constructs.iter().find(|&&(con_block, _, con_type)| {
            con_type == dec_type && dom_tree.dominates(dec_block, con_block)
        });

        if let Some(&(con_block, con_dst, _)) = matching {
            // Check aliasing: if dec_var is live_for_use in the construct's
            // block, the value is still needed (can't reset it).
            let con_block_idx = con_block.index();
            let reason = if con_block_idx < refined.len()
                && refined[con_block_idx].live_for_use.contains(&dec_var)
            {
                MissedReuseReason::IntermediateUse { use_span: None }
            } else {
                // Should have been caught by detect_reset_reuse — if it
                // wasn't, the variable might be possibly shared.
                MissedReuseReason::PossiblyShared
            };
            missed.push(MissedReuse {
                dec_var,
                dec_block,
                construct_dst: Some(con_dst),
                construct_block: Some(con_block),
                reason,
            });
        } else {
            // Check if there's a type mismatch or no Construct at all.
            let type_mismatch = constructs
                .iter()
                .find(|&&(con_block, _, _)| dom_tree.dominates(dec_block, con_block));

            if let Some(&(con_block, con_dst, con_type)) = type_mismatch {
                missed.push(MissedReuse {
                    dec_var,
                    dec_block,
                    construct_dst: Some(con_dst),
                    construct_block: Some(con_block),
                    reason: MissedReuseReason::TypeMismatch {
                        dec_type,
                        construct_type: con_type,
                    },
                });
            } else {
                // No dominated Construct at all — check for non-dominated ones.
                let any_construct = constructs.iter().find(|&&(_, _, t)| t == dec_type);

                let reason = if any_construct.is_some() {
                    MissedReuseReason::NoDominance
                } else {
                    MissedReuseReason::NoMatchingConstruct
                };
                missed.push(MissedReuse {
                    dec_var,
                    dec_block,
                    construct_dst: None,
                    construct_block: None,
                    reason,
                });
            }
        }
    }
    missed
}

/// Check FBIP enforcement for a post-pipeline ARC function.
///
/// Returns an `ArcProblem::FbipViolation` if the function has missed reuse
/// opportunities. Only called for functions annotated `#fbip`.
///
/// Rebuilds dominator tree and refined liveness from the post-pipeline IR
/// state, then delegates to [`analyze_fbip`].
pub fn check_fbip_enforcement(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    func_name: &str,
    func_span: Span,
) -> Option<crate::lower::ArcProblem> {
    let dom_tree = DominatorTree::build(func);
    let (refined, _liveness) = crate::liveness::compute_refined_liveness(func, classifier);

    let report = analyze_fbip(func, classifier, &dom_tree, &refined);

    if report.missed.is_empty() {
        // Fully compliant (or nothing to reuse) — no violation.
        None
    } else {
        Some(crate::lower::ArcProblem::FbipViolation {
            func_name: func_name.to_string(),
            missed_count: report.missed.len(),
            achieved_count: report.achieved.len(),
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
