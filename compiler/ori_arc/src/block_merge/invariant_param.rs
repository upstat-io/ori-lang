//! Phase 7: Invariant block param elimination.
//!
//! After Phase 6 removes dead (unused) params, multi-predecessor blocks
//! may still have params whose incoming values all agree — either every
//! predecessor passes the same `ArcVarId`, or the only variation is the
//! param referencing itself (a back-edge in a loop).
//!
//! A block param `(param_var, ty)` on block B is **invariant** if, after
//! removing self-references (`param_var` passed back to itself on
//! back-edges), all remaining incoming values are the same `ArcVarId`.
//! The param can be replaced by that single incoming value everywhere.
//!
//! This eliminates loop-invariant phi nodes at the LLVM level — the most
//! common case is a mutable binding defined before a loop, used inside
//! the loop body, but never modified.
//!
//! # Algorithm
//!
//! For each block B with params:
//!
//! 1. Collect all `Jump { target: B, args }` terminators (predecessors).
//! 2. For each param position `p` with var `param_var`, gather `args[p]`
//!    from every predecessor, filter out self-references (back-edge passes
//!    `param_var` to itself), and check if exactly one unique non-self
//!    value `V` remains — if so, the param is invariant.
//! 3. Replace all uses of each invariant `param_var` with `V` across
//!    the entire function (instructions and terminators).
//! 4. Remove invariant params from the block and corresponding args
//!    from predecessor Jumps.

use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::graph;
use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};

/// Eliminate block params that are loop-invariant.
///
/// A param is invariant when all non-self incoming values agree on a
/// single `ArcVarId`. Replaces uses of the param with the common
/// value, then removes the param and its corresponding Jump args.
#[tracing::instrument(skip_all, name = "phase7_invariant_param")]
pub(crate) fn eliminate_invariant_params(func: &mut ArcFunction) {
    // Collect (block_index, param_position, replacement_var) triples.
    let replacements = find_invariant_params(func);

    if replacements.is_empty() {
        return;
    }

    // Apply substitutions: replace all uses of param_var with the
    // common incoming value, across the entire function.
    for &(_, _, param_var, replacement) in &replacements {
        substitute_everywhere(func, param_var, replacement);
    }

    // Remove invariant params and their corresponding Jump args.
    // Process blocks in order, but within each block remove positions
    // in reverse to preserve indices.
    remove_invariant_params(func, &replacements);
}

/// Scan for invariant block params across the function.
///
/// Returns `(block_idx, param_position, param_var, replacement_var)`.
fn find_invariant_params(func: &ArcFunction) -> SmallVec<[(usize, usize, ArcVarId, ArcVarId); 4]> {
    let mut results = SmallVec::new();
    let predecessors = graph::compute_predecessors(func);

    for (b_idx, block) in func.blocks.iter().enumerate() {
        if block.params.is_empty() {
            continue;
        }

        // Collect Jump args from pre-computed predecessors.
        let incoming = collect_incoming_args_from_preds(func, block.id, &predecessors);

        if incoming.is_empty() {
            continue;
        }

        for (p_idx, &(param_var, _ty)) in block.params.iter().enumerate() {
            // Gather the arg at position p_idx from each predecessor.
            let mut non_self_values: FxHashSet<ArcVarId> = FxHashSet::default();

            for pred_args in &incoming {
                if p_idx >= pred_args.len() {
                    // Mismatched arg count — skip this param defensively.
                    non_self_values.clear();
                    break;
                }
                let arg = pred_args[p_idx];
                // Filter out self-references (back-edge passes param to itself).
                if arg != param_var {
                    non_self_values.insert(arg);
                }
            }

            // Invariant: exactly one unique non-self value.
            if non_self_values.len() == 1 {
                // Invariant: len == 1 ensures iter().next() is Some.
                let Some(&replacement) = non_self_values.iter().next() else {
                    unreachable!("len == 1 but iter empty");
                };
                tracing::debug!(
                    block = b_idx,
                    param_pos = p_idx,
                    param_var = param_var.raw(),
                    replacement = replacement.raw(),
                    "invariant block param detected"
                );
                results.push((b_idx, p_idx, param_var, replacement));
            }
        }
    }

    results
}

/// Collect incoming Jump args for a target block using pre-computed predecessors.
///
/// Returns a vec of arg slices — one per predecessor Jump targeting `target_id`.
fn collect_incoming_args_from_preds<'a>(
    func: &'a ArcFunction,
    target_id: ArcBlockId,
    predecessors: &[Vec<usize>],
) -> Vec<&'a [ArcVarId]> {
    let target_idx = target_id.index();
    let Some(preds) = predecessors.get(target_idx) else {
        return Vec::new();
    };
    let mut incoming = Vec::with_capacity(preds.len());
    for &pred_idx in preds {
        if let ArcTerminator::Jump { target, args } = &func.blocks[pred_idx].terminator {
            if *target == target_id {
                incoming.push(args.as_slice());
            }
        }
    }
    incoming
}

/// Replace all uses of `old` with `new` in instruction bodies and
/// terminators across the entire function.
fn substitute_everywhere(func: &mut ArcFunction, old: ArcVarId, new: ArcVarId) {
    for block in &mut func.blocks {
        for instr in &mut block.body {
            instr.substitute_var(old, new);
        }
        block.terminator.substitute_var(old, new);
    }
}

/// Remove invariant params from blocks and corresponding args from Jumps.
fn remove_invariant_params(
    func: &mut ArcFunction,
    replacements: &[(usize, usize, ArcVarId, ArcVarId)],
) {
    // Group removals by block index. Within each block, collect
    // param positions to remove.
    let mut block_removals: Vec<(usize, SmallVec<[usize; 4]>)> = Vec::new();
    let mut current_block: Option<usize> = None;

    for &(b_idx, p_idx, _, _) in replacements {
        match current_block {
            Some(cb) if cb == b_idx => {
                // current_block is Some iff we pushed at least once.
                if let Some(last) = block_removals.last_mut() {
                    last.1.push(p_idx);
                }
            }
            _ => {
                block_removals.push((b_idx, SmallVec::from_elem(p_idx, 1)));
                current_block = Some(b_idx);
            }
        }
    }

    for (b_idx, positions) in &block_removals {
        let block_id = func.blocks[*b_idx].id;

        // Remove params in reverse order to preserve indices.
        for &p in positions.iter().rev() {
            func.blocks[*b_idx].params.remove(p);
        }

        // Remove corresponding args from all predecessor Jumps.
        for block in &mut func.blocks {
            if let ArcTerminator::Jump { target, args } = &mut block.terminator {
                if *target == block_id && !args.is_empty() {
                    for &p in positions.iter().rev() {
                        if p < args.len() {
                            args.remove(p);
                        }
                    }
                }
            }
        }
    }
}
