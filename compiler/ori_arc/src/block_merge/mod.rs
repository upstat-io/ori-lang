//! Post-lowering ARC block merge pass.
//!
//! Eliminates redundant basic blocks created by the ARC lowerer's use of
//! `Invoke` terminators. After all ARC optimization passes run (RC insertion,
//! edge cleanup, `expand_reuse`, RC elimination), many `Invoke`s become
//! trivial — their unwind blocks are empty `Resume` stubs and their normal
//! blocks are single-predecessor continuations connected by unconditional
//! branches.
//!
//! # Three-Phase Transform
//!
//! 1. **Compact** — remove blocks unreachable from the entry (dead unwind
//!    blocks, orphaned blocks from earlier passes).
//! 2. **Downgrade** — convert trivial `Invoke`s to `Apply` + `Jump` when
//!    the unwind block is an empty `Resume` and the normal block has a
//!    single predecessor with no params.
//! 3. **Merge** — collapse `Jump`-chain blocks where the target has a single
//!    predecessor, merging the target's body into the source.
//!
//! # Pipeline Placement
//!
//! **Must run AFTER RC elimination but BEFORE [`compute_drop_hints`].**
//! Drop hints store `(block_idx, instr_idx)` coordinates that would become
//! invalid if blocks are renumbered after hint computation.
//!
//! [`compute_drop_hints`]: crate::uniqueness::compute_drop_hints

use rustc_hash::FxHashSet;

use crate::graph::{compute_pred_counts, successor_block_ids};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};
use crate::uniqueness::DropHints;

/// Run the full block merge pass on a function.
///
/// Calls the three phases in order: compact → downgrade → merge.
///
/// # Precondition
///
/// Drop hints must not have been computed yet — they use `(block_idx,
/// instr_idx)` coordinates that merge invalidates. This function
/// defensively clears `func.drop_hints` at entry.
pub(crate) fn merge_blocks(func: &mut ArcFunction) {
    // Defensive: clear stale drop hints (they'll be recomputed after us).
    func.drop_hints = DropHints::default();

    // Phase 1: remove unreachable blocks so predecessor counts are accurate.
    compact_blocks(func);

    // Phase 2: downgrade trivial invokes to Apply + Jump.
    downgrade_trivial_invokes(func);

    // Phase 3: merge single-predecessor Jump chains (fixed point).
    merge_jump_chains(func);
}

// ── Phase 1: Compact Unreachable Blocks ─────────────────────────────

/// Remove blocks unreachable from the entry block.
///
/// Computes reachability via DFS, builds an old→new block ID remap for
/// surviving blocks, filters out dead blocks, and rewrites all block
/// references in surviving terminators.
///
/// Also remaps `cow_annotations` block indices and drops annotations
/// for dead blocks.
fn compact_blocks(func: &mut ArcFunction) {
    let num_blocks = func.blocks.len();
    if num_blocks == 0 {
        return;
    }

    // DFS reachability from entry.
    let mut reachable = vec![false; num_blocks];
    let mut stack = vec![func.entry.index()];
    while let Some(idx) = stack.pop() {
        if idx >= num_blocks || reachable[idx] {
            continue;
        }
        reachable[idx] = true;
        for succ in successor_block_ids(&func.blocks[idx].terminator) {
            let si = succ.index();
            if si < num_blocks && !reachable[si] {
                stack.push(si);
            }
        }
    }

    // Check if all blocks are reachable — early exit.
    if reachable.iter().all(|&r| r) {
        return;
    }

    // Build remap: old index → Some(new index) for reachable, None for dead.
    let mut remap: Vec<Option<usize>> = vec![None; num_blocks];
    let mut counter = 0usize;
    for (i, &is_reachable) in reachable.iter().enumerate() {
        if is_reachable {
            remap[i] = Some(counter);
            counter += 1;
        }
    }

    // Filter to reachable blocks, assigning new sequential IDs.
    // We drain blocks/spans to avoid needing Default on ArcBlock.
    let old_blocks: Vec<_> = func.blocks.drain(..).collect();
    let old_spans: Vec<_> = func.spans.drain(..).collect();
    let mut new_blocks = Vec::with_capacity(counter);
    let mut new_spans = Vec::with_capacity(counter);
    for (i, (mut block, spans)) in old_blocks.into_iter().zip(old_spans).enumerate() {
        if reachable[i] {
            block.id = remap_to_block_id(remap[i]);
            new_blocks.push(block);
            new_spans.push(spans);
        }
    }

    // Rewrite targets in surviving blocks.
    for block in &mut new_blocks {
        remap_terminator_targets(&mut block.terminator, &remap);
    }

    func.blocks = new_blocks;
    func.spans = new_spans;
    func.entry = remap_to_block_id(remap[func.entry.index()]);
    func.cow_annotations.remap_block_indices(&remap);
}

