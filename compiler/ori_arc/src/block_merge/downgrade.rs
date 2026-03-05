//! Phase 2: Downgrade trivial `Invoke` terminators to `Apply` + `Jump`.
//!
//! An invoke is trivial when:
//! 1. `normal != unwind` (same block would route success to `Resume`)
//! 2. The unwind block is empty body + `Resume` terminator + no params
//! 3. The normal block has no params
//! 4. The normal block has exactly one predecessor (the invoking block)
//!
//! The `Invoke { dst, ty, func, args, arg_ownership, normal, unwind }`
//! becomes an `Apply { dst, ty, func, args, arg_ownership }` appended to
//! the block body, with terminator replaced by `Jump { target: normal }`.

use crate::graph::compute_pred_counts;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator};

use super::usize_to_block_id;

/// Convert trivial `Invoke` terminators to `Apply` + `Jump`.
#[tracing::instrument(skip_all, name = "phase2_downgrade")]
pub(crate) fn downgrade_trivial_invokes(func: &mut ArcFunction) {
    let pred_counts = compute_pred_counts(func);

    for block_idx in 0..func.blocks.len() {
        // Check if this block has a trivial invoke — extract normal_idx
        // and apply fields if so.
        let Some(normal_idx) = is_trivial_invoke(func, block_idx, &pred_counts) else {
            continue;
        };

        // Extract invoke fields. We know the terminator is Invoke from
        // the check above.
        let (dst, ty, callee, args, arg_ownership) = {
            let ArcTerminator::Invoke {
                dst,
                ty,
                func: callee,
                args,
                arg_ownership,
                ..
            } = &func.blocks[block_idx].terminator
            else {
                continue;
            };
            (*dst, *ty, *callee, args.clone(), arg_ownership.clone())
        };

        // Append Apply to body.
        func.blocks[block_idx].body.push(ArcInstr::Apply {
            dst,
            ty,
            func: callee,
            args,
            arg_ownership,
        });

        // Append None span for the new Apply.
        func.spans[block_idx].push(None);

        // Replace terminator with Jump.
        func.blocks[block_idx].terminator = ArcTerminator::Jump {
            target: usize_to_block_id(normal_idx),
            args: vec![],
        };
    }
}

/// Check if a block's `Invoke` terminator is trivial and return the
/// normal successor index if so.
///
/// Returns `None` if the block doesn't have an `Invoke`, or if any of
/// the four criteria for trivial invoke downgrade are not met.
fn is_trivial_invoke(func: &ArcFunction, block_idx: usize, pred_counts: &[usize]) -> Option<usize> {
    let ArcTerminator::Invoke { normal, unwind, .. } = &func.blocks[block_idx].terminator else {
        return None;
    };

    // Criterion 1: normal != unwind.
    if normal == unwind {
        return None;
    }

    let normal_idx = normal.index();
    let unwind_idx = unwind.index();

    // Criterion 2: unwind block is trivial (empty + Resume + no params).
    let ub = &func.blocks[unwind_idx];
    if !ub.body.is_empty() || ub.terminator != ArcTerminator::Resume || !ub.params.is_empty() {
        return None;
    }

    // Criterion 3: normal block has no params.
    if !func.blocks[normal_idx].params.is_empty() {
        return None;
    }

    // Criterion 4: normal block has exactly one predecessor.
    if pred_counts[normal_idx] != 1 {
        return None;
    }

    Some(normal_idx)
}
