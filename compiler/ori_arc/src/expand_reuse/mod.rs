//! Constructor Reuse Expansion (Section 09).
//!
//! Expands `Reset`/`Reuse` intermediate instructions (inserted by Section 07.6)
//! into conditional two-path code:
//!
//! - **`IsShared` check**: tests whether the value's refcount > 1.
//! - **Fast path** (unique, refcount == 1): in-place field mutation via `Set`.
//! - **Slow path** (shared, refcount > 1): `RcDec` + fresh `Construct`.
//!
//! After this pass, no `Reset` or `Reuse` instructions remain in the ARC IR.
//!
//! # Sub-optimizations
//!
//! - **Projection-Increment Erasure** (§09.4): erases redundant `RcInc` ops
//!   for projected fields. On the fast path, we exclusively own the parent, so
//!   projected fields are implicitly owned. On the slow path, the erased incs
//!   are restored.
//!
//! - **Self-Set Elimination** (§09.5): skips `Set` instructions that write a
//!   field back to its original projected position (a no-op).
//!
//! # References
//!
//! - Lean 4: `src/Lean/Compiler/IR/ExpandResetReuse.lean`
//! - Koka: Perceus paper §4 (reuse analysis)

use rustc_hash::FxHashMap;

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};
use crate::ArcClassification;

// Data structures

/// A matched `Reset`/`Reuse` pair within a single block.
struct ResetReusePair {
    /// Index of the `Reset` instruction in the block body.
    reset_idx: usize,
    /// Index of the `Reuse` instruction in the block body.
    reuse_idx: usize,
    /// The variable being tested for uniqueness (`Reset.var`).
    reset_var: ArcVarId,
    /// Destination of the `Reuse` instruction.
    reuse_dst: ArcVarId,
    /// Type of the constructed value.
    reuse_ty: ori_types::Idx,
    /// Constructor kind.
    reuse_ctor: CtorKind,
    /// Arguments for the constructor.
    reuse_args: Vec<ArcVarId>,
}

/// Maps `(base_var, field_index)` → `projected_var` for projections seen
/// before the `Reset`. Used for self-set elimination and projection-increment
/// erasure.
type ProjMap = FxHashMap<(ArcVarId, u32), ArcVarId>;

/// Fields whose `RcInc` was erased by projection-increment erasure.
/// Maps `field_index` → `projected_var`.
type ClaimedFields = FxHashMap<u32, ArcVarId>;

// Public API

/// Expand all `Reset`/`Reuse` pairs into `IsShared` + conditional fast/slow paths.
///
/// After this pass completes, no `Reset` or `Reuse` instructions remain.
/// Each pair is replaced by:
/// 1. An `IsShared` check on the reset variable.
/// 2. A `Branch` to slow (shared) or fast (unique) path.
/// 3. Fast path: in-place `Set` mutations (with self-set elimination).
/// 4. Slow path: `RcDec` + fresh `Construct`.
///
/// Both paths merge via a continuation block if there are instructions after
/// the `Reuse`.
pub fn expand_reset_reuse(func: &mut ArcFunction, classifier: &dyn ArcClassification) {
    // Only process original blocks — newly added blocks are already expanded.
    let original_block_count = func.blocks.len();

    for block_idx in 0..original_block_count {
        try_expand_block(func, block_idx, classifier);
    }

    tracing::debug!(
        function = func.name.raw(),
        blocks_before = original_block_count,
        blocks_after = func.blocks.len(),
        "constructor reuse expansion complete"
    );
}

// Block expansion

