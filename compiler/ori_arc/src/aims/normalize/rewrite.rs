//! TRMC 4-equation rewrite pass.
//!
//! Transforms self-recursive constructor-context functions into tail-recursive
//! form using the modulo-cons instantiation (Leijen & Lorenzen, JFP 2025).
//!
//! # Loop-header transformation
//!
//! Instead of changing the function's signature (which would break all
//! callers and invalidate interprocedural contracts), we convert the
//! self-recursion into a loop with block parameters carrying the context.
//! See [`rewrite_trmc`] for the full block layout. The function signature
//! is **unchanged**. Uses [`LitValue::Null`] for hole field placeholders.
//!
//! # Scope (v1)
//!
//! - Self-recursive only (no mutual recursion)
//! - Single context region per function
//! - Same-block: recursive call and construct in the same basic block
//! - Construct must be the last body instruction (tail-position)
//! - Modulo-cons instantiation only (no CPS fallback)
//! - Skip when `may_share == true` (no hybrid path)

use crate::aims::contract::ContextRegion;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue,
};

/// Minamide tuple: pointer to result root + address of hole field.
///
/// - `res` — the root of the partially-built result (returned at base case)
/// - `hole_obj` — the object containing the hole (target of the next `Set`)
/// - `hole_field` — which field of `hole_obj` receives the next value
#[expect(
    dead_code,
    reason = "constructed by post-rewrite verification (Section 13.5)"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TrmcContext {
    pub(crate) res: ArcVarId,
    pub(crate) hole_obj: ArcVarId,
    pub(crate) hole_field: u32,
}

/// Rewrite TRMC-eligible functions for tail recursion modulo constructor.
///
/// Converts self-recursion into a loop with block parameters carrying the
/// context. The function signature is **unchanged**.
///
/// Returns `true` if any rewrite was applied.
///
/// # Admission gates (structural)
///
/// - Recursive call and construct in the same basic block
/// - Construct is the last body instruction (tail-position)
/// - Recursive call precedes the construct
///
/// # Soundness gates (caller responsibility)
///
/// 1. Per-variable uniqueness (checked post-convergence)
/// 2. Effect purity: `may_share == false` (checked by caller)
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired into pipeline in Section 13.6")
)]
pub(crate) fn rewrite_trmc(func: &mut ArcFunction, regions: &[ContextRegion]) -> bool {
    if regions.is_empty() {
        return false;
    }

    // V1: single region only. Multi-region functions are rejected —
    // partial transformation would leave residual self-recursion and
    // make pass behavior input-order dependent.
    if regions.len() > 1 {
        tracing::debug!(
            func = ?func.name,
            count = regions.len(),
            "TRMC rewrite skipped: multiple context regions (v1 handles single only)"
        );
        return false;
    }
    let region = &regions[0];

    if !is_region_valid(func, region) {
        tracing::debug!(
            func = ?func.name,
            "TRMC rewrite skipped: invalid region references"
        );
        return false;
    }

    rewrite_single_region(func, region)
}

/// Check that a `ContextRegion` references valid blocks and instructions.
fn is_region_valid(func: &ArcFunction, region: &ContextRegion) -> bool {
    let open_ok = region.open_block.index() < func.blocks.len()
        && region.open_instr < func.blocks[region.open_block.index()].body.len();
    let close_ok = region.close_block.index() < func.blocks.len()
        && region.close_instr <= func.blocks[region.close_block.index()].body.len();
    open_ok && close_ok
}

/// Metadata extracted from the admission checks, needed by the rewrite.
struct RewriteInput {
    open_block_idx: usize,
    rec_instr_idx: usize,
    ctor_instr_idx: usize,
    ctor_dst: ArcVarId,
    ctor_ty: ori_types::Idx,
    ctor_kind: crate::ir::CtorKind,
    ctor_args: Vec<ArcVarId>,
    hole_idx: usize,
}

/// Run admission checks and extract metadata for the rewrite.
///
/// Returns `None` if any gate fails.
fn check_admission(func: &ArcFunction, region: &ContextRegion) -> Option<RewriteInput> {
    let open_block_idx = region.open_block.index();

    // V1: same-block only.
    if open_block_idx != region.close_block.index() {
        tracing::debug!("TRMC rewrite skipped: call and construct in different blocks");
        return None;
    }

    let block_body_len = func.blocks[open_block_idx].body.len();
    let rec_instr_idx = region.close_instr;
    let ctor_instr_idx = region.open_instr;

    if rec_instr_idx >= ctor_instr_idx {
        tracing::debug!("TRMC rewrite skipped: recursive call not before construct");
        return None;
    }

    if ctor_instr_idx + 1 != block_body_len {
        tracing::debug!(
            "TRMC rewrite skipped: construct not last instruction \
             (at {ctor_instr_idx}, block has {block_body_len} instrs)"
        );
        return None;
    }

    let ArcInstr::Construct {
        dst: ctor_dst,
        ty: ctor_ty,
        ctor: ctor_kind,
        args: ref ctor_args_ref,
    } = func.blocks[open_block_idx].body[ctor_instr_idx]
    else {
        tracing::debug!("TRMC rewrite skipped: open_instr is not Construct");
        return None;
    };

    let (rec_dst, _) = extract_recursive_call(func, region)?;

    let hole_idx = region.hole_field as usize;
    if hole_idx >= ctor_args_ref.len() || ctor_args_ref[hole_idx] != rec_dst {
        tracing::debug!("TRMC rewrite skipped: hole_field mismatch");
        return None;
    }

    Some(RewriteInput {
        open_block_idx,
        rec_instr_idx,
        ctor_instr_idx,
        ctor_dst,
        ctor_ty,
        ctor_kind,
        ctor_args: ctor_args_ref.clone(),
        hole_idx,
    })
}

