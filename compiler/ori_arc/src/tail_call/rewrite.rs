//! Loop-lowering rewrite for detected self-recursive tail calls.
//!
//! Transforms tail-recursive calls into `Jump(header, args)` back-edges,
//! converting tail recursion into loops. Handles both body `Apply`
//! instructions and `Invoke` terminators (user function calls).
//!
//! # Algorithm
//!
//! 1. The original entry block becomes the **loop header** — block params
//!    are added matching the function's parameters.
//! 2. A **trampoline** block is created as the new function entry — it
//!    jumps to the header with the original parameter values.
//! 3. Each tail call site is rewritten:
//!    - **Apply**: The `Apply` instruction is removed from the body; the
//!      terminator is changed to `Jump(header, call_args)`.
//!    - **Invoke**: The `Invoke` terminator is replaced with
//!      `Jump(header, call_args)`. `RcDec` ops from the normal continuation
//!      block are moved into the call block before the jump. The normal
//!      and unwind blocks become dead and are cleaned up by `block_merge`.
//! 4. Merge blocks that lose all predecessors become unreachable and are
//!    cleaned up by `block_merge` (which runs after this pass).
//!
//! # Pipeline placement
//!
//! Runs immediately after [`detect_tail_calls`](super::detect_tail_calls),
//! AFTER `rc_elim` and BEFORE `block_merge`.

use std::sync::LazyLock;

use super::TailCallKind;
use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

/// `ORI_DISABLE_TRMC_TRANSFERRED_RESULT_DEC_DROP=1` restores the base
/// behavior of moving EVERY normal-continuation RC op into the call block when
/// a recursive `Invoke` tail call is rewritten to a loop back-edge — including
/// the RC ops on the Invoke's now-eliminated result var. Default (unset): drop
/// the RC ops whose operand is the eliminated result var (a forbidden post-call
/// dec on the transferred tail-call result; the result is never materialized
/// post-rewrite, so the op is a use-before-def). Spec: Annex E §AIMS RL-34
/// (`RL34_never_post_call_dec`).
static TRMC_TRANSFERRED_RESULT_DEC_DROP_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_TRMC_TRANSFERRED_RESULT_DEC_DROP").as_deref() == Ok("1")
});

/// Rewrite detected tail calls as loop back-edges.
///
/// Consumes the function's `tail_calls` annotations (populated by
/// [`detect_tail_calls`](super::detect_tail_calls)) and rewrites
/// the ARC IR to use loops instead of recursive calls.
///
/// After this pass, self-recursive tail calls are replaced by
/// `Jump(header, new_args)` back-edges, achieving O(1) stack space.
/// The function's `tail_calls` field is emptied (consumed).
#[tracing::instrument(skip_all, fields(func = ?func.name))]
pub(crate) fn rewrite_tail_calls(func: &mut ArcFunction) {
    let tail_calls = std::mem::take(&mut func.tail_calls);
    if tail_calls.is_empty() {
        return;
    }

    let header_id = func.entry;
    let header_idx = header_id.index();

    // Step 1: Add block params to the header using FRESH variable IDs.
    //
    // We must NOT reuse the function's parameter ArcVarIds here. If we did,
    // the trampoline's `Jump header [v0, v1]` would pass `v0` to a block
    // param also named `v0`. Block merge Phase 7 (invariant param elimination)
    // would see `arg == param_var` and treat the trampoline as a self-referencing
    // back-edge, incorrectly eliminating the param.
    //
    // With fresh IDs, Phase 7 sees distinct vars from both predecessors and
    // correctly identifies the params as non-invariant.
    let params: Vec<_> = func.params.iter().map(|p| (p.var, p.ty)).collect();
    let block_params: Vec<_> = params
        .iter()
        .map(|&(source, ty)| {
            let fresh = func.fresh_var_like_typed(source, ty);
            (fresh, ty)
        })
        .collect();
    func.blocks[header_idx].params.clone_from(&block_params);

    // Step 2: Create a trampoline block as the new function entry.
    // It simply jumps to the header with the original parameter values,
    // starting the first "iteration" of the loop.
    let trampoline_id = func.next_block_id();
    let param_vars: Vec<_> = func.params.iter().map(|p| p.var).collect();
    func.push_block(ArcBlock {
        id: trampoline_id,
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Jump {
            target: header_id,
            args: param_vars,
        },
    });
    func.entry = trampoline_id;

    // Step 3: Rewrite each tail call site.
    //
    // This must happen BEFORE prepending Let bindings to the header (Step 4),
    // because Apply tail calls in the header block use `instr_idx` from
    // detection — prepending would shift those indices.
    for site in &tail_calls {
        let block_idx = site.call_block.index();

        match &site.kind {
            TailCallKind::Apply { instr_idx } => {
                rewrite_apply_site(func, block_idx, *instr_idx, header_id, site);
            }
            TailCallKind::Invoke => {
                rewrite_invoke_site(func, block_idx, header_id);
            }
        }
    }

    // Step 4: Prepend Let bindings that define the original param vars from
    // the fresh block params. The header body references the original vars,
    // so these bindings bridge fresh block params → original names.
    //
    // Done after Step 3 so that tail call instr_idx values remain valid.
    let mut let_bindings: Vec<ArcInstr> = Vec::with_capacity(block_params.len());
    for (i, param) in func.params.iter().enumerate() {
        let_bindings.push(ArcInstr::Let {
            dst: param.var,
            ty: param.ty,
            value: ArcValue::Var(block_params[i].0),
        });
    }
    let original_body = std::mem::take(&mut func.blocks[header_idx].body);
    func.blocks[header_idx].body = let_bindings;
    func.blocks[header_idx].body.extend(original_body);

    // Prepend None spans for the synthetic Let bindings.
    let original_spans = std::mem::take(&mut func.spans[header_idx]);
    func.spans[header_idx] = vec![None; block_params.len()];
    func.spans[header_idx].extend(original_spans);

    tracing::debug!(count = tail_calls.len(), "tail call loop lowering complete");
}