/// Attempt to expand a single block's `Reset`/`Reuse` pair.
fn try_expand_block(func: &mut ArcFunction, block_idx: usize, classifier: &dyn ArcClassification) {
    let Some(pair) = find_reset_reuse_pair(&func.blocks[block_idx]) else {
        return;
    };

    tracing::debug!(
        block = block_idx,
        reset_var = pair.reset_var.raw(),
        reuse_dst = pair.reuse_dst.raw(),
        "expanding Reset/Reuse pair"
    );

    // 1. Build projection map from instructions before the Reset.
    let proj_map = build_proj_map(
        &func.blocks[block_idx].body[..pair.reset_idx],
        pair.reset_var,
    );

    // 2. Projection-increment erasure (§09.4): erase RcInc ops for projected
    //    fields, building a claimed-fields mask.
    let (erased_indices, claimed) =
        erase_proj_increments(&func.blocks[block_idx].body[..pair.reset_idx], &proj_map);

    // Apply erasures to the actual block body.
    // Remove in reverse order to preserve indices.
    {
        let body = &mut func.blocks[block_idx].body;
        for &idx in erased_indices.iter().rev() {
            body.remove(idx);
        }
        // Update the span list to match.
        let spans = &mut func.spans[block_idx];
        for &idx in erased_indices.iter().rev() {
            if idx < spans.len() {
                spans.remove(idx);
            }
        }
    }

    // Re-find the pair (indices shifted due to erasures).
    let Some(pair) = find_reset_reuse_pair(&func.blocks[block_idx]) else {
        debug_assert!(false, "pair should still exist after erasure");
        return;
    };

    // 3. Move "between" instructions (Reset..Reuse exclusive) to before
    //    the Reset. They don't use the reset_var (constraint from detection),
    //    so reordering is safe.
    move_between_to_prefix(func, block_idx, &pair);

    // Re-find pair again (indices shifted).
    let Some(pair) = find_reset_reuse_pair(&func.blocks[block_idx]) else {
        debug_assert!(false, "pair should still exist after reorder");
        return;
    };

    // At this point, Reset is immediately followed by Reuse (no between instrs).
    debug_assert_eq!(
        pair.reuse_idx,
        pair.reset_idx + 1,
        "Reset and Reuse should be adjacent after reordering"
    );

    // 4. Determine block structure.
    let suffix = func.blocks[block_idx].body[pair.reuse_idx + 1..].to_vec();
    let original_terminator = func.blocks[block_idx].terminator.clone();
    let has_suffix = !suffix.is_empty();
    let terminator_uses_dst = original_terminator.uses_var(pair.reuse_dst);
    let needs_merge = has_suffix || terminator_uses_dst;

    // 5. Allocate new block IDs.
    let fast_id = func.next_block_id();
    let slow_id = ArcBlockId::new(fast_id.raw() + 1);
    let merge_id = if needs_merge {
        Some(ArcBlockId::new(slow_id.raw() + 1))
    } else {
        None
    };

    let ctx = ExpansionContext {
        pair: &pair,
        proj_map: &proj_map,
        claimed: &claimed,
        original_terminator: &original_terminator,
        merge_id,
    };

    // 6. Build fast-path block (§09.3 + §09.5 self-set elimination).
    let fast_block = build_fast_path(func, fast_id, &ctx, classifier);

    // 7. Build slow-path block (§09.3).
    let slow_block = build_slow_path(slow_id, &ctx, &suffix);

    // 8. Build merge block if needed.
    let merge_block = merge_id.map(|mid| {
        build_merge_block(
            func,
            mid,
            pair.reuse_dst,
            pair.reuse_ty,
            &suffix,
            &original_terminator,
        )
    });

    // 9. Create IsShared variable and truncate original block.
    let is_shared_var = func.fresh_var(ori_types::Idx::BOOL);
    let body = &mut func.blocks[block_idx].body;
    body.truncate(pair.reset_idx);
    body.push(ArcInstr::IsShared {
        dst: is_shared_var,
        var: pair.reset_var,
    });
    func.blocks[block_idx].terminator = ArcTerminator::Branch {
        cond: is_shared_var,
        then_block: slow_id, // shared → slow path
        else_block: fast_id, // unique → fast path (fall-through)
    };
    // Update spans for truncated block.
    func.spans[block_idx].truncate(pair.reset_idx);
    func.spans[block_idx].push(None); // IsShared span

    // 10. Push new blocks.
    func.push_block(fast_block);
    func.push_block(slow_block);
    if let Some(mb) = merge_block {
        func.push_block(mb);
    }
}

// Pair detection

