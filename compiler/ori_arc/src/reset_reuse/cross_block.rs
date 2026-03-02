//! Cross-block reset/reuse detection using dominator tree and refined liveness.
//!
//! Extends intra-block detection to find reuse opportunities across basic
//! blocks. The canonical case is linked-list `map`:
//!
//! ```text
//! B0: RcDec(node)          ← unpaired after intra-block detection
//! B1: ...                   ← dominated by B0
//! B2: new = Construct(Node) ← allocation in dominated block
//! ```
//!
//! If `node` is only live-for-drop (not read as operand) in B1, then we can
//! replace `RcDec(node)` → `Reset(node, token)` in B0 and `Construct` →
//! `Reuse(token, ...)` in B2.

use ori_types::{Idx, Pool};
use rustc_hash::FxHashSet;

use crate::graph::{DominatorTree, PostDominatorTree};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId};
use crate::liveness::RefinedLiveness;
use crate::ArcClassification;

use super::{detect_reset_reuse, is_collection_ctor, repr_for_type};

/// Cross-block reset/reuse detection using dominator tree and refined liveness.
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
/// * `post_dom_tree` — precomputed post-dominator tree.
/// * `refined` — precomputed refined liveness per block.
/// * `pool` — type pool for computing `ValueRepr`.
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

/// A matched cross-block RcDec/Construct pair.
struct CrossBlockMatch {
    dec_block: usize,
    dec_instr: usize,
    dec_var: ArcVarId,
    construct_block: usize,
    construct_instr: usize,
}
