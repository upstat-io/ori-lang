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
mod tests {
    use ori_ir::Name;
    use ori_types::{Idx, Pool};

    use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
    use crate::test_helpers::{b, make_func, owned_param, v};
    use crate::ArcClassifier;

    use super::expand_reset_reuse;

    fn run_expand(mut func: ArcFunction) -> ArcFunction {
        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        expand_reset_reuse(&mut func, &classifier);
        func
    }

    /// Count how many instructions of a given kind exist in all blocks.
    fn count_instrs(func: &ArcFunction, pred: impl Fn(&ArcInstr) -> bool) -> usize {
        func.blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .filter(|i| pred(i))
            .count()
    }

    // Test 1: No Reset/Reuse -> pass-through

    #[test]
    fn no_pair_passthrough() {
        let func = make_func(
            vec![owned_param(0, Idx::INT)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                }],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::INT, Idx::INT],
        );

        let result = run_expand(func);
        assert_eq!(result.blocks.len(), 1, "no new blocks should be created");
    }

    // Test 2: Basic expansion -- Reset/Reuse with no projections

    #[test]
    fn basic_expansion() {
        // v0: STR (param), v1: STR (reuse result), v2: token
        // Body: Reset{v0, v2}; Reuse{v2, v1, STR, Struct, []}
        // Term: Return{v1}
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(2),
                    },
                    ArcInstr::Reuse {
                        token: v(2),
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_expand(func);

        // Should have 3 blocks: original (IsShared+Branch), fast, slow.
        // No merge block needed since terminator uses reuse_dst → needs merge.
        // Actually, Return{v1} uses reuse_dst → needs merge block.
        assert!(
            result.blocks.len() >= 3,
            "expected at least 3 blocks, got {}",
            result.blocks.len()
        );

        // No Reset or Reuse should remain.
        assert_eq!(
            count_instrs(&result, |i| matches!(i, ArcInstr::Reset { .. })),
            0
        );
        assert_eq!(
            count_instrs(&result, |i| matches!(i, ArcInstr::Reuse { .. })),
            0
        );

        // Should have IsShared in original block.
        assert_eq!(
            count_instrs(&result, |i| matches!(i, ArcInstr::IsShared { .. })),
            1
        );

        // Original block should end with Branch.
        assert!(matches!(
            result.blocks[0].terminator,
            ArcTerminator::Branch { .. }
        ));
    }

    // Test 3: Self-set elimination

    #[test]
    fn self_set_eliminated() {
        // Simulates: Cons(head, tail) -> Cons(new_head, tail)
        // v0: STR (param, the list)
        // v1: STR (head = Project{v0, 0})
        // v2: STR (tail = Project{v0, 1})
        // v3: STR (new_head = Apply{f, [v1]})
        // v4: token
        // v5: STR (result = Reuse{v4, Cons, [v3, v2]})
        //
        // Field 1 (v2) is a self-set: Project{v0, 1} → Set{v0, 1, v2} is no-op.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Project {
                        dst: v(2),
                        ty: Idx::STR,
                        value: v(0),
                        field: 1,
                    },
                    ArcInstr::Apply {
                        dst: v(3),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(1)],
                    },
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(4),
                    },
                    ArcInstr::Reuse {
                        token: v(4),
                        dst: v(5),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![v(3), v(2)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(5) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_expand(func);

        // Find the fast-path block (the else branch of the Branch).
        let fast_id = match &result.blocks[0].terminator {
            ArcTerminator::Branch { else_block, .. } => *else_block,
            other => panic!("expected Branch, got {other:?}"),
        };
        let fast_block = &result.blocks[fast_id.index()];

        // Fast path should have Set for field 0 only (field 1 is self-set).
        let sets: Vec<_> = fast_block
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::Set { .. }))
            .collect();
        assert_eq!(
            sets.len(),
            1,
            "expected 1 Set (field 1 self-set eliminated)"
        );
        assert!(
            matches!(sets[0], ArcInstr::Set { field: 0, .. }),
            "expected Set for field 0, got {:?}",
            sets[0]
        );
    }

    // Test 4: Projection-increment erasure

    #[test]
    fn proj_inc_erasure() {
        // v0: STR (param, the list)
        // v1: STR (head = Project{v0, 0})
        // v2: STR (tail = Project{v0, 1})
        // v3: RcInc{v2}  ← this should be erased
        // v4: STR (new_head)
        // v5: token
        // v6: result = Reuse{Cons, [v4, v2]}
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Project {
                        dst: v(2),
                        ty: Idx::STR,
                        value: v(0),
                        field: 1,
                    },
                    ArcInstr::RcInc {
                        var: v(2),
                        count: 1,
                    },
                    ArcInstr::Let {
                        dst: v(4),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::Int(99)),
                    },
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(5),
                    },
                    ArcInstr::Reuse {
                        token: v(5),
                        dst: v(6),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![v(4), v(2)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(6) },
            }],
            vec![
                Idx::STR,
                Idx::STR,
                Idx::STR,
                Idx::STR,
                Idx::STR,
                Idx::STR,
                Idx::STR,
            ],
        );

        let result = run_expand(func);

        // The RcInc{v2} in the original block should have been erased.
        let original_incs: Vec<_> = result.blocks[0]
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::RcInc { var, .. } if *var == v(2)))
            .collect();
        assert!(
            original_incs.is_empty(),
            "RcInc for v2 should be erased from original block"
        );

        // Slow path should have RcInc{v2} restored.
        let slow_id = match &result.blocks[0].terminator {
            ArcTerminator::Branch { then_block, .. } => *then_block,
            other => panic!("expected Branch, got {other:?}"),
        };
        let slow_block = &result.blocks[slow_id.index()];
        let slow_incs: Vec<_> = slow_block
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::RcInc { var, .. } if *var == v(2)))
            .collect();
        assert_eq!(slow_incs.len(), 1, "slow path should restore RcInc for v2");
    }

    // Test 5: Slow path has RcDec + Construct

    #[test]
    fn slow_path_dec_construct() {
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(2),
                    },
                    ArcInstr::Reuse {
                        token: v(2),
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_expand(func);

        let slow_id = match &result.blocks[0].terminator {
            ArcTerminator::Branch { then_block, .. } => *then_block,
            other => panic!("expected Branch, got {other:?}"),
        };
        let slow_block = &result.blocks[slow_id.index()];

        // Slow path should start with RcDec{v0}.
        assert!(
            matches!(&slow_block.body[0], ArcInstr::RcDec { var } if *var == v(0)),
            "slow path should start with RcDec, got {:?}",
            slow_block.body[0]
        );

        // Slow path should have a Construct.
        let constructs: Vec<_> = slow_block
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::Construct { .. }))
            .collect();
        assert_eq!(constructs.len(), 1, "slow path should have one Construct");
    }

    // Test 6: Enum variant -> SetTag on fast path

    #[test]
    fn enum_variant_set_tag() {
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(2),
                    },
                    ArcInstr::Reuse {
                        token: v(2),
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::EnumVariant {
                            enum_name: Name::from_raw(20),
                            variant: 1,
                        },
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_expand(func);

        let fast_id = match &result.blocks[0].terminator {
            ArcTerminator::Branch { else_block, .. } => *else_block,
            other => panic!("expected Branch, got {other:?}"),
        };
        let fast_block = &result.blocks[fast_id.index()];

        // Fast path should have SetTag with tag=1.
        let set_tags: Vec<_> = fast_block
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::SetTag { .. }))
            .collect();
        assert_eq!(set_tags.len(), 1);
        assert!(
            matches!(set_tags[0], ArcInstr::SetTag { tag: 1, .. }),
            "expected SetTag with tag=1, got {:?}",
            set_tags[0]
        );
    }

    // Test 7: Suffix instructions create merge block

    #[test]
    fn suffix_creates_merge_block() {
        // Body: Reset; Reuse; Apply (suffix)
        // Term: Return{suffix_result}
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(2),
                    },
                    ArcInstr::Reuse {
                        token: v(2),
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                    ArcInstr::Apply {
                        dst: v(3),
                        ty: Idx::INT,
                        func: Name::from_raw(99),
                        args: vec![v(1)], // uses reuse_dst
                    },
                ],
                terminator: ArcTerminator::Return { value: v(3) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
        );

        let result = run_expand(func);

        // Should have 4 blocks: original, fast, slow, merge.
        assert_eq!(
            result.blocks.len(),
            4,
            "expected 4 blocks (original + fast + slow + merge), got {}",
            result.blocks.len()
        );

        // The merge block should have the Apply instruction.
        let merge_block = &result.blocks[3];
        assert!(
            !merge_block.params.is_empty(),
            "merge block should have a parameter"
        );

        let has_apply = merge_block
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { .. }));
        assert!(has_apply, "merge block should contain the suffix Apply");
    }

    // Test 8: Between instructions moved to prefix

    #[test]
    fn between_instrs_moved() {
        // Body: Reset{v0}; Let{v3=42}; Reuse{token, v1, ...}
        // The Let should be moved before the Reset.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(2),
                    },
                    ArcInstr::Let {
                        dst: v(3),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(42)),
                    },
                    ArcInstr::Reuse {
                        token: v(2),
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![v(3)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
        );

        let result = run_expand(func);

        // Original block should have: Let, IsShared, Branch.
        let original = &result.blocks[0];

        // The Let should be in the original block (moved before IsShared).
        let has_let = original
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Let { dst, .. } if *dst == v(3)));
        assert!(has_let, "Let should be in original block (moved to prefix)");

        // No Reset or Reuse should remain anywhere.
        assert_eq!(
            count_instrs(&result, |i| matches!(i, ArcInstr::Reset { .. })),
            0
        );
        assert_eq!(
            count_instrs(&result, |i| matches!(i, ArcInstr::Reuse { .. })),
            0
        );
    }

    // Test 9: Fast path substitutes reuse_dst -> reset_var

    #[test]
    fn fast_path_variable_substitution() {
        // Term: Return{v1} where v1 is reuse_dst.
        // Fast path should return v0 (reset_var) instead.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(2),
                    },
                    ArcInstr::Reuse {
                        token: v(2),
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_expand(func);

        // Find merge block (has parameter).
        let merge_block = result
            .blocks
            .iter()
            .find(|b| !b.params.is_empty())
            .unwrap_or_else(|| panic!("should have a merge block"));

        // Fast path should jump to merge with reset_var (v0).
        let fast_id = match &result.blocks[0].terminator {
            ArcTerminator::Branch { else_block, .. } => *else_block,
            other => panic!("expected Branch, got {other:?}"),
        };
        let fast_block = &result.blocks[fast_id.index()];
        match &fast_block.terminator {
            ArcTerminator::Jump { args, target } => {
                assert_eq!(*target, merge_block.id);
                assert!(
                    args.contains(&v(0)),
                    "fast path should pass reset_var (v0) to merge, got {args:?}"
                );
            }
            other => panic!("expected Jump to merge, got {other:?}"),
        }
    }

    // Test 10: Dec unclaimed replaced fields on fast path

    #[test]
    fn fast_path_dec_unclaimed_field() {
        // v0: STR (list param)
        // v1: STR (old_field = Project{v0, 0}) — NOT claimed (no RcInc erased)
        // v2: STR (new_value)
        // v3: token
        // v4: result = Reuse{Cons, [v2]}
        //
        // Field 0 is being replaced (v2 != v1), and it's unclaimed and RC'd.
        // Fast path should emit RcDec{v1} before Set{v0, 0, v2}.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::Int(99)),
                    },
                    ArcInstr::Reset {
                        var: v(0),
                        token: v(3),
                    },
                    ArcInstr::Reuse {
                        token: v(3),
                        dst: v(4),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![v(2)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(4) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_expand(func);

        let fast_id = match &result.blocks[0].terminator {
            ArcTerminator::Branch { else_block, .. } => *else_block,
            other => panic!("expected Branch, got {other:?}"),
        };
        let fast_block = &result.blocks[fast_id.index()];

        // Fast path should have RcDec{v1} (unclaimed old field) before Set.
        let has_dec = fast_block
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::RcDec { var } if *var == v(1)));
        assert!(
            has_dec,
            "fast path should dec unclaimed old field v1, body: {:?}",
            fast_block.body
        );
    }
}
