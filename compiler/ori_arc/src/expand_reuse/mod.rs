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

mod analysis;
mod paths;

use rustc_hash::FxHashMap;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind, RcStrategy,
};
use crate::ArcClassification;

/// Compute [`RcStrategy`] for a variable, falling back to `HeapPointer`
/// when Pool or `var_reprs` are unavailable.
fn rc_strategy(func: &ArcFunction, pool: Option<&ori_types::Pool>, var: ArcVarId) -> RcStrategy {
    let Some(repr) = func.var_repr(var) else {
        return RcStrategy::HeapPointer;
    };
    let Some(pool) = pool else {
        return RcStrategy::HeapPointer;
    };
    RcStrategy::from_var(repr, pool, func.var_type(var))
}

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

/// Configuration for building fast/slow path blocks.
struct ExpansionContext<'a> {
    pair: &'a ResetReusePair,
    proj_map: &'a ProjMap,
    claimed: &'a ClaimedFields,
    original_terminator: &'a ArcTerminator,
    merge_id: Option<ArcBlockId>,
}

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
pub fn expand_reset_reuse(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    pool: Option<&ori_types::Pool>,
) {
    let blocks_before = func.blocks.len();
    let mut block_idx = 0;

    // Process all blocks, including newly appended ones. When a block
    // contains multiple Reset/Reuse pairs, expanding the first pair moves
    // later instructions (including subsequent pairs) into a merge block
    // appended beyond the original block count. This loop visits those
    // new blocks to expand any remaining pairs.
    while block_idx < func.blocks.len() {
        try_expand_block(func, block_idx, classifier, pool);
        block_idx += 1;
    }

    tracing::debug!(
        function = func.name.raw(),
        blocks_before,
        blocks_after = func.blocks.len(),
        "constructor reuse expansion complete"
    );
}

// Block expansion

/// Attempt to expand a single block's `Reset`/`Reuse` pair.
fn try_expand_block(
    func: &mut ArcFunction,
    block_idx: usize,
    classifier: &dyn ArcClassification,
    pool: Option<&ori_types::Pool>,
) {
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
    let proj_map = analysis::build_proj_map(
        &func.blocks[block_idx].body[..pair.reset_idx],
        pair.reset_var,
    );

    // 2. Projection-increment erasure (§09.4): erase RcInc ops for projected
    //    fields, building a claimed-fields mask.
    let (erased_indices, claimed) =
        analysis::erase_proj_increments(&func.blocks[block_idx].body[..pair.reset_idx], &proj_map);

    analysis::apply_erasures(func, block_idx, &erased_indices);

    // Re-find the pair (indices shifted due to erasures).
    let Some(pair) = find_reset_reuse_pair(&func.blocks[block_idx]) else {
        debug_assert!(false, "pair should still exist after erasure");
        return;
    };

    // 3. Move "between" instructions (Reset..Reuse exclusive) to before
    //    the Reset. They don't use the reset_var (constraint from detection),
    //    so reordering is safe.
    analysis::move_between_to_prefix(func, block_idx, pair.reset_idx, pair.reuse_idx);

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
    let fast_block = paths::build_fast_path(func, fast_id, &ctx, classifier, pool);

    // 7. Build slow-path block (§09.3).
    let slow_block = paths::build_slow_path(slow_id, &ctx, &suffix, pool, func);

    // 8. Build merge block if needed.
    let merge_block = merge_id.map(|mid| {
        paths::build_merge_block(
            func,
            mid,
            &ctx,
            &suffix,
            &original_terminator,
            classifier,
            pool,
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

    // 10. Propagate merge substitution to all existing blocks.
    if let Some(mb) = &merge_block {
        let merge_param = mb.params[0].0;
        analysis::propagate_merge_substitution(func, merge_param, pair.reuse_dst);
    }

    // 11. Push new blocks.
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
                        // Defense-in-depth: collection ctors are gated at
                        // detection time (reset_reuse::is_collection_ctor),
                        // so this should be unreachable. Kept as a safety net.
                        if matches!(
                            ctor,
                            CtorKind::ListLiteral | CtorKind::MapLiteral | CtorKind::SetLiteral
                        ) {
                            continue;
                        }
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

// Tests

#[cfg(test)]
mod tests;
