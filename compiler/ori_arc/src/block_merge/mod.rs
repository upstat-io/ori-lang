//! Post-lowering ARC block merge pass.
//!
//! Eliminates redundant basic blocks created by the ARC lowerer's use of
//! `Invoke` terminators and trivial if/else diamond patterns. After all ARC
//! optimization passes run (RC insertion, edge cleanup, `expand_reuse`, RC
//! elimination), many `Invoke`s become trivial — their unwind blocks are
//! empty `Resume` stubs and their normal blocks are single-predecessor
//! continuations connected by unconditional branches. Similarly, simple
//! if/else expressions with trivial arm bodies can be folded into `Select`
//! instructions.
//!
//! # Four-Phase Transform
//!
//! 1. **Compact** — remove blocks unreachable from the entry (dead unwind
//!    blocks, orphaned blocks from earlier passes).
//! 2. **Downgrade** — convert trivial `Invoke`s to `Apply` + `Jump` when
//!    the unwind block is an empty `Resume` and the normal block has a
//!    single predecessor with no params.
//! 3. **Select-fold** — convert trivial if/else diamond patterns into
//!    `Select` instructions. A diamond is `Branch → then/else → merge+phi`
//!    where both arm blocks have empty or trivial bodies (only `Let`
//!    bindings of literals or pre-branch variables). After folding, a
//!    compaction sub-step (3b) removes the dead arm blocks.
//! 4. **Merge** — collapse `Jump`-chain blocks where the target has a single
//!    predecessor, merging the target's body into the source.
//!
//! # Pipeline Placement
//!
//! **Must run AFTER RC elimination but BEFORE [`compute_drop_hints`].**
//! Drop hints store `(block_idx, instr_idx)` coordinates that would become
//! invalid if blocks are renumbered after hint computation.
//!
//! [`compute_drop_hints`]: crate::uniqueness::compute_drop_hints

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::graph::{compute_pred_counts, successor_block_ids};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};
use crate::uniqueness::DropHints;

/// Run the full block merge pass on a function.
///
/// Calls the four phases in order: compact → downgrade → select-fold →
/// merge.
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

    // Phase 3a: fold trivial if/else diamonds into Select instructions.
    fold_select_diamonds(func);

    // Phase 3b: remove dead then/else blocks left by select folding.
    compact_blocks(func);

    // Phase 4: merge single-predecessor Jump chains (fixed point).
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

// ── Phase 3: Select-Fold Trivial If/Else Diamonds ───────────────────

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
///
/// For each `Branch { cond, then_block, else_block }` where both arm blocks
/// are trivial (empty or only `Let { Literal | Var }` bindings) and jump to
/// the same merge block, we:
/// 1. Move arm-local definitions into the branch block with fresh names
/// 2. Emit `Select` instructions for each merge-block parameter
/// 3. Replace the `Branch` with a `Jump` to the merge block
/// 4. Mark arm blocks as `Unreachable` (cleaned up by `compact_blocks`)
fn fold_select_diamonds(func: &mut ArcFunction) {
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

// ── Phase 4: Merge Jump Chains ──────────────────────────────────────

/// Merge single-predecessor Jump chains until fixed point (Phase 4).
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
