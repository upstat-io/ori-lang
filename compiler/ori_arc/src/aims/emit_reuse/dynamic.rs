//! Dynamic reuse emission (`MaybeShared` → `IsShared` + `Branch`).
//!
//! When a dying value has `MaybeShared` uniqueness, the runtime must check
//! whether the reference count is 1 before reusing the allocation. This module
//! emits the conditional branch expansion: `IsShared` check → fast path
//! (in-place `Set` mutations with self-set elimination) / slow path (`RcDec`
//! + fresh `Construct`).

use ori_ir::Span;
use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind, RcStrategy,
};

use super::set_ops::{build_proj_map, is_self_set, ProjMap};
use super::ReuseOpportunity;

/// Extracted data from a block needed for dynamic reuse expansion.
struct DynamicReuseContext {
    /// `RcDec` strategy from the original instruction.
    rc_strategy: RcStrategy,
    /// Projection map for self-set elimination.
    proj_map: ProjMap,
    /// Construct destination variable.
    dst: ArcVarId,
    /// Construct type.
    ty: Idx,
    /// Constructor kind.
    ctor: CtorKind,
    /// Constructor arguments.
    args: Vec<ArcVarId>,
    /// Instructions between `RcDec` and `Construct` (safe to reorder).
    between: Vec<ArcInstr>,
    /// Spans for between instructions.
    between_spans: Vec<Option<Span>>,
    /// Instructions after the `Construct`.
    suffix: Vec<ArcInstr>,
    /// Original block terminator.
    original_terminator: ArcTerminator,
    /// Whether a merge block is required.
    needs_merge: bool,
}

/// Extract all data needed for dynamic reuse before mutating the function.
///
/// Returns `None` if the block doesn't contain the expected instructions.
fn extract_dynamic_context(
    func: &ArcFunction,
    opp: &ReuseOpportunity,
    target_instr_idx: usize,
) -> Option<DynamicReuseContext> {
    let block_idx = opp.source_block.index();

    let rc_strategy = match &func.blocks[block_idx].body[opp.source_instr] {
        ArcInstr::RcDec { strategy, .. } => *strategy,
        other => {
            tracing::warn!(
                "expected RcDec at source_instr {}, got {:?}",
                opp.source_instr,
                other
            );
            return None;
        }
    };

    let (dst, ty, ctor, args) = match &func.blocks[block_idx].body[target_instr_idx] {
        ArcInstr::Construct {
            dst,
            ty,
            ctor,
            args,
        } => (*dst, *ty, *ctor, args.clone()),
        other => {
            tracing::warn!(
                "expected Construct at target_instr {}, got {:?}",
                target_instr_idx,
                other
            );
            return None;
        }
    };

    let proj_map = build_proj_map(
        &func.blocks[block_idx].body[..opp.source_instr],
        opp.source_var,
    );

    let between = func.blocks[block_idx].body[opp.source_instr + 1..target_instr_idx].to_vec();
    let between_spans = if block_idx < func.spans.len() {
        let start = (opp.source_instr + 1).min(func.spans[block_idx].len());
        let end = target_instr_idx.min(func.spans[block_idx].len());
        func.spans[block_idx][start..end].to_vec()
    } else {
        Vec::new()
    };

    let suffix = func.blocks[block_idx].body[target_instr_idx + 1..].to_vec();
    let original_terminator = func.blocks[block_idx].terminator.clone();
    let needs_merge = !suffix.is_empty() || original_terminator.uses_var(dst);

    Some(DynamicReuseContext {
        rc_strategy,
        proj_map,
        dst,
        ty,
        ctor,
        args,
        between,
        between_spans,
        suffix,
        original_terminator,
        needs_merge,
    })
}

/// Build the fast-path body: `Set` for changed fields + `SetTag` for enums.
///
/// Returns `(body, fields_skipped)`.
fn build_fast_body(
    source_var: ArcVarId,
    args: &[ArcVarId],
    ctor: CtorKind,
    proj_map: &ProjMap,
) -> (Vec<ArcInstr>, usize) {
    let mut body = Vec::new();
    let mut fields_skipped = 0;
    for (field_idx, arg) in args.iter().enumerate() {
        let field = u32::try_from(field_idx)
            .unwrap_or_else(|_| panic!("field index {field_idx} exceeds u32::MAX"));
        if is_self_set(source_var, field, *arg, proj_map) {
            fields_skipped += 1;
            continue;
        }
        body.push(ArcInstr::Set {
            base: source_var,
            field,
            value: *arg,
        });
    }
    if let CtorKind::EnumVariant { variant, .. } = ctor {
        body.push(ArcInstr::SetTag {
            base: source_var,
            tag: u64::from(variant),
        });
    }
    (body, fields_skipped)
}

