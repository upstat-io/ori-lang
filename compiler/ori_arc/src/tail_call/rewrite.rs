//! Loop-lowering rewrite for detected self-recursive tail calls.
//!
//! Transforms `Apply @self(args) → Jump(merge, [result])` patterns into
//! `Jump(header, args)` back-edges, converting tail recursion into loops.
//!
//! # Algorithm
//!
//! 1. The original entry block becomes the **loop header** — block params
//!    are added matching the function's parameters.
//! 2. A **trampoline** block is created as the new function entry — it
//!    jumps to the header with the original parameter values.
//! 3. Each tail call site's `Apply` is removed; the terminator is changed
//!    to `Jump(header, call_args)` — the loop back-edge.
//! 4. `RcDec` operations between the (removed) `Apply` and the terminator
//!    remain in place — they clean up the current iteration's values
//!    before the next iteration begins.
//! 5. Merge blocks that lose all predecessors become unreachable and are
//!    cleaned up by `block_merge` (which runs after this pass).
//!
//! # Pipeline placement
//!
//! Runs immediately after [`detect_tail_calls`](super::detect_tail_calls),
//! AFTER `rc_elim` and BEFORE `block_merge`.

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator};

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

    // Step 1: Add block params to the header (original entry) matching
    // the function's parameters. On each loop iteration, the block params
    // rebind the parameter variables to the new argument values.
    let block_params: Vec<_> = func.params.iter().map(|p| (p.var, p.ty)).collect();
    func.blocks[header_idx].params = block_params;

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
    for site in &tail_calls {
        let block_idx = site.call_block.index();
        let instr_idx = site.call_instr_idx;

        // Extract the recursive call's arguments.
        let apply_args = match &func.blocks[block_idx].body[instr_idx] {
            ArcInstr::Apply { args, .. } => args.clone(),
            other => {
                tracing::warn!(
                    ?other,
                    block = ?site.call_block,
                    instr_idx,
                    "expected Apply at tail call site"
                );
                continue;
            }
        };

        // Remove the Apply instruction. RcDec operations that followed it
        // remain in place — they clean up the current iteration's values
        // before the back-edge jump starts the next iteration.
        func.blocks[block_idx].body.remove(instr_idx);
        if instr_idx < func.spans[block_idx].len() {
            func.spans[block_idx].remove(instr_idx);
        }

        // Replace the terminator with a back-edge to the loop header,
        // passing the recursive call's arguments as the next iteration's
        // parameter values.
        func.blocks[block_idx].terminator = ArcTerminator::Jump {
            target: header_id,
            args: apply_args,
        };
    }

    tracing::debug!(count = tail_calls.len(), "tail call loop lowering complete");
}
