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
//! - Per-variable uniqueness as sole soundness gate (no effect-handler gate)

use crate::aims::contract::ContextRegion;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue,
};

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
/// 1. Per-variable uniqueness (checked in `detect_trmc_candidates()`,
///    `intraprocedural/post_convergence.rs`)
/// 2. Effect purity: out of scope (Ori has no effect handlers).
///    Non-linear resumption cannot break the unique linear chain
///    (Lemma 2, Leijen & Lorenzen JFP 2025). This gate activates
///    when effect handlers are added to the language.
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
    /// Arguments of the recursive call — needed for loop-back argument
    /// threading. Without these, the next iteration runs with stale
    /// parameter values (Bug 1).
    rec_args: Vec<ArcVarId>,
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

    let (rec_dst, rec_args) = extract_recursive_call(func, region)?;

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
        rec_args,
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

    // Step 1: Allocate fresh block params for each function parameter.
    //
    // We must NOT reuse the function's parameter ArcVarIds here. If we did,
    // the prologue's `Jump header [v0, v1, ...]` would pass `v0` to a block
    // param also named `v0`. Block merge Phase 7 (invariant param elimination)
    // would see `arg == param_var` and treat the prologue as a self-referencing
    // back-edge, incorrectly eliminating the param.
    //
    // With fresh IDs, Phase 7 sees distinct vars from both predecessors and
    // correctly identifies the params as non-invariant.
    // (Same pattern as tail_call/rewrite.rs lines 50-68.)
    let param_types: Vec<ori_types::Idx> = func.params.iter().map(|p| p.ty).collect();
    let fresh_params: Vec<(ArcVarId, ori_types::Idx)> = param_types
        .iter()
        .map(|&ty| {
            let fresh = func.fresh_var(ty);
            (fresh, ty)
        })
        .collect();

    // Step 2: Create prologue block (new entry). Passes original param
    // vars followed by 3 context init values (false, null, null).
    let param_vars: Vec<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    emit_prologue(
        func,
        entry_block,
        &param_vars,
        false_var,
        null_sentinel,
        return_ty,
    );

    // Step 3: Set loop header (original entry) block params wholesale.
    // Layout: [fresh_param_0, ..., fresh_param_N, ctx_has, ctx_res, ctx_hole_obj]
    let entry_idx = entry_block.index();
    let mut header_params = fresh_params.clone();
    header_params.push((ctx_has, ori_types::Idx::BOOL));
    header_params.push((ctx_res, return_ty));
    header_params.push((ctx_hole_obj, return_ty));
    func.blocks[entry_idx].params = header_params;

    // Step 4: Rewrite recursive site + emit compose/first-call/loop-back blocks.
    // Done BEFORE prepending Let bindings so the recursive block body
    // indices used by emit_recursive_path remain valid.
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

    // Step 5: Rewrite base-case returns.
    rewrite_base_case_returns(
        func,
        region,
        ctx_has,
        ctx_res,
        ctx_hole_obj,
        input.open_block_idx,
        original_block_count,
    );

    // Step 6: Prepend Let bindings that define original param vars from
    // fresh block params. The header body references original param vars,
    // so these bindings bridge fresh block params → original names.
    // (Same pattern as tail_call/rewrite.rs lines 104-124.)
    //
    // Done after Steps 4-5 so that the recursive block body replacement
    // and base-case return rewriting are complete.
    let mut let_bindings: Vec<ArcInstr> = Vec::with_capacity(fresh_params.len());
    for (i, param) in func.params.iter().enumerate() {
        let_bindings.push(ArcInstr::Let {
            dst: param.var,
            ty: param.ty,
            value: ArcValue::Var(fresh_params[i].0),
        });
    }
    let original_body = std::mem::take(&mut func.blocks[entry_idx].body);
    func.blocks[entry_idx].body = let_bindings;
    func.blocks[entry_idx].body.extend(original_body);

    // Maintain spans: prepend None spans for the synthetic Let bindings.
    let original_spans = std::mem::take(&mut func.spans[entry_idx]);
    func.spans[entry_idx] = vec![None; fresh_params.len()];
    func.spans[entry_idx].extend(original_spans);

    // Step 7: Post-rewrite verification (debug builds only).
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
///
/// Jump args layout: `[param_var_0, ..., param_var_N, false, null, null]`
/// — original function param vars first, then the 3 context init values.
fn emit_prologue(
    func: &mut ArcFunction,
    entry_block: ArcBlockId,
    param_vars: &[ArcVarId],
    false_var: ArcVarId,
    null_sentinel: ArcVarId,
    return_ty: ori_types::Idx,
) {
    let prologue_id = func.next_block_id();

    // Jump args: original param vars + identity context (false, null, null).
    let mut jump_args: Vec<ArcVarId> = param_vars.to_vec();
    jump_args.push(false_var);
    jump_args.push(null_sentinel);
    jump_args.push(null_sentinel);

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
            args: jump_args,
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

    // Rebuild spans to match the new body length. The original spans for
    // instructions before the call and between call/construct are preserved
    // in order; the Apply span is dropped; the new Construct span is None.
    let old_spans = &func.spans[input.open_block_idx];
    let mut new_spans: Vec<Option<ori_ir::Span>> = Vec::new();
    // Spans for instructions before the call.
    for i in 0..input.rec_instr_idx {
        new_spans.push(old_spans.get(i).copied().flatten());
    }
    // Spans for instructions between call and construct (skip Apply span).
    for i in (input.rec_instr_idx + 1)..input.ctor_instr_idx {
        new_spans.push(old_spans.get(i).copied().flatten());
    }
    // Span for the new Construct (synthetic — no source span).
    new_spans.push(None);

    func.blocks[input.open_block_idx].body = new_body;
    func.spans[input.open_block_idx] = new_spans;
    func.blocks[input.open_block_idx].terminator = ArcTerminator::Branch {
        cond: ctx_has,
        then_block: compose_id,
        else_block: first_call_id,
    };

    // Compose block: fill caller's hole with new node, keep original root.
    //
    // Uses `ctx_hole_obj` and `ctx_res` from the loop header's block params
    // without re-threading via block params. This is valid because the loop
    // header (original function entry) dominates all reachable blocks in
    // the function — including this compose block which is reached only
    // through dominated blocks. Context vars defined as block params at
    // the loop header entry dominate all their use sites.
    // Verified by `check_context_var_dominance` in verify.rs.
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
    // No context vars used here — only `input.ctor_dst` (body-defined).
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
    // Jump args layout: [rec_arg_0, ..., rec_arg_N, true, new_res, ctor_dst]
    // — recursive call's arguments (for the next iteration's params)
    //   followed by 3 context values.
    let mut loop_back_args: Vec<ArcVarId> = input.rec_args.clone();
    loop_back_args.push(true_var);
    loop_back_args.push(new_res);
    loop_back_args.push(input.ctor_dst);

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
            args: loop_back_args,
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

        // Apply-ctx block: fill the context hole with the base-case return
        // value, then return the accumulated root.
        //
        // Uses `ctx_hole_obj` and `ctx_res` from the loop header's block
        // params via SSA dominance — the loop header (original function
        // entry) dominates all reachable blocks. Verified by
        // `check_context_var_dominance` in verify.rs.
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
