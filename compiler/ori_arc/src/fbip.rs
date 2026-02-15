//! FBIP (Functional But In-Place) diagnostic analysis.
//!
//! After the ARC pipeline runs, this pass catalogs which constructor-reuse
//! opportunities were achieved (`Reset`/`Reuse` pairs) and which were missed
//! (`RcDec` of a value followed by a `Construct` of the same type, without
//! reuse). This helps developers understand where heap allocation can be
//! avoided and why.
//!
//! Inspired by Koka's `CheckFBIP.hs` — a read-only diagnostic pass that
//! reports on the effectiveness of Perceus reference counting.
//!
//! # Usage
//!
//! Run after the full ARC pipeline (insert → detect → expand → eliminate).
//! The report is purely informational and does not modify the IR.

use ori_ir::Span;
use ori_types::Idx;

use crate::graph::DominatorTree;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId};
use crate::liveness::RefinedLiveness;
use crate::ArcClassification;

/// Summary of FBIP analysis for a single function.
pub struct FbipReport {
    /// Successfully paired Reset/Reuse — allocation is reused in-place.
    pub achieved: Vec<ReuseOpportunity>,
    /// Unpaired `RcDec` + `Construct` that could have been reuse but weren't.
    pub missed: Vec<MissedReuse>,
    /// `true` if the function achieves full FBIP (all allocations reused).
    pub is_fbip: bool,
}

/// A successfully achieved reuse opportunity.
pub struct ReuseOpportunity {
    /// The variable whose allocation is recycled.
    pub reset_var: ArcVarId,
    /// The constructor that reuses the allocation.
    pub reuse_dst: ArcVarId,
    /// The type being reused.
    pub ty: Idx,
    /// Block where the reuse occurs.
    pub block: ArcBlockId,
}

/// A missed reuse opportunity.
pub struct MissedReuse {
    /// The variable being decremented (potential allocation to reuse).
    pub dec_var: ArcVarId,
    /// Block where the `RcDec` occurs.
    pub dec_block: ArcBlockId,
    /// The Construct destination that could have reused the allocation.
    pub construct_dst: Option<ArcVarId>,
    /// Block where the Construct occurs.
    pub construct_block: Option<ArcBlockId>,
    /// Why the reuse couldn't be achieved.
    pub reason: MissedReuseReason,
}

/// Reasons why an allocation reuse opportunity was missed.
pub enum MissedReuseReason {
    /// The decrement and construct have different types.
    TypeMismatch { dec_type: Idx, construct_type: Idx },
    /// The decremented variable is still used between the Dec and Construct.
    IntermediateUse { use_span: Option<Span> },
    /// The `Construct` is not dominated by the `RcDec`.
    NoDominance,
    /// The variable might be shared (refcount > 1), so reset is unsafe.
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
pub fn analyze_fbip(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    dom_tree: &DominatorTree,
    refined: &[RefinedLiveness],
) -> FbipReport {
    let mut achieved = Vec::new();
    let mut missed = Vec::new();

    // Phase 1: Collect achieved reuse (expanded Reset/Reuse or IsShared patterns).
    //
    // After expand_reset_reuse, Reset/Reuse have been lowered to IsShared
    // branches. But we can still detect the pattern by looking for Reset/Reuse
    // in the pre-expansion IR, or for IsShared in the post-expansion IR.
    //
    // Since we run AFTER expansion, look for the IsShared pattern:
    //   IsShared(var) → branch → fast path (reuse) / slow path (alloc)
    //
    // Also catch any un-expanded Reset/Reuse (should only happen if expansion
    // was skipped in testing).
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

    // Phase 2: Collect unpaired RcDec instructions (potential missed reuse).
    //
    // An RcDec that is NOT part of a Reset/Reuse pattern is a missed
    // opportunity IF there's a Construct of the same type somewhere
    // reachable.
    let mut all_constructs: Vec<(ArcBlockId, ArcVarId, Idx)> = Vec::new();
    let mut unpaired_decs: Vec<(ArcBlockId, ArcVarId, Idx)> = Vec::new();

    // Collect all Construct instructions.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, ty, .. } = instr {
                if classifier.needs_rc(*ty) {
                    all_constructs.push((block.id, *dst, *ty));
                }
            }
        }
    }

    // Collect RcDec that are not preceded by IsShared (i.e., not part of reuse).
    //
    // Heuristic: an RcDec is "unpaired" if the variable was never tested
    // with IsShared in the same block. This isn't perfect but catches the
    // common case.
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
            if let ArcInstr::RcDec { var } = instr {
                if !is_shared_vars.contains(var) && classifier.needs_rc(func.var_type(*var)) {
                    unpaired_decs.push((block.id, *var, func.var_type(*var)));
                }
            }
        }
    }

    // Phase 3: Match unpaired RcDec against Constructs.
    for &(dec_block, dec_var, dec_type) in &unpaired_decs {
        // Find a Construct of the same type in a dominated block.
        let matching = all_constructs.iter().find(|&&(con_block, _, con_type)| {
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
            let type_mismatch = all_constructs
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
                let any_construct = all_constructs.iter().find(|&&(_, _, t)| t == dec_type);

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

    let is_fbip = missed.is_empty() && !achieved.is_empty();

    FbipReport {
        achieved,
        missed,
        is_fbip,
    }
}

#[cfg(test)]
mod tests;