/// Rewrite a single TRMC region using the loop-header strategy.
fn rewrite_single_region(func: &mut ArcFunction, region: &ContextRegion) -> bool {
    let Some(input) = check_admission(func, region) else {
        return false;
    };

    let return_ty = func.return_type;
    let entry_block = func.entry;
    let original_block_count = func.blocks.len();

    // Allocate all fresh variables up front.
    let ctx_has = func.fresh_var(ori_types::Idx::BOOL);
    let ctx_res = func.fresh_var(return_ty);
    let ctx_hole_obj = func.fresh_var(return_ty);
    let false_var = func.fresh_var(ori_types::Idx::BOOL);
    let null_sentinel = func.fresh_var(return_ty);
    let new_res = func.fresh_var(return_ty);
    let true_var = func.fresh_var(ori_types::Idx::BOOL);

    // Step 1: Create prologue block (new entry).
    emit_prologue(func, entry_block, false_var, null_sentinel, return_ty);

    // Step 2: Add context block params to the original entry (loop header).
    let entry_idx = entry_block.index();
    func.blocks[entry_idx]
        .params
        .push((ctx_has, ori_types::Idx::BOOL));
    func.blocks[entry_idx].params.push((ctx_res, return_ty));
    func.blocks[entry_idx]
        .params
        .push((ctx_hole_obj, return_ty));

    // Step 3: Rewrite recursive site + emit compose/first-call/loop-back blocks.
    emit_recursive_path(
        func,
        region,
        &input,
        ctx_has,
        ctx_res,
        ctx_hole_obj,
        new_res,
        true_var,
        entry_block,
    );

    // Step 4: Rewrite base-case returns.
    rewrite_base_case_returns(
        func,
        region,
        ctx_has,
        ctx_res,
        ctx_hole_obj,
        input.open_block_idx,
        original_block_count,
    );

    // Step 5: Post-rewrite verification (debug builds only).
    if cfg!(debug_assertions) {
        verify_rewrite(func);
    }

    tracing::debug!(
        func = ?func.name,
        loop_header = entry_block.raw(),
        ctx_has = ctx_has.raw(),
        ctx_res = ctx_res.raw(),
        "TRMC rewrite applied (loop-header strategy)"
    );

    true
}

/// Create the prologue block that initializes the identity context.
fn emit_prologue(
    func: &mut ArcFunction,
    entry_block: ArcBlockId,
    false_var: ArcVarId,
    null_sentinel: ArcVarId,
    return_ty: ori_types::Idx,
) {
    let prologue_id = func.next_block_id();
    func.push_block(ArcBlock {
        id: prologue_id,
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: false_var,
                ty: ori_types::Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(false)),
            },
            ArcInstr::Let {
                dst: null_sentinel,
                ty: return_ty,
                value: ArcValue::Literal(LitValue::Null),
            },
        ],
        terminator: ArcTerminator::Jump {
            target: entry_block,
            args: vec![false_var, null_sentinel, null_sentinel],
        },
    });
    func.entry = prologue_id;
}