/// Find the first `Reset`/`Reuse` pair in a block.
fn find_reset_reuse_pair(block: &ArcBlock) -> Option<ResetReusePair> {
    for (i, instr) in block.body.iter().enumerate() {
        if let ArcInstr::Reset { var, token } = instr {
            let reset_var = *var;
            let token_var = *token;

            // Find matching Reuse with same token.
            for (j, candidate) in block.body.iter().enumerate().skip(i + 1) {
                if let ArcInstr::Reuse {
                    token: t,
                    dst,
                    ty,
                    ctor,
                    args,
                } = candidate
                {
                    if *t == token_var {
                        return Some(ResetReusePair {
                            reset_idx: i,
                            reuse_idx: j,
                            reset_var,
                            reuse_dst: *dst,
                            reuse_ty: *ty,
                            reuse_ctor: *ctor,
                            reuse_args: args.clone(),
                        });
                    }
                }
            }
        }
    }
    None
}

// Projection map

/// Build a map from `(base_var, field_index)` → `projected_var` for all
/// `Project` instructions in `instrs` that project from `base`.
fn build_proj_map(instrs: &[ArcInstr], base: ArcVarId) -> ProjMap {
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
fn erase_proj_increments(instrs: &[ArcInstr], proj_map: &ProjMap) -> (Vec<usize>, ClaimedFields) {
    let mut erased = Vec::new();
    let mut claimed = ClaimedFields::default();

    // For each projected field, scan for a matching RcInc.
    for (&(_base, field), &proj_var) in proj_map {
        // Find the RcInc for this projected variable (scan backwards).
        for (idx, instr) in instrs.iter().enumerate().rev() {
            if let ArcInstr::RcInc { var, count } = instr {
                if *var == proj_var {
                    if *count == 1 {
                        // Erase entirely.
                        erased.push(idx);
                    }
                    // If count > 1, we'd reduce by 1 — but for 0.1-alpha,
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
fn move_between_to_prefix(func: &mut ArcFunction, block_idx: usize, pair: &ResetReusePair) {
    if pair.reuse_idx <= pair.reset_idx + 1 {
        return; // Nothing between Reset and Reuse.
    }

    let body = &mut func.blocks[block_idx].body;
    let spans = &mut func.spans[block_idx];

    // Collect between instructions and their spans.
    let between_start = pair.reset_idx + 1;
    let between_end = pair.reuse_idx;
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

    // Insert before the Reset (which is now at pair.reset_idx - removed count
    // but we removed AFTER it, so reset_idx is unchanged).
    // Actually, we removed indices > reset_idx, so reset_idx is still correct.
    let insert_pos = pair.reset_idx;
    for (i, instr) in between_instrs.into_iter().enumerate() {
        body.insert(insert_pos + i, instr);
    }
    for (i, span) in between_spans.into_iter().enumerate() {
        if insert_pos + i <= spans.len() {
            spans.insert(insert_pos + i, span);
        }
    }
}

// Fast-path construction (§09.3 + §09.5)

/// Configuration for building fast/slow path blocks.
struct ExpansionContext<'a> {
    pair: &'a ResetReusePair,
    proj_map: &'a ProjMap,
    claimed: &'a ClaimedFields,
    original_terminator: &'a ArcTerminator,
    merge_id: Option<ArcBlockId>,
}

/// Build the fast-path block: in-place field mutation via `Set`.
///
/// On the fast path, the value is uniquely owned (refcount == 1).
/// We mutate fields in-place and return the original object.
fn build_fast_path(
    func: &mut ArcFunction,
    block_id: ArcBlockId,
    ctx: &ExpansionContext<'_>,
    classifier: &dyn ArcClassification,
) -> ArcBlock {
    let mut body = Vec::new();

    // For each field being replaced:
    for (field_idx, arg) in ctx.pair.reuse_args.iter().enumerate() {
        let field = u32::try_from(field_idx)
            .unwrap_or_else(|_| panic!("field index {field_idx} exceeds u32::MAX"));

        // §09.5 Self-set elimination: if the arg was projected from the same
        // base at the same field index, the Set is a no-op.
        if is_self_set(ctx.pair.reset_var, field, *arg, ctx.proj_map) {
            continue;
        }

        // Dec the old field value if it's RC'd and not claimed.
        if !ctx.claimed.contains_key(&field) {
            if let Some(&old_val) = ctx.proj_map.get(&(ctx.pair.reset_var, field)) {
                let old_ty = func.var_type(old_val);
                if classifier.needs_rc(old_ty) {
                    body.push(ArcInstr::RcDec { var: old_val });
                }
            }
        }

        // Emit Set instruction.
        body.push(ArcInstr::Set {
            base: ctx.pair.reset_var,
            field,
            value: *arg,
        });
    }

    // SetTag for enum variants (in case the variant changed).
    if let CtorKind::EnumVariant { variant, .. } = ctx.pair.reuse_ctor {
        body.push(ArcInstr::SetTag {
            base: ctx.pair.reset_var,
            tag: u64::from(variant),
        });
    }

    // Terminator: merge or direct.
    let terminator = if let Some(mid) = ctx.merge_id {
        ArcTerminator::Jump {
            target: mid,
            args: vec![ctx.pair.reset_var], // result IS the original object
        }
    } else {
        let mut term = ctx.original_terminator.clone();
        term.substitute_var(ctx.pair.reuse_dst, ctx.pair.reset_var);
        term
    };

    ArcBlock {
        id: block_id,
        params: vec![],
        body,
        terminator,
    }
}

// Slow-path construction (§09.3)

/// Build the slow-path block: `RcDec` + fresh `Construct`.
///
/// On the slow path, the value is shared (refcount > 1). We decrement
/// the original (which won't reach zero) and allocate fresh memory.
fn build_slow_path(
    block_id: ArcBlockId,
    ctx: &ExpansionContext<'_>,
    suffix: &[ArcInstr],
) -> ArcBlock {
    let mut body = Vec::new();

    // Dec the shared original.
    body.push(ArcInstr::RcDec {
        var: ctx.pair.reset_var,
    });

    // Restore erased incs for claimed fields (§09.4).
    for (&_field, &proj_var) in ctx.claimed {
        body.push(ArcInstr::RcInc {
            var: proj_var,
            count: 1,
        });
    }

    // Fresh allocation via Construct.
    body.push(ArcInstr::Construct {
        dst: ctx.pair.reuse_dst,
        ty: ctx.pair.reuse_ty,
        ctor: ctx.pair.reuse_ctor,
        args: ctx.pair.reuse_args.clone(),
    });

    // Terminator: merge or direct.
    let terminator = if let Some(mid) = ctx.merge_id {
        ArcTerminator::Jump {
            target: mid,
            args: vec![ctx.pair.reuse_dst],
        }
    } else {
        ctx.original_terminator.clone()
    };

    // If no merge block, append suffix instructions.
    if ctx.merge_id.is_none() {
        body.extend_from_slice(suffix);
    }

    ArcBlock {
        id: block_id,
        params: vec![],
        body,
        terminator,
    }
}

// Merge block

/// Build the merge block that receives the result from fast/slow paths.
///
/// Creates a fresh parameter variable and substitutes `reuse_dst` with
/// it in the suffix instructions and terminator.
fn build_merge_block(
    func: &mut ArcFunction,
    block_id: ArcBlockId,
    reuse_dst: ArcVarId,
    reuse_ty: ori_types::Idx,
    suffix: &[ArcInstr],
    original_terminator: &ArcTerminator,
) -> ArcBlock {
    // The merge parameter receives either reset_var (fast) or reuse_dst (slow).
    let merge_param = func.fresh_var(reuse_ty);

    // Clone suffix and substitute reuse_dst → merge_param.
    let mut body: Vec<ArcInstr> = suffix.to_vec();
    for instr in &mut body {
        instr.substitute_var(reuse_dst, merge_param);
    }

    let mut terminator = original_terminator.clone();
    terminator.substitute_var(reuse_dst, merge_param);

    ArcBlock {
        id: block_id,
        params: vec![(merge_param, reuse_ty)],
        body,
        terminator,
    }
}

// Self-set detection (§09.5)

/// Check whether writing `value` to `base.field` is a self-set (no-op).
///
/// A Set is a self-set if `value` was obtained via `Project { value: base, field }`.
fn is_self_set(base: ArcVarId, field: u32, value: ArcVarId, proj_map: &ProjMap) -> bool {
    proj_map.get(&(base, field)) == Some(&value)
}

// Tests

#[cfg(test)]
mod tests;
