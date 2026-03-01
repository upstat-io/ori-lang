//! Reset/Reuse detection for ARC IR (Section 07.6).
//!
//! After RC insertion (§07.2), identifies opportunities for in-place
//! constructor reuse: when an `RcDec` is immediately followed by a
//! `Construct` of the same type, the memory can be reused instead of
//! freed and reallocated.
//!
//! This pass replaces:
//! ```text
//! RcDec { var: x }
//! Construct { dst: y, ty: T, ctor, args }
//! ```
//! with:
//! ```text
//! Reset { var: x, token: t }
//! Reuse { token: t, dst: y, ty: T, ctor, args }
//! ```
//!
//! where `t` is a fresh reuse token. The `Reset`/`Reuse` pair is later
//! expanded by Section 09 into a conditional: if `x` is uniquely owned
//! (RC == 1), reuse the memory in-place; otherwise allocate fresh.
//!
//! # Constraints
//!
//! A `RcDec`/`Construct` pair is only valid for reset/reuse if:
//!
//! 1. The types match: `typeof(x) == ty` of the `Construct`.
//! 2. No use of `x` between the `RcDec` and `Construct` (no aliasing).
//! 3. The type needs RC (is heap-allocated).
//!
//! # References
//!
//! - Lean 4: `src/Lean/Compiler/IR/ExpandResetReuse.lean`
//! - Lean 4: `src/Lean/Compiler/IR/ResetReuse.lean`
//! - Koka: Perceus paper §4 (reuse analysis)

use ori_types::{Idx, Pool};
use rustc_hash::FxHashSet;

use crate::graph::{DominatorTree, PostDominatorTree};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId, CtorKind, ValueRepr};
use crate::liveness::RefinedLiveness;
use crate::ArcClassification;

/// Detect and replace `RcDec`/`Construct` pairs with `Reset`/`Reuse`.
///
/// Scans each block forward for matching pairs. Only intra-block matches
/// are considered (cross-block reuse would require more complex analysis).
///
/// # Arguments
///
/// * `func` — the ARC IR function to transform (mutated in place).
/// * `classifier` — type classifier for `needs_rc()` checks.
/// * `pool` — type pool for computing [`ValueRepr`] of token variables.
pub(crate) fn detect_reset_reuse(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) {
    // Precondition: detection creates Reset/Reuse — none should exist yet.
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::Reset { .. } | ArcInstr::Reuse { .. })),
        "detect_reset_reuse: IR already contains Reset/Reuse — pipeline ordering error"
    );

    tracing::debug!(
        function = func.name.raw(),
        "detecting reset/reuse opportunities"
    );

    let num_blocks = func.blocks.len();

    for block_idx in 0..num_blocks {
        detect_in_block(func, block_idx, classifier, pool);
    }
}