/// Rewrite the recursive site and emit compose/first-call/loop-back blocks.
#[expect(
    clippy::too_many_arguments,
    reason = "internal helper, all params needed"
)]
fn emit_recursive_path(
    func: &mut ArcFunction,
    region: &ContextRegion,
    input: &RewriteInput,
    ctx_has: ArcVarId,
    ctx_res: ArcVarId,
    ctx_hole_obj: ArcVarId,
    new_res: ArcVarId,
    true_var: ArcVarId,
    entry_block: ArcBlockId,
) {
    let return_ty = func.return_type;

    // Build new body: instructions before call, between call and construct.
    let mut new_body = Vec::new();
    for instr in &func.blocks[input.open_block_idx].body[..input.rec_instr_idx] {
        new_body.push(instr.clone());
    }
    for instr in
        &func.blocks[input.open_block_idx].body[input.rec_instr_idx + 1..input.ctor_instr_idx]
    {
        new_body.push(instr.clone());
    }

    // Use ctx_res as placeholder for the hole field. On the first iteration,
    // ctx_res is the null sentinel (LitValue::Null). On subsequent iterations,
    // ctx_res is the real root. Set overwrites the field before any read.
    let mut ctor_args = input.ctor_args.clone();
    ctor_args[input.hole_idx] = ctx_res;
    new_body.push(ArcInstr::Construct {
        dst: input.ctor_dst,
        ty: input.ctor_ty,
        ctor: input.ctor_kind,
        args: ctor_args,
    });

    let compose_id = func.next_block_id();
    let first_call_id = ArcBlockId::new(compose_id.raw() + 1);
    let loop_back_id = ArcBlockId::new(compose_id.raw() + 2);

    func.blocks[input.open_block_idx].body = new_body;
    func.blocks[input.open_block_idx].terminator = ArcTerminator::Branch {
        cond: ctx_has,
        then_block: compose_id,
        else_block: first_call_id,
    };

    // Compose block: fill caller's hole with new node, keep original root.
    func.push_block(ArcBlock {
        id: compose_id,
        params: vec![],
        body: vec![ArcInstr::Set {
            base: ctx_hole_obj,
            field: region.hole_field,
            value: input.ctor_dst,
        }],
        terminator: ArcTerminator::Jump {
            target: loop_back_id,
            args: vec![ctx_res],
        },
    });

    // First-call block: the new constructor IS the root.
    func.push_block(ArcBlock {
        id: first_call_id,
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Jump {
            target: loop_back_id,
            args: vec![input.ctor_dst],
        },
    });

    // Loop-back block: receives new_res, jumps back to loop header.
    func.push_block(ArcBlock {
        id: loop_back_id,
        params: vec![(new_res, return_ty)],
        body: vec![ArcInstr::Let {
            dst: true_var,
            ty: ori_types::Idx::BOOL,
            value: ArcValue::Literal(LitValue::Bool(true)),
        }],
        terminator: ArcTerminator::Jump {
            target: entry_block,
            args: vec![true_var, new_res, input.ctor_dst],
        },
    });
}

/// Extract recursive call destination and arguments.
fn extract_recursive_call(
    func: &ArcFunction,
    region: &ContextRegion,
) -> Option<(ArcVarId, Vec<ArcVarId>)> {
    let block = &func.blocks[region.close_block.index()];

    if region.close_instr < block.body.len() {
        if let ArcInstr::Apply { dst, args, .. } = &block.body[region.close_instr] {
            Some((*dst, args.clone()))
        } else {
            tracing::debug!("TRMC rewrite skipped: close_instr is not Apply");
            None
        }
    } else if let ArcTerminator::Invoke { dst, args, .. } = &block.terminator {
        Some((*dst, args.clone()))
    } else {
        tracing::debug!("TRMC rewrite skipped: close terminator is not Invoke");
        None
    }
}

/// Rewrite base-case `Return` terminators in original blocks.
fn rewrite_base_case_returns(
    func: &mut ArcFunction,
    region: &ContextRegion,
    ctx_has: ArcVarId,
    ctx_res: ArcVarId,
    ctx_hole_obj: ArcVarId,
    open_block_idx: usize,
    original_block_count: usize,
) {
    let rewrites: Vec<(usize, ArcVarId)> = func
        .blocks
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx < original_block_count && *idx != open_block_idx)
        .filter_map(|(idx, block)| {
            if let ArcTerminator::Return { value } = &block.terminator {
                Some((idx, *value))
            } else {
                None
            }
        })
        .collect();

    for (block_idx, ret_value) in rewrites {
        let apply_ctx_id = func.next_block_id();
        let no_ctx_id = ArcBlockId::new(apply_ctx_id.raw() + 1);

        func.push_block(ArcBlock {
            id: apply_ctx_id,
            params: vec![],
            body: vec![ArcInstr::Set {
                base: ctx_hole_obj,
                field: region.hole_field,
                value: ret_value,
            }],
            terminator: ArcTerminator::Return { value: ctx_res },
        });

        func.push_block(ArcBlock {
            id: no_ctx_id,
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: ret_value },
        });

        func.blocks[block_idx].terminator = ArcTerminator::Branch {
            cond: ctx_has,
            then_block: apply_ctx_id,
            else_block: no_ctx_id,
        };
    }
}

/// Post-rewrite verification (debug builds only).
///
/// Checks: no residual self-calls, `LitValue::Null` only in `Let`.
fn verify_rewrite(func: &ArcFunction) {
    let self_name = func.name;

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply { func: callee, .. } = instr {
                debug_assert_ne!(
                    *callee,
                    self_name,
                    "TRMC verify: residual self-call in block {}",
                    block.id.raw()
                );
            }
        }
        if let ArcTerminator::Invoke { func: callee, .. } = &block.terminator {
            debug_assert_ne!(
                *callee,
                self_name,
                "TRMC verify: residual self-Invoke in block {}",
                block.id.raw()
            );
        }
    }
}