/// Apply dynamic reuse with `IsShared` conditional branch.
///
/// For the dynamic path (`MaybeShared`, source RC unknown):
/// 1. Move "between" instructions (after `RcDec`, before `Construct`) up
/// 2. Emit `IsShared` check + `Branch` in the original block
/// 3. Fast path (unique): in-place `Set` mutations with self-set elimination
/// 4. Slow path (shared): `RcDec` + fresh `Construct`
/// 5. Merge block if suffix instructions or terminator uses the result
///
/// Returns the number of fields skipped via self-set elimination.
pub(super) fn apply_dynamic_reuse(func: &mut ArcFunction, opp: &ReuseOpportunity) -> usize {
    let (target_block, target_instr_idx) = opp.target_instr;
    debug_assert_eq!(opp.source_block, target_block, "same-block only");

    let Some(ctx) = extract_dynamic_context(func, opp, target_instr_idx) else {
        return 0;
    };

    let (fast_body, fields_skipped) =
        build_fast_body(opp.source_var, &ctx.args, ctx.ctor, &ctx.proj_map);

    let (fast_block, slow_block, merge_block) = build_dynamic_blocks(func, opp, &ctx, fast_body);

    rewrite_original_block(func, opp, &ctx);

    // Substitute dst → merge_param in existing blocks (before push).
    if let Some(mb) = &merge_block {
        let merge_param = mb.params[0].0;
        for block in &mut func.blocks {
            for instr in &mut block.body {
                instr.substitute_var(ctx.dst, merge_param);
            }
            block.terminator.substitute_var(ctx.dst, merge_param);
        }
    }

    func.push_block(fast_block);
    func.push_block(slow_block);
    if let Some(mb) = merge_block {
        func.push_block(mb);
    }

    fields_skipped
}

/// Build fast, slow, and optional merge blocks for dynamic reuse.
fn build_dynamic_blocks(
    func: &mut ArcFunction,
    opp: &ReuseOpportunity,
    ctx: &DynamicReuseContext,
    fast_body: Vec<ArcInstr>,
) -> (ArcBlock, ArcBlock, Option<ArcBlock>) {
    let fast_id = func.next_block_id();
    let slow_id = ArcBlockId::new(fast_id.raw() + 1);
    let merge_id = if ctx.needs_merge {
        Some(ArcBlockId::new(slow_id.raw() + 1))
    } else {
        None
    };

    // Merge block (built first — needs `func.fresh_var`).
    let merge_block = merge_id.map(|mid| {
        let merge_param = func.fresh_var(ctx.ty);
        let mut merge_body: Vec<ArcInstr> = ctx.suffix.clone();
        for instr in &mut merge_body {
            instr.substitute_var(ctx.dst, merge_param);
        }
        let mut merge_term = ctx.original_terminator.clone();
        merge_term.substitute_var(ctx.dst, merge_param);
        ArcBlock {
            id: mid,
            params: vec![(merge_param, ctx.ty)],
            body: merge_body,
            terminator: merge_term,
        }
    });

    // Fast block.
    let fast_terminator = if let Some(mid) = merge_id {
        ArcTerminator::Jump {
            target: mid,
            args: vec![opp.source_var],
        }
    } else {
        let mut term = ctx.original_terminator.clone();
        term.substitute_var(ctx.dst, opp.source_var);
        term
    };
    let fast_block = ArcBlock {
        id: fast_id,
        params: vec![],
        body: fast_body,
        terminator: fast_terminator,
    };

    // Slow block.
    let mut slow_body = vec![
        ArcInstr::RcDec {
            var: opp.source_var,
            strategy: ctx.rc_strategy,
        },
        ArcInstr::Construct {
            dst: ctx.dst,
            ty: ctx.ty,
            ctor: ctx.ctor,
            args: ctx.args.clone(),
        },
    ];
    let slow_terminator = if let Some(mid) = merge_id {
        ArcTerminator::Jump {
            target: mid,
            args: vec![ctx.dst],
        }
    } else {
        slow_body.extend_from_slice(&ctx.suffix);
        ctx.original_terminator.clone()
    };
    let slow_block = ArcBlock {
        id: slow_id,
        params: vec![],
        body: slow_body,
        terminator: slow_terminator,
    };

    (fast_block, slow_block, merge_block)
}

/// Rewrite the original block: prefix + between + `IsShared` + `Branch`.
fn rewrite_original_block(
    func: &mut ArcFunction,
    opp: &ReuseOpportunity,
    ctx: &DynamicReuseContext,
) {
    let block_idx = opp.source_block.index();
    let num_blocks = func.blocks.len();
    // fast/slow/merge IDs are based on current block count.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "ARC IR block counts fit in u32"
    )]
    let fast_id = ArcBlockId::new(num_blocks as u32);
    let slow_id = ArcBlockId::new(fast_id.raw() + 1);

    let is_shared_var = func.fresh_var(Idx::BOOL);
    let prefix = func.blocks[block_idx].body[..opp.source_instr].to_vec();
    let mut new_body = Vec::with_capacity(prefix.len() + ctx.between.len() + 1);
    new_body.extend(prefix);
    new_body.extend_from_slice(&ctx.between);
    new_body.push(ArcInstr::IsShared {
        dst: is_shared_var,
        var: opp.source_var,
    });
    func.blocks[block_idx].body = new_body;
    func.blocks[block_idx].terminator = ArcTerminator::Branch {
        cond: is_shared_var,
        then_block: slow_id,
        else_block: fast_id,
    };

    // Update spans.
    if block_idx < func.spans.len() {
        let old_spans = &func.spans[block_idx];
        let prefix_end = opp.source_instr.min(old_spans.len());
        let mut new_spans = Vec::with_capacity(prefix_end + ctx.between_spans.len() + 1);
        new_spans.extend_from_slice(&old_spans[..prefix_end]);
        new_spans.extend_from_slice(&ctx.between_spans);
        new_spans.push(None);
        func.spans[block_idx] = new_spans;
    }
}
