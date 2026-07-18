//! Loop-lowering rewrite for detected self-recursive tail calls.
//!
//! The original entry becomes a loop header, a new trampoline supplies its
//! initial parameters, and each tail call becomes a back-edge. For an
//! `Invoke`, safe continuation cleanup moves before the edge; the obsolete
//! normal and unwind blocks are left for `block_merge`. The pass runs after
//! RC elimination, when cleanup positions are final.

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
    report_transferred_result_dec_drop_toggle(
        std::env::var("ORI_DISABLE_TRMC_TRANSFERRED_RESULT_DEC_DROP").as_deref() == Ok("1"),
    )
});

fn report_transferred_result_dec_drop_toggle(disabled: bool) -> bool {
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_TRMC_TRANSFERRED_RESULT_DEC_DROP",
            effect = "retain RC ops on eliminated recursive Invoke results",
            "ablation toggle fired"
        );
    }
    disabled
}

fn transferred_result_dec_drop_disabled() -> bool {
    *TRMC_TRANSFERRED_RESULT_DEC_DROP_DISABLED
}

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

    // Header parameters need fresh IDs; reusing function parameters would make
    // the trampoline look self-referential and trigger incorrect invariant-
    // parameter elimination.
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

    // Rewrite sites before prepending header bindings, which would invalidate
    // detected instruction indices.
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

    // Bridge fresh header parameters back to the original variables after all
    // index-sensitive rewrites.
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
    // Preserve following decrements because they clean the current iteration
    // before the back edge; detection guarantees the removed instruction is Apply.
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
        // The loop back edge removes the invoke result; any moved RC operation
        // on it would violate RL34 and use an undefined value.
        let result_dst = *dst;

        // Move normal-path cleanup into the call block. Extend body and spans
        // independently so mismatched arrays are never silently truncated.
        let mut normal_body: Vec<ArcInstr> = func.blocks[normal_idx].body.drain(..).collect();
        let mut normal_spans: Vec<Option<ori_ir::Span>> =
            func.spans[normal_idx].drain(..).collect();

        // Remove RC operations on the eliminated invoke result in body/span
        // lockstep; the ablation restores move-everything behavior for bisection.
        if !transferred_result_dec_drop_disabled() {
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

#[cfg(test)]
mod toggle_tests {
    #[test]
    fn transferred_result_dec_drop_toggle_reports_effect() {
        crate::test_helpers::assert_ablation_env_event(
            concat!(
                module_path!(),
                "::transferred_result_dec_drop_toggle_reports_effect"
            ),
            "ORI_DISABLE_TRMC_TRANSFERRED_RESULT_DEC_DROP",
            "retain RC ops on eliminated recursive Invoke results",
            super::transferred_result_dec_drop_disabled,
        );
    }
}
