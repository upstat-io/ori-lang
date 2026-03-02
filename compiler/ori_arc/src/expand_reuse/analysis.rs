//! Pre-expansion analysis and instruction manipulation.
//!
//! Provides helpers for the expand-reuse pass:
//! - Building projection maps (field projections from a base variable).
//! - Identifying and erasing redundant RC increments on projected fields.
//! - Reordering "between" instructions so Reset and Reuse are adjacent.
//! - Propagating merge-block substitutions to pre-existing blocks.

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

use super::{ClaimedFields, ProjMap};

// Projection map

/// Build a map from `(base_var, field_index)` → `projected_var` for all
/// `Project` instructions in `instrs` that project from `base`.
pub(super) fn build_proj_map(instrs: &[ArcInstr], base: ArcVarId) -> ProjMap {
    let mut map = ProjMap::default();
    for instr in instrs {
        if let ArcInstr::Project {
            dst, value, field, ..
        } = instr
        {
            if *value == base {
                map.insert((base, *field), *dst);
            }
        }
    }
    map
}

// Projection-increment erasure (§09.4)

/// Scan backwards for `Project`/`RcInc` patterns and identify which
/// increments can be erased.
///
/// Returns:
/// - Indices of `RcInc` instructions to erase (sorted ascending).
/// - Map of claimed fields (`field_index` → `projected_var`).
pub(super) fn erase_proj_increments(
    instrs: &[ArcInstr],
    proj_map: &ProjMap,
) -> (Vec<usize>, ClaimedFields) {
    let mut erased = Vec::new();
    let mut claimed = ClaimedFields::default();

    // For each projected field, scan for a matching RcInc.
    for (&(_base, field), &proj_var) in proj_map {
        // Find the RcInc for this projected variable (scan backwards).
        for (idx, instr) in instrs.iter().enumerate().rev() {
            if let ArcInstr::RcInc { var, count, .. } = instr {
                if *var == proj_var {
                    if *count == 1 {
                        // Erase entirely.
                        erased.push(idx);
                    }
                    // If count > 1, we'd reduce by 1 — but for 2026,
                    // single-count incs are the common case. Multi-count
                    // would need an edit-in-place, handled in a future pass.
                    claimed.insert(field, proj_var);
                    break;
                }
            }
        }
    }

    erased.sort_unstable();
    (erased, claimed)
}

// Between-instruction reordering

/// Move instructions between `Reset` and `Reuse` to before the `Reset`.
///
/// These instructions don't use the reset variable (guaranteed by the
/// detection constraint in Section 07.6), so reordering is safe.
pub(super) fn move_between_to_prefix(
    func: &mut ArcFunction,
    block_idx: usize,
    reset_idx: usize,
    reuse_idx: usize,
) {
    if reuse_idx <= reset_idx + 1 {
        return; // Nothing between Reset and Reuse.
    }

    let body = &mut func.blocks[block_idx].body;
    let spans = &mut func.spans[block_idx];

    // Collect between instructions and their spans.
    let between_start = reset_idx + 1;
    let between_end = reuse_idx;
    let between_instrs: Vec<ArcInstr> = body[between_start..between_end].to_vec();
    let between_spans: Vec<Option<ori_ir::Span>> = if between_end <= spans.len() {
        spans[between_start..between_end.min(spans.len())].to_vec()
    } else {
        vec![None; between_instrs.len()]
    };

    // Remove between instructions (in reverse to preserve indices).
    for idx in (between_start..between_end).rev() {
        body.remove(idx);
        if idx < spans.len() {
            spans.remove(idx);
        }
    }

    // Insert before the Reset (which is now at reset_idx - removed count
    // but we removed AFTER it, so reset_idx is unchanged).
    // Actually, we removed indices > reset_idx, so reset_idx is still correct.
    let insert_pos = reset_idx;
    for (i, instr) in between_instrs.into_iter().enumerate() {
        body.insert(insert_pos + i, instr);
    }
    for (i, span) in between_spans.into_iter().enumerate() {
        if insert_pos + i <= spans.len() {
            spans.insert(insert_pos + i, span);
        }
    }
}

// Erasure and substitution helpers

/// Remove erased instructions (and their spans) from a block.
///
/// Indices are removed in reverse order to preserve earlier indices.
pub(super) fn apply_erasures(func: &mut ArcFunction, block_idx: usize, erased_indices: &[usize]) {
    let body = &mut func.blocks[block_idx].body;
    for &idx in erased_indices.iter().rev() {
        body.remove(idx);
    }
    let spans = &mut func.spans[block_idx];
    for &idx in erased_indices.iter().rev() {
        if idx < spans.len() {
            spans.remove(idx);
        }
    }
}

/// Substitute `reuse_dst → merge_param` in all pre-existing blocks.
///
/// The merge block itself already uses `merge_param`, but successor blocks
/// reachable from the merge block's terminator (e.g., Invoke destinations)
/// still reference `reuse_dst`. This ensures consistency.
pub(super) fn propagate_merge_substitution(
    func: &mut ArcFunction,
    merge_param: ArcVarId,
    reuse_dst: ArcVarId,
) {
    for block in &mut func.blocks {
        for instr in &mut block.body {
            instr.substitute_var(reuse_dst, merge_param);
        }
        block.terminator.substitute_var(reuse_dst, merge_param);
    }
}