/// Convert a `usize` block index to an `ArcBlockId`.
///
/// # Panics
///
/// Panics if `idx` exceeds `u32::MAX`.
fn usize_to_block_id(idx: usize) -> ArcBlockId {
    let raw = u32::try_from(idx).unwrap_or_else(|_| panic!("block index {idx} exceeds u32::MAX"));
    ArcBlockId::new(raw)
}

/// Convert a remap entry to an `ArcBlockId`.
///
/// # Panics
///
/// Panics if the entry is `None` (unreachable block used where
/// reachable was expected) or exceeds `u32::MAX`.
fn remap_to_block_id(entry: Option<usize>) -> ArcBlockId {
    let idx = entry.unwrap_or_else(|| panic!("block remap entry is None for a required block"));
    usize_to_block_id(idx)
}

/// Rewrite all `ArcBlockId` references in a terminator using a remap table.
fn remap_terminator_targets(term: &mut ArcTerminator, remap: &[Option<usize>]) {
    fn remap_id(id: &mut ArcBlockId, remap: &[Option<usize>]) {
        *id = remap_to_block_id(remap[id.index()]);
    }

    match term {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
        ArcTerminator::Jump { target, .. } => remap_id(target, remap),
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            remap_id(then_block, remap);
            remap_id(else_block, remap);
        }
        ArcTerminator::Switch { cases, default, .. } => {
            for (_, target) in cases {
                remap_id(target, remap);
            }
            remap_id(default, remap);
        }
        ArcTerminator::Invoke { normal, unwind, .. } => {
            remap_id(normal, remap);
            remap_id(unwind, remap);
        }
    }
}

// ── Phase 2: Downgrade Trivial Invokes ──────────────────────────────