/// Detect reset/reuse pairs within a single block.
///
/// Uses a forward scan. When we find an `RcDec`, we look ahead for a
/// matching `Construct`. If found and constraints are satisfied, replace
/// both instructions.
fn detect_in_block(
    func: &mut ArcFunction,
    block_idx: usize,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) {
    // Track which RcDec indices have been paired, so we don't pair twice.
    let mut paired_decs: FxHashSet<usize> = FxHashSet::default();
    // Track which Construct indices have been paired.
    let mut paired_constructs: FxHashSet<usize> = FxHashSet::default();

    // Phase 1: Scan — collect matched (dec_idx, construct_idx, dec_ty)
    // triples. Token allocation is deferred to after the scan to avoid
    // a borrow conflict (body borrows func.blocks immutably, fresh_var
    // borrows func mutably).
    let mut matched: Vec<(usize, usize, Idx)> = Vec::new();

    let body = &func.blocks[block_idx].body;

    for i in 0..body.len() {
        if paired_decs.contains(&i) {
            continue;
        }

        // Look for RcDec instructions.
        let dec_var = match &body[i] {
            ArcInstr::RcDec { var, .. } => *var,
            _ => continue,
        };

        // Check that the type needs RC (skip scalars).
        let dec_ty = func.var_type(dec_var);
        if !classifier.needs_rc(dec_ty) {
            continue;
        }

        // Scan forward for a matching Construct.
        for (j, candidate) in body.iter().enumerate().skip(i + 1) {
            if paired_constructs.contains(&j) {
                continue;
            }

            // Check constraint: no use of dec_var between i and j.
            if candidate.uses_var(dec_var) && !matches!(candidate, ArcInstr::Construct { .. }) {
                // dec_var is used before we find a Construct → cannot reuse.
                break;
            }

            match candidate {
                ArcInstr::Construct { ty, ctor, .. } if *ty == dec_ty => {
                    // Skip collection constructors: their data lives in a
                    // separate RC'd buffer, not inline struct fields. The
                    // fast-path Set instruction assumes struct GEP layout,
                    // which doesn't apply to List/Map/Set buffers.
                    if is_collection_ctor(ctor) {
                        continue;
                    }
                    // Check that dec_var is NOT used in the Construct's args.
                    // (If it is, there's an alias and reuse is unsafe.)
                    if candidate.uses_var(dec_var) {
                        continue;
                    }

                    matched.push((i, j, dec_ty));
                    paired_decs.insert(i);
                    paired_constructs.insert(j);
                    break;
                }
                _ => {
                    // Check if this instruction uses dec_var → constraint violation.
                    if candidate.uses_var(dec_var) {
                        break;
                    }
                }
            }
        }
    }

    // Phase 2: Allocate fresh token variables with correct repr (body borrow is released).
    let pairs: Vec<(usize, usize, ArcVarId)> = matched
        .into_iter()
        .map(|(dec_idx, construct_idx, dec_ty)| {
            let repr = repr_for_type(classifier, pool, dec_ty);
            let token = func.fresh_var_repr(dec_ty, repr);
            (dec_idx, construct_idx, token)
        })
        .collect();

    // Apply replacements (in reverse order to preserve indices).
    // Since we're replacing in-place at fixed indices, order doesn't matter
    // for correctness, but we process pairs as collected.
    let body = &mut func.blocks[block_idx].body;
    for (dec_idx, construct_idx, token) in pairs {
        // Extract Construct details before replacing.
        let (dst, ty, ctor, args) = match &body[construct_idx] {
            ArcInstr::Construct {
                dst,
                ty,
                ctor,
                args,
            } => (*dst, *ty, *ctor, args.clone()),
            _ => unreachable!("paired construct index must be a Construct"),
        };

        let dec_var = match &body[dec_idx] {
            ArcInstr::RcDec { var, .. } => *var,
            _ => unreachable!("paired dec index must be an RcDec"),
        };

        // Replace RcDec → Reset.
        body[dec_idx] = ArcInstr::Reset {
            var: dec_var,
            token,
        };

        // Replace Construct → Reuse.
        body[construct_idx] = ArcInstr::Reuse {
            token,
            dst,
            ty,
            ctor,
            args,
        };
    }
}

/// Cross-block reset/reuse detection using dominator tree and refined liveness.
///
/// Extends intra-block detection to find reuse opportunities across basic
/// blocks. The canonical case is linked-list `map`:
///
/// ```text
/// B0: RcDec(node)          ← unpaired after intra-block detection
/// B1: ...                   ← dominated by B0
/// B2: new = Construct(Node) ← allocation in dominated block
/// ```
///
/// If `node` is only live-for-drop (not read as operand) in B1, then we can
/// replace `RcDec(node)` → `Reset(node, token)` in B0 and `Construct` →
/// `Reuse(token, ...)` in B2.
///
/// # Safety
///
/// This transformation is valid because:
/// 1. B0 dominates B2 → the token is always available at B2
/// 2. `node` is not live-for-use in any block between B0 and B2 → no aliasing
/// 3. The types match → memory layout is compatible for reuse
///
/// # Arguments
///
/// * `func` — the ARC IR function (mutated in place).
/// * `classifier` — type classifier for `needs_rc()` checks.
/// * `dom_tree` — precomputed dominator tree.
/// * `refined` — precomputed refined liveness per block.
pub fn detect_reset_reuse_cfg(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    dom_tree: &DominatorTree,
    post_dom_tree: &PostDominatorTree,
    refined: &[RefinedLiveness],
    pool: &Pool,
) {
    // Step 1: Run intra-block detection first (fast path).
    detect_reset_reuse(func, classifier, pool);

    // Step 2: Collect unpaired RcDec instructions.
    // After intra-block detection, remaining RcDec instructions are candidates
    // for cross-block pairing.
    let mut unpaired_decs: Vec<(usize, usize, ArcVarId, Idx)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::RcDec { var, .. } = instr {
                let ty = func.var_type(*var);
                if classifier.needs_rc(ty) {
                    unpaired_decs.push((block_idx, instr_idx, *var, ty));
                }
            }
        }
    }

    if unpaired_decs.is_empty() {
        return;
    }

    tracing::debug!(
        unpaired = unpaired_decs.len(),
        "cross-block reset/reuse: scanning dominated blocks"
    );

    // Step 3: Find cross-block matches and apply replacements.
    let matches = find_cross_block_matches(func, &unpaired_decs, dom_tree, post_dom_tree, refined);

    if matches.is_empty() {
        return;
    }

    tracing::debug!(
        cross_block_pairs = matches.len(),
        "cross-block reset/reuse: applying transformations"
    );

    // Step 4: Apply cross-block replacements.
    for m in &matches {
        apply_cross_block_match(func, m, classifier, pool);
    }
}

