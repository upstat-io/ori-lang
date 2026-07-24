//! Folding of trivial if/else diamonds into `Select` instructions.
//!
//! Eligible arms contain only literal or variable bindings and converge at
//! one merge block. The fold preserves arm-local definitions with fresh names,
//! emits one `Select` per merge parameter, and leaves unreachable arms for
//! block compaction.

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::graph::compute_pred_counts;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::usize_to_block_id;

/// Owned data for a select-eligible diamond.
///
/// All fields are owned (cloned from block data) so that `apply_select_fold`
/// can mutate blocks freely without borrow conflicts.
struct SelectDiamond {
    cond: ArcVarId,
    then_idx: usize,
    else_idx: usize,
    merge_idx: usize,
    then_args: Vec<ArcVarId>,
    else_args: Vec<ArcVarId>,
}

/// Fold trivial if/else diamond patterns into `Select` instructions.
#[tracing::instrument(skip_all, name = "phase3_select")]
pub(super) fn fold_select_diamonds(func: &mut ArcFunction) {
    let pred_counts = compute_pred_counts(func);

    for block_idx in 0..func.blocks.len() {
        if let Some(info) = detect_select_diamond(func, block_idx, &pred_counts) {
            tracing::debug!(
                block = block_idx,
                then = info.then_idx,
                else_ = info.else_idx,
                merge = info.merge_idx,
                "select: folding diamond"
            );
            apply_select_fold(func, block_idx, &info);
        }
    }
}

/// Check if a body is trivial — only `Let { Literal }` or `Let { Var(v) }`
/// where `v` is not defined by another instruction in the same body.
///
/// This ensures the arm body has no side effects and no chained local
/// references that would require topological renaming.
pub(super) fn is_trivial_body(body: &[ArcInstr]) -> bool {
    // Collect all vars defined in this body — SmallVec avoids heap for
    // typically 0-2 element arm bodies.
    let local_defs: SmallVec<[ArcVarId; 4]> =
        body.iter().filter_map(ArcInstr::defined_var).collect();

    body.iter().all(|instr| match instr {
        ArcInstr::Let {
            value: ArcValue::Literal(_),
            ..
        } => true,
        ArcInstr::Let {
            value: ArcValue::Var(v),
            ..
        } => !local_defs.contains(v),
        _ => false,
    })
}

/// Check if an arm block is eligible for select folding (criteria 2-4).
fn is_eligible_arm(
    func: &ArcFunction,
    arm_idx: usize,
    pred_counts: &[usize],
    block_idx: usize,
    label: &str,
) -> bool {
    if !func.blocks[arm_idx].params.is_empty() {
        tracing::trace!(block = block_idx, "{label} block has params");
        return false;
    }
    if pred_counts[arm_idx] != 1 {
        tracing::trace!(
            block = block_idx,
            preds = pred_counts[arm_idx],
            "{label} block has multiple predecessors"
        );
        return false;
    }
    if !is_trivial_body(&func.blocks[arm_idx].body) {
        tracing::trace!(block = block_idx, "{label} body is not trivial");
        return false;
    }
    true
}

/// Detect whether a block's `Branch` terminator forms a select-eligible
/// diamond pattern. Returns `None` if any criterion fails.
fn detect_select_diamond(
    func: &ArcFunction,
    block_idx: usize,
    pred_counts: &[usize],
) -> Option<SelectDiamond> {
    let ArcTerminator::Branch {
        cond,
        then_block,
        else_block,
    } = &func.blocks[block_idx].terminator
    else {
        tracing::trace!(block = block_idx, "select: not a Branch terminator");
        return None;
    };

    let then_idx = then_block.index();
    let else_idx = else_block.index();

    // Criterion 1: degenerate diamond guard.
    if then_idx == else_idx {
        tracing::trace!(
            block = block_idx,
            "select: degenerate diamond (then == else)"
        );
        return None;
    }

    // Criteria 2-4: arm eligibility (no params, single pred, trivial body).
    if !is_eligible_arm(func, then_idx, pred_counts, block_idx, "select: then") {
        return None;
    }
    if !is_eligible_arm(func, else_idx, pred_counts, block_idx, "select: else") {
        return None;
    }

    // Criterion 5: both arms terminate with Jump to the same merge block.
    let ArcTerminator::Jump {
        target: then_target_id,
        args: then_args_ref,
    } = &func.blocks[then_idx].terminator
    else {
        tracing::trace!(block = block_idx, "select: then terminator is not Jump");
        return None;
    };
    let then_target = then_target_id.index();
    let then_args = then_args_ref.clone();

    let ArcTerminator::Jump {
        target: else_target_id,
        args: else_args_ref,
    } = &func.blocks[else_idx].terminator
    else {
        tracing::trace!(block = block_idx, "select: else terminator is not Jump");
        return None;
    };
    let else_target = else_target_id.index();
    let else_args = else_args_ref.clone();

    if then_target != else_target {
        tracing::trace!(
            block = block_idx,
            then_target,
            else_target,
            "select: arm terminators don't jump to same merge block"
        );
        return None;
    }

    // Criterion 6: jump arg arity match.
    if then_args.len() != else_args.len() {
        tracing::trace!(
            block = block_idx,
            then_count = then_args.len(),
            else_count = else_args.len(),
            "select: jump arg arity mismatch"
        );
        return None;
    }

    // Criterion 7: merge params must be scalar.
    //
    // Non-scalar values (FatVal, RcPointer, Aggregate) need RC management.
    // Select materializes both operands eagerly, so the non-selected heap
    // value would leak — there's no RcDec emitted for it. Keep the branch
    // so only the taken arm's allocation runs.
    for (param_var, _ty) in &func.blocks[then_target].params {
        let repr = func.var_repr(*param_var).unwrap_or(ValueRepr::Scalar);
        if !matches!(repr, ValueRepr::Scalar) {
            tracing::trace!(
                block = block_idx,
                var = param_var.raw(),
                ?repr,
                "select: merge param is non-scalar — skipping to prevent RC leak"
            );
            return None;
        }
    }

    Some(SelectDiamond {
        cond: *cond,
        then_idx,
        else_idx,
        merge_idx: then_target,
        then_args,
        else_args,
    })
}