/// Convert trivial `Invoke` terminators to `Apply` + `Jump`.
///
/// An invoke is trivial when:
/// 1. `normal != unwind` (same block would route success to `Resume`)
/// 2. The unwind block is empty body + `Resume` terminator + no params
/// 3. The normal block has no params
/// 4. The normal block has exactly one predecessor (the invoking block)
///
/// The `Invoke { dst, ty, func, args, arg_ownership, normal, unwind }`
/// becomes an `Apply { dst, ty, func, args, arg_ownership }` appended to
/// the block body, with terminator replaced by `Jump { target: normal }`.
fn downgrade_trivial_invokes(func: &mut ArcFunction) {
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

// ── Phase 3: Merge Jump Chains ──────────────────────────────────────

/// Merge single-predecessor Jump chains until fixed point.
///
/// For each block A with terminator `Jump { target: B, args }` where:
/// - A != B (self-loop guard)
/// - B has exactly one predecessor (A)
/// - B is not the entry block
///
/// Lower B's params as Let bindings (parallel-copy semantics), then
/// merge B's body and spans into A.
///
/// Runs to fixed point for transitive chains (A → B → C all merge into A).
/// After fixed point, runs a final compaction to remove dead blocks.
fn merge_jump_chains(func: &mut ArcFunction) {
    let mut dead: FxHashSet<usize> = FxHashSet::default();

    loop {
        let mut changed = false;
        let pred_counts = compute_pred_counts(func);

        for a_idx in 0..func.blocks.len() {
            if dead.contains(&a_idx) {
                continue;
            }

            let (b_idx, jump_args) = {
                let ArcTerminator::Jump { target, args } = &func.blocks[a_idx].terminator else {
                    continue;
                };
                let b_idx = target.index();

                // Self-loop guard.
                if a_idx == b_idx {
                    continue;
                }
                // B must have exactly one predecessor.
                if pred_counts[b_idx] != 1 {
                    continue;
                }
                // B must not be the entry block.
                if b_idx == func.entry.index() {
                    continue;
                }
                // B must not already be dead.
                if dead.contains(&b_idx) {
                    continue;
                }

                (b_idx, args.clone())
            };

            let b_params = func.blocks[b_idx].params.clone();

            // Arity check: Jump args must match target block params.
            debug_assert_eq!(
                b_params.len(),
                jump_args.len(),
                "Jump args/params arity mismatch: block {a_idx} → block {b_idx}",
            );
            if b_params.len() != jump_args.len() {
                continue;
            }

            // Lower parallel-copy semantics: block params → Let bindings.
            lower_parallel_copy(func, a_idx, &b_params, &jump_args);

            // Remap COW annotations: B's entries → A's coordinates.
            let offset = func.blocks[a_idx].body.len();
            func.cow_annotations.remap_block_merge(b_idx, a_idx, offset);

            // Merge B's body into A.
            let b_body: Vec<ArcInstr> = func.blocks[b_idx].body.drain(..).collect();
            func.blocks[a_idx].body.extend(b_body);

            // Merge B's spans into A.
            let b_spans: Vec<Option<ori_ir::Span>> = func.spans[b_idx].drain(..).collect();
            func.spans[a_idx].extend(b_spans);

            // Replace A's terminator with B's.
            let b_term = std::mem::replace(
                &mut func.blocks[b_idx].terminator,
                ArcTerminator::Unreachable,
            );
            func.blocks[a_idx].terminator = b_term;

            // Mark B as dead.
            dead.insert(b_idx);
            changed = true;
        }

        if !changed {
            break;
        }
    }

    // Final compaction: remove dead blocks.
    if !dead.is_empty() {
        compact_blocks(func);
    }
}

/// Lower block-param parallel-copy semantics to sequential Let bindings.
///
/// Jump args are parallel phi inputs — all args are read before any param
/// is written. When no arg aliases a target param, direct Let is safe.
/// When overlap exists (e.g., swap: `Jump { args: [p1, p0] }` → params
/// `[p0, p1]`), we use fresh temps to avoid clobbering.
fn lower_parallel_copy(
    func: &mut ArcFunction,
    block_idx: usize,
    params: &[(ArcVarId, ori_types::Idx)],
    args: &[ArcVarId],
) {
    if params.is_empty() {
        return;
    }

    // Check for overlap: does any arg alias a target param?
    let param_vars: FxHashSet<ArcVarId> = params.iter().map(|(v, _)| *v).collect();
    let has_overlap = args.iter().any(|a| param_vars.contains(a));

    if has_overlap {
        // Slow path: copy all args to fresh temps first, then temps to params.
        // Use fresh_var_repr to preserve repr metadata for ref-typed params.
        let temps: Vec<ArcVarId> = args
            .iter()
            .zip(params.iter())
            .map(|(arg, (_, ty))| {
                let repr = func
                    .var_reprs
                    .get(arg.index())
                    .copied()
                    .unwrap_or(ValueRepr::Scalar);
                func.fresh_var_repr(*ty, repr)
            })
            .collect();

        // Phase 1: args → temps.
        for ((&arg, temp), (_, ty)) in args.iter().zip(temps.iter()).zip(params.iter()) {
            func.blocks[block_idx].body.push(ArcInstr::Let {
                dst: *temp,
                ty: *ty,
                value: ArcValue::Var(arg),
            });
            func.spans[block_idx].push(None);
        }

        // Phase 2: temps → params.
        for ((param_var, param_ty), temp) in params.iter().zip(temps.iter()) {
            func.blocks[block_idx].body.push(ArcInstr::Let {
                dst: *param_var,
                ty: *param_ty,
                value: ArcValue::Var(*temp),
            });
            func.spans[block_idx].push(None);
        }
    } else {
        // Fast path: no aliasing, direct Let is safe.
        for ((param_var, param_ty), &arg) in params.iter().zip(args.iter()) {
            func.blocks[block_idx].body.push(ArcInstr::Let {
                dst: *param_var,
                ty: *param_ty,
                value: ArcValue::Var(arg),
            });
            func.spans[block_idx].push(None);
        }
    }
}

#[cfg(test)]
mod tests;