/// Walk dominated blocks for each unpaired `RcDec` to find a matching `Construct`.
///
/// A match requires: same type, no aliasing (`dec_var` not live-for-use),
/// and the construct block post-dominates the dec block (ensuring the
/// reuse token is consumed on all paths).
fn find_cross_block_matches(
    func: &ArcFunction,
    unpaired_decs: &[(usize, usize, ArcVarId, Idx)],
    dom_tree: &DominatorTree,
    post_dom_tree: &PostDominatorTree,
    refined: &[RefinedLiveness],
) -> Vec<CrossBlockMatch> {
    let num_blocks = func.blocks.len();
    let mut paired_constructs: FxHashSet<(usize, usize)> = FxHashSet::default();
    let mut matches: Vec<CrossBlockMatch> = Vec::new();

    for &(dec_block_idx, dec_instr_idx, dec_var, dec_ty) in unpaired_decs {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let dec_block_id = ArcBlockId::new(dec_block_idx as u32);
        let dominated = dom_tree.dominated_preorder(dec_block_id, num_blocks);

        let mut found = false;
        for &target_block_id in &dominated {
            let target_idx = target_block_id.index();

            if target_idx == dec_block_idx {
                continue;
            }

            if target_idx < refined.len() && refined[target_idx].live_for_use.contains(&dec_var) {
                break;
            }

            for (ci, instr) in func.blocks[target_idx].body.iter().enumerate() {
                if paired_constructs.contains(&(target_idx, ci)) {
                    continue;
                }
                if let ArcInstr::Construct { ty, ctor, .. } = instr {
                    if *ty == dec_ty
                        && !is_collection_ctor(ctor)
                        && !instr.uses_var(dec_var)
                        && post_dom_tree.post_dominates(target_block_id, dec_block_id)
                    {
                        matches.push(CrossBlockMatch {
                            dec_block: dec_block_idx,
                            dec_instr: dec_instr_idx,
                            dec_var,
                            construct_block: target_idx,
                            construct_instr: ci,
                        });
                        paired_constructs.insert((target_idx, ci));
                        found = true;
                        break;
                    }
                }
            }

            if found {
                break;
            }
        }
    }

    matches
}

/// Apply a single cross-block `RcDec`→`Reset` / `Construct`→`Reuse` replacement.
fn apply_cross_block_match(
    func: &mut ArcFunction,
    m: &CrossBlockMatch,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) {
    let dec_ty = func.var_type(m.dec_var);
    let repr = repr_for_type(classifier, pool, dec_ty);
    let token = func.fresh_var_repr(dec_ty, repr);

    let (dst, ty, ctor, args) = match &func.blocks[m.construct_block].body[m.construct_instr] {
        ArcInstr::Construct {
            dst,
            ty,
            ctor,
            args,
        } => (*dst, *ty, *ctor, args.clone()),
        _ => unreachable!("paired construct must be a Construct"),
    };

    func.blocks[m.dec_block].body[m.dec_instr] = ArcInstr::Reset {
        var: m.dec_var,
        token,
    };

    func.blocks[m.construct_block].body[m.construct_instr] = ArcInstr::Reuse {
        token,
        dst,
        ty,
        ctor,
        args,
    };
}

/// Collection constructors use a separate RC'd buffer layout, not inline
/// struct fields. The `Set` fast-path in expansion assumes struct GEP, so
/// these constructors must be excluded from Reset/Reuse pairing.
fn is_collection_ctor(ctor: &CtorKind) -> bool {
    matches!(
        ctor,
        CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral
    )
}

/// Compute [`ValueRepr`] for a type from classifier + pool.
fn repr_for_type(classifier: &dyn ArcClassification, pool: &Pool, ty: Idx) -> ValueRepr {
    let class = classifier.arc_class(ty);
    ValueRepr::from_arc_class(class, pool, ty)
}

/// A matched cross-block RcDec/Construct pair.
struct CrossBlockMatch {
    dec_block: usize,
    dec_instr: usize,
    dec_var: ArcVarId,
    construct_block: usize,
    construct_instr: usize,
}

#[cfg(test)]
mod tests;
