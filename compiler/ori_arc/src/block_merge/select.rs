//! Phase 3: Fold trivial if/else diamond patterns into `Select` instructions.
//!
//! For each `Branch { cond, then_block, else_block }` where both arm blocks
//! are trivial (empty or only `Let { Literal | Var }` bindings) and jump to
//! the same merge block, we:
//! 1. Move arm-local definitions into the branch block with fresh names
//! 2. Emit `Select` instructions for each merge-block parameter
//! 3. Replace the `Branch` with a `Jump` to the merge block
//! 4. Mark arm blocks as `Unreachable` (cleaned up by `compact_blocks`)

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::graph::compute_pred_counts;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::usize_to_block_id;

/// Information extracted from a select-eligible diamond pattern.
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
pub(crate) fn fold_select_diamonds(func: &mut ArcFunction) {
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
pub(crate) fn is_trivial_body(body: &[ArcInstr]) -> bool {
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

    // Criterion 2: arm blocks have no params.
    if !func.blocks[then_idx].params.is_empty() {
        tracing::trace!(block = block_idx, "select: then block has params");
        return None;
    }
    if !func.blocks[else_idx].params.is_empty() {
        tracing::trace!(block = block_idx, "select: else block has params");
        return None;
    }

    // Criterion 3: both arms have exactly 1 predecessor.
    if pred_counts[then_idx] != 1 {
        tracing::trace!(
            block = block_idx,
            then_preds = pred_counts[then_idx],
            "select: then block has multiple predecessors"
        );
        return None;
    }
    if pred_counts[else_idx] != 1 {
        tracing::trace!(
            block = block_idx,
            else_preds = pred_counts[else_idx],
            "select: else block has multiple predecessors"
        );
        return None;
    }

    // Criterion 4: both bodies are trivial.
    if !is_trivial_body(&func.blocks[then_idx].body) {
        tracing::trace!(block = block_idx, "select: then body is not trivial");
        return None;
    }
    if !is_trivial_body(&func.blocks[else_idx].body) {
        tracing::trace!(block = block_idx, "select: else body is not trivial");
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
/// Moves arm-local definitions into the branch block with fresh names,
/// emits `Select` instructions for each merge parameter, replaces the
/// `Branch` with a `Jump`, and marks arm blocks as `Unreachable`.
fn apply_select_fold(func: &mut ArcFunction, branch_idx: usize, diamond: &SelectDiamond) {
    // Step 1: Drain arm bodies and spans into local Vecs.
    // This avoids borrow conflicts when mutating the branch block.
    let then_body: Vec<ArcInstr> = func.blocks[diamond.then_idx].body.drain(..).collect();
    let then_spans: Vec<_> = func.spans[diamond.then_idx].drain(..).collect();
    let else_body: Vec<ArcInstr> = func.blocks[diamond.else_idx].body.drain(..).collect();
    let else_spans: Vec<_> = func.spans[diamond.else_idx].drain(..).collect();

    // Step 2: Fresh-rename arm-local definitions into the branch block.
    let then_renames = move_arm_body(func, branch_idx, &then_body, &then_spans);
    let else_renames = move_arm_body(func, branch_idx, &else_body, &else_spans);

    // Step 3: Resolve jump args through rename maps.
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

    // Step 4: Emit Select (or Var passthrough) for each merge param.
    let merge_params: Vec<(ArcVarId, ori_types::Idx)> =
        func.blocks[diamond.merge_idx].params.clone();
    let mut select_results = Vec::with_capacity(merge_params.len());

    for (i, (merge_param, ty)) in merge_params.iter().enumerate() {
        let then_val = resolved_then[i];
        let else_val = resolved_else[i];
        let repr = func.var_repr(*merge_param).unwrap_or(ValueRepr::Scalar);
        let dst = func.fresh_var_repr(*ty, repr);

        if then_val == else_val {
            // Both arms produce the same value — no need for select.
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

    // Step 5: Replace Branch with Jump to merge block.
    func.blocks[branch_idx].terminator = ArcTerminator::Jump {
        target: usize_to_block_id(diamond.merge_idx),
        args: select_results,
    };

    // Step 6: Mark arm blocks as unreachable (bodies already drained).
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
                let repr = func.var_repr(*dst).unwrap_or(ValueRepr::Scalar);
                let fresh = func.fresh_var_repr(*ty, repr);
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
                debug_assert!(false, "non-Let instruction in trivial arm body");
            }
        }
    }

    renames
}