/// Rewrite a body `Apply` tail call site as a loop back-edge.
fn rewrite_apply_site(
    func: &mut ArcFunction,
    block_idx: usize,
    instr_idx: usize,
    header_id: crate::ir::ArcBlockId,
    site: &super::TailCallSite,
) {
    // Remove the Apply instruction and extract its args. The detection
    // pass guarantees this is an Apply — the else branch is defensive.
    // RcDec operations that followed it remain in place — they clean up
    // the current iteration's values before the back-edge jump.
    let apply_args = match func.blocks[block_idx].body.remove(instr_idx) {
        ArcInstr::Apply { args, .. } => args,
        other => {
            tracing::warn!(
                ?other,
                block = ?site.call_block,
                instr_idx,
                "expected Apply at tail call site — re-inserting"
            );
            func.blocks[block_idx].body.insert(instr_idx, other);
            return;
        }
    };
    if instr_idx < func.spans[block_idx].len() {
        func.spans[block_idx].remove(instr_idx);
    }

    // Replace the terminator with a back-edge to the loop header.
    func.blocks[block_idx].terminator = ArcTerminator::Jump {
        target: header_id,
        args: apply_args,
    };
}

/// Rewrite an `Invoke` terminator tail call site as a loop back-edge.
///
/// The Invoke's normal continuation block may contain `RcDec` instructions
/// for iteration cleanup. These are moved into the call block before the
/// back-edge jump so they execute at the end of each iteration.
fn rewrite_invoke_site(func: &mut ArcFunction, block_idx: usize, header_id: crate::ir::ArcBlockId) {
    // Extract args from the Invoke terminator.
    let invoke_args = if let ArcTerminator::Invoke {
        args, normal, dst, ..
    } = &func.blocks[block_idx].terminator
    {
        let normal_idx = normal.index();
        let args = args.clone();
        // The Invoke's result var — eliminated by this rewrite (the recursive
        // call is replaced by the loop back-edge, so the result is never
        // materialized). Any RC op on it in the moved normal-continuation body
        // is a forbidden post-call dec on the transferred tail-call result
        // (RL34_never_post_call_dec) that would dangle as a use-before-def.
        let result_dst = *dst;

        // Move the normal continuation block's body into the call block.
        // These clean up the current iteration's values before the next
        // iteration begins. The body and spans are moved as PARALLEL arrays
        // (independent extends preserve any pre-existing length relationship —
        // a zip would silently truncate when the arrays differ in length).
        let mut normal_body: Vec<ArcInstr> = func.blocks[normal_idx].body.drain(..).collect();
        let mut normal_spans: Vec<Option<ori_ir::Span>> =
            func.spans[normal_idx].drain(..).collect();

        // Drop every RC op whose subject is the Invoke's now-eliminated result:
        // the result is never materialized after the loop-back rewrite, so a
        // post-call dec on it is forbidden (RL34_never_post_call_dec) and would
        // dangle as a use-before-def. Index-aligned removal keeps the body /
        // span arrays in lockstep. `ORI_DISABLE_TRMC_TRANSFERRED_RESULT_DEC_DROP=1`
        // restores the base move-everything behavior for bisection.
        if !*TRMC_TRANSFERRED_RESULT_DEC_DROP_DISABLED {
            let mut i = 0;
            while i < normal_body.len() {
                if rc_op_on_var(&normal_body[i], result_dst) {
                    tracing::trace!(
                        target: "ori_arc::tail_call",
                        result_dst = result_dst.index(),
                        instr = ?normal_body[i],
                        "TRMC rewrite: dropping eliminated-Invoke-result RC op (RL-34)"
                    );
                    normal_body.remove(i);
                    if i < normal_spans.len() {
                        normal_spans.remove(i);
                    }
                } else {
                    i += 1;
                }
            }
        }

        func.blocks[block_idx].body.extend(normal_body);
        func.spans[block_idx].extend(normal_spans);

        args
    } else {
        tracing::warn!(
            block_idx,
            "expected Invoke terminator at tail call site — skipping"
        );
        return;
    };

    // Replace the Invoke terminator with a back-edge to the loop header.
    func.blocks[block_idx].terminator = ArcTerminator::Jump {
        target: header_id,
        args: invoke_args,
    };
}

/// True iff `instr` is an RC / burden op (realized or burden-spelled) whose
/// subject var is `var`. Used to drop the eliminated-Invoke-result RC ops when
/// a recursive tail call is rewritten to a loop back-edge: the result is never
/// materialized post-rewrite, so a dec on it is a forbidden post-call dec that
/// dangles as a use-before-def. Spec: Annex E §AIMS RL-34.
fn rc_op_on_var(instr: &ArcInstr, var: ArcVarId) -> bool {
    match instr {
        ArcInstr::RcInc { var: v, .. }
        | ArcInstr::RcDec { var: v, .. }
        | ArcInstr::RcDecPartial { var: v, .. }
        | ArcInstr::RcDecVariant { var: v }
        | ArcInstr::BurdenInc { var: v }
        | ArcInstr::BurdenDec { var: v }
        | ArcInstr::BurdenDecPartial { var: v, .. }
        | ArcInstr::BurdenDecVariant { var: v } => *v == var,
        ArcInstr::RcDecField { base, .. } | ArcInstr::BurdenDecField { base, .. } => *base == var,
        _ => false,
    }
}