/// Apply the select fold transformation to a detected diamond.
///
/// Move arm-local definitions into the branch block and emit merge selects.
///
/// The resulting jump targets the merge block; both arm blocks are unreachable.
fn apply_select_fold(func: &mut ArcFunction, branch_idx: usize, diamond: &SelectDiamond) {
    // Why: Draining both arms avoids overlapping borrows of the branch block.
    let then_body: Vec<ArcInstr> = func.blocks[diamond.then_idx].body.drain(..).collect();
    let then_spans: Vec<_> = func.spans[diamond.then_idx].drain(..).collect();
    let else_body: Vec<ArcInstr> = func.blocks[diamond.else_idx].body.drain(..).collect();
    let else_spans: Vec<_> = func.spans[diamond.else_idx].drain(..).collect();

    let then_renames = move_arm_body(func, branch_idx, &then_body, &then_spans);
    let else_renames = move_arm_body(func, branch_idx, &else_body, &else_spans);

    let resolved_then: Vec<ArcVarId> = diamond
        .then_args
        .iter()
        .map(|a| then_renames.get(a).copied().unwrap_or(*a))
        .collect();
    let resolved_else: Vec<ArcVarId> = diamond
        .else_args
        .iter()
        .map(|a| else_renames.get(a).copied().unwrap_or(*a))
        .collect();

    let merge_params: Vec<(ArcVarId, ori_types::Idx)> =
        func.blocks[diamond.merge_idx].params.clone();
    let mut select_results = Vec::with_capacity(merge_params.len());

    for (i, (merge_param, ty)) in merge_params.iter().enumerate() {
        let then_val = resolved_then[i];
        let else_val = resolved_else[i];
        let dst = func.fresh_var_like_typed(*merge_param, *ty);

        if then_val == else_val {
            func.blocks[branch_idx].body.push(ArcInstr::Let {
                dst,
                ty: *ty,
                value: ArcValue::Var(then_val),
            });
        } else {
            func.blocks[branch_idx].body.push(ArcInstr::Select {
                dst,
                ty: *ty,
                cond: diamond.cond,
                true_val: then_val,
                false_val: else_val,
            });
        }
        func.spans[branch_idx].push(None);
        select_results.push(dst);
    }

    func.blocks[branch_idx].terminator = ArcTerminator::Jump {
        target: usize_to_block_id(diamond.merge_idx),
        args: select_results,
    };

    func.blocks[diamond.then_idx].terminator = ArcTerminator::Unreachable;
    func.blocks[diamond.else_idx].terminator = ArcTerminator::Unreachable;
}

/// Move an arm block's body into the branch block with fresh variable names.
///
/// Returns a rename map `old_dst → fresh_dst` for resolving jump args.
fn move_arm_body(
    func: &mut ArcFunction,
    branch_idx: usize,
    body: &[ArcInstr],
    spans: &[Option<ori_ir::Span>],
) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut renames = FxHashMap::default();

    for (i, instr) in body.iter().enumerate() {
        match instr {
            ArcInstr::Let { dst, ty, value } => {
                let fresh = func.fresh_var_like_typed(*dst, *ty);
                renames.insert(*dst, fresh);

                func.blocks[branch_idx].body.push(ArcInstr::Let {
                    dst: fresh,
                    ty: *ty,
                    value: value.clone(),
                });
                func.spans[branch_idx].push(spans[i]);
            }
            _ => {
                // is_trivial_body guarantees only Let instructions.
                unreachable!("non-Let instruction in trivial arm body");
            }
        }
    }

    renames
}
