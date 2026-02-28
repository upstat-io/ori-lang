//! Core RC insertion logic — the backward-walk Perceus algorithm.
//!
//! Contains the two entry points ([`insert_rc_ops`] for tests,
//! [`insert_rc_ops_with_ownership`] for production) and the shared
//! inner implementation that processes each block.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::liveness::BlockLiveness;
use crate::ownership::{DerivedOwnership, Ownership};
use crate::ArcClassification;

#[cfg(test)]
use super::compute_borrows;
use super::{is_borrowing_instr, needs_rc_trackable, rc_strategy, RcContext};

/// Insert `RcInc`/`RcDec` operations into an ARC IR function.
///
/// Modifies `func.blocks` in-place, inserting RC operations based on
/// liveness analysis. Borrowed parameters (from §06.2) and variables
/// derived from them skip RC tracking entirely.
///
/// # Arguments
///
/// * `func` — the ARC IR function to transform (mutated in place).
/// * `classifier` — type classifier for `needs_rc()` checks.
/// * `liveness` — precomputed liveness (from [`compute_liveness`](crate::compute_liveness)).
#[cfg(test)]
pub(crate) fn insert_rc_ops(
    func: &mut crate::ir::ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
) {
    // Precondition: RC insertion should run on fresh IR with no existing RC ops.
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. })),
        "insert_rc_ops: IR already contains RcInc/RcDec — pipeline ordering error"
    );

    tracing::debug!(function = func.name.raw(), "inserting RC operations");

    // Collect borrowed function parameters.
    let borrowed_params: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();

    let entry_idx = func.entry.index();
    let num_blocks = func.blocks.len();

    // Precompute Invoke dst definitions for each normal successor.
    // See liveness.rs `collect_invoke_defs` — same concept: an Invoke's
    // dst is defined at the normal successor's entry, like a block param.
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    // Collect per-block borrow sets for reuse by insert_edge_cleanup,
    // avoiding the redundant recomputation that compute_global_borrows
    // would perform.
    let mut per_block_borrows: Vec<FxHashSet<ArcVarId>> = Vec::with_capacity(num_blocks);

    for block_idx in 0..num_blocks {
        let borrows = compute_borrows(&func.blocks[block_idx], &borrowed_params);
        per_block_borrows.push(borrows);

        let (new_body, new_spans) = {
            let ctx = RcContext {
                func,
                classifier,
                pool: None, // test-only path — no Pool available
                borrowed_params: &borrowed_params,
                borrows: &per_block_borrows[block_idx],
                sigs: None,
                block_live_out: None,
            };
            process_block_rc(
                &ctx,
                block_idx,
                &liveness.live_out[block_idx],
                &invoke_defs,
                block_idx == entry_idx,
            )
        };

        func.blocks[block_idx].body = new_body;
        func.spans[block_idx] = new_spans;
    }

    // Step 5: Edge cleanup
    //
    // After per-block RC insertion, handle "stranded" variables that are
    // live at a predecessor's exit but not needed by a successor.
    // See `insert_edge_cleanup` for details.
    //
    // Build global borrow set from pre-collected per-block sets (avoids
    // redundant recomputation via compute_global_borrows).
    let global_borrows: FxHashSet<ArcVarId> = per_block_borrows
        .into_iter()
        .flat_map(FxHashSet::into_iter)
        .collect();
    super::edge_cleanup::insert_edge_cleanup(
        func,
        classifier,
        liveness,
        &borrowed_params,
        &global_borrows,
        None, // test-only path — no Pool
    );
}

/// Insert `RcInc`/`RcDec` operations using global [`DerivedOwnership`] analysis.
///
/// Enhanced version of [`insert_rc_ops`] that uses the whole-function
/// `DerivedOwnership` vector (from [`infer_derived_ownership`](crate::borrow::infer_derived_ownership))
/// instead of per-block `compute_borrows`. This captures cross-block borrow
/// propagation that the per-block approach misses.
///
/// When a variable derived from a borrowed parameter flows across a block
/// boundary (e.g., defined in B0 but used in B1), the per-block approach
/// loses track and treats it as owned in B1 — potentially omitting the
/// `RcInc` needed at owned positions. The `DerivedOwnership` vector has
/// global knowledge, ensuring correct RC ops in all blocks.
///
/// With `sigs`, also performs closure capture analysis (Step 2.4):
/// `PartialApply` captures of borrowed-derived vars at `Borrowed` callee
/// positions skip `RcInc` when the closure doesn't escape the block.
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn insert_rc_ops_with_ownership(
    func: &mut crate::ir::ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    ownership: &[DerivedOwnership],
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    pool: &ori_types::Pool,
) {
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. })),
        "insert_rc_ops_with_ownership: IR already contains RcInc/RcDec"
    );

    tracing::debug!(
        function = func.name.raw(),
        "inserting RC operations (ownership-enhanced)"
    );

    let borrowed_params: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();

    // Global borrow set from DerivedOwnership — covers cross-block propagation.
    let global_borrows: FxHashSet<ArcVarId> = ownership
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, DerivedOwnership::BorrowedFrom(_)))
        .map(|(i, _)| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR var counts fit in u32"
            )]
            ArcVarId::new(i as u32)
        })
        .collect();

    let entry_idx = func.entry.index();
    let num_blocks = func.blocks.len();
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    for block_idx in 0..num_blocks {
        let (new_body, new_spans) = {
            let ctx = RcContext {
                func,
                classifier,
                pool: Some(pool),
                borrowed_params: &borrowed_params,
                borrows: &global_borrows,
                sigs: Some(sigs),
                block_live_out: Some(&liveness.live_out[block_idx]),
            };
            process_block_rc(
                &ctx,
                block_idx,
                &liveness.live_out[block_idx],
                &invoke_defs,
                block_idx == entry_idx,
            )
        };

        func.blocks[block_idx].body = new_body;
        func.spans[block_idx] = new_spans;
    }

    super::edge_cleanup::insert_edge_cleanup(
        func,
        classifier,
        liveness,
        &borrowed_params,
        &global_borrows,
        Some(pool),
    );
}

/// Process a single block for RC insertion, returning the new body and spans.
///
/// Shared inner implementation for [`insert_rc_ops`] and
/// [`insert_rc_ops_with_ownership`]. Performs the backward walk over
/// instructions, inserting `RcInc`/`RcDec` based on liveness and borrowing.
fn process_block_rc(
    ctx: &RcContext<'_>,
    block_idx: usize,
    live_out: &FxHashSet<ArcVarId>,
    invoke_defs: &FxHashMap<crate::ir::ArcBlockId, Vec<ArcVarId>>,
    is_entry: bool,
) -> (Vec<ArcInstr>, Vec<Option<ori_ir::Span>>) {
    let mut live = live_out.clone();
    let mut new_body: Vec<ArcInstr> = Vec::new();
    let mut new_spans: Vec<Option<ori_ir::Span>> = Vec::new();

    let block = &ctx.func.blocks[block_idx];
    let old_spans = &ctx.func.spans[block_idx];

    // Step 1: Process terminator uses
    process_terminator_uses(
        &block.terminator,
        &mut live,
        &mut new_body,
        &mut new_spans,
        ctx,
    );

    // Step 2: Backward body pass
    for (instr_idx, instr) in block.body.iter().enumerate().rev() {
        let span = if instr_idx < old_spans.len() {
            old_spans[instr_idx]
        } else {
            None
        };

        // Definition: if dst is RC'd, non-borrowed, and not live → dead def, emit Dec.
        if let Some(dst) = instr.defined_var() {
            if needs_rc_trackable(dst, ctx) && !live.remove(&dst) {
                new_body.push(ArcInstr::RcDec {
                    var: dst,
                    strategy: rc_strategy(ctx, dst),
                });
                new_spans.push(None);
            }
        }

        // Borrowing uses: emit Dec for last-use RC-typed args.
        //
        // PrimOps and external calls BORROW their args (read without consuming).
        // In the Perceus model, only consuming uses (Invoke, Apply, Construct,
        // PartialApply) transfer ownership. Borrowing uses leave the caller
        // responsible for freeing the value.
        //
        // At this point in the backward walk, `live` reflects variables needed
        // by later instructions (in forward order). Args NOT in `live` are at
        // their last use. In the reversed body, these Decs appear BEFORE the
        // instruction → AFTER the borrowing use in forward order.
        //
        // Ref: Lean 4 `src/Lean/Compiler/IR/RC.lean` — primitive ops are
        // non-consuming; Dec inserted at last borrowing use.
        if is_borrowing_instr(instr, ctx) {
            for &arg in &instr.used_vars() {
                if needs_rc_trackable(arg, ctx) && !live.contains(&arg) {
                    new_body.push(ArcInstr::RcDec {
                        var: arg,
                        strategy: rc_strategy(ctx, arg),
                    });
                    new_spans.push(None);
                }
            }
        }

        new_body.push(instr.clone());
        new_spans.push(span);

        process_instruction_uses(instr, &mut live, &mut new_body, &mut new_spans, ctx);
    }

    // Step 3: Block parameters
    for &(param_var, _ty) in block.params.iter().rev() {
        if needs_rc_trackable(param_var, ctx) && !live.remove(&param_var) {
            new_body.push(ArcInstr::RcDec {
                var: param_var,
                strategy: rc_strategy(ctx, param_var),
            });
            new_spans.push(None);
        }
    }

    // Step 3.5: Invoke dst definitions
    let block_id = ctx.func.blocks[block_idx].id;
    if let Some(dsts) = invoke_defs.get(&block_id) {
        for &dst in dsts.iter().rev() {
            if needs_rc_trackable(dst, ctx) && !live.remove(&dst) {
                new_body.push(ArcInstr::RcDec {
                    var: dst,
                    strategy: rc_strategy(ctx, dst),
                });
                new_spans.push(None);
            }
        }
    }

    // Step 4: Entry block function params
    if is_entry {
        for param in ctx.func.params.iter().rev() {
            if param.ownership == Ownership::Owned
                && ctx.classifier.needs_rc(param.ty)
                && !live.remove(&param.var)
            {
                new_body.push(ArcInstr::RcDec {
                    var: param.var,
                    strategy: rc_strategy(ctx, param.var),
                });
                new_spans.push(None);
            }
        }
    }

    new_body.reverse();
    new_spans.reverse();

    (new_body, new_spans)
}

/// Process terminator uses for RC insertion.
///
/// Each variable used in the terminator: if it needs RC, is not borrowed,
/// and is already in the live set → emit `RcInc`. Add to live set.
///
/// For `Return`, the returned variable is treated as an owned position
/// for borrowed-derived vars (transfer to caller requires Inc).
///
/// **Borrowing Invoke args**: An Invoke arg is treated as borrowing (not
/// consuming) when:
/// - The callee is external (C runtime `ori_*` — borrows all args), OR
/// - The callee's `AnnotatedSig` marks the corresponding param as `Borrowed`
///
/// Borrowing args: add to `live` (keep alive through call) but no `RcInc`.
/// The companion post-pass [`insert_external_invoke_cleanup`](super::edge_cleanup::insert_external_invoke_cleanup)
/// handles the `RcDec` for args whose last use is a borrowing Invoke position.
///
/// Ref: Lean 4 `src/Lean/Compiler/IR/RC.lean` — caller checks callee param
/// ownership to decide Inc/Dec at call sites.
fn process_terminator_uses(
    terminator: &ArcTerminator,
    live: &mut FxHashSet<ArcVarId>,
    new_body: &mut Vec<ArcInstr>,
    new_spans: &mut Vec<Option<ori_ir::Span>>,
    ctx: &RcContext<'_>,
) {
    // Determine which terminator positions are "owned" for borrowed-derived vars.
    let is_return = matches!(terminator, ArcTerminator::Return { .. });

    // Read per-arg borrowing info from the Invoke's pre-computed `arg_ownership`.
    // Populated by `annotate_arg_ownership` before RC insertion.
    let invoke_arg_ownership = match terminator {
        ArcTerminator::Invoke { arg_ownership, .. } => arg_ownership.as_slice(),
        _ => &[],
    };

    for (pos, var) in terminator.used_vars().into_iter().enumerate() {
        if !ctx.classifier.needs_rc(ctx.func.var_type(var)) {
            continue;
        }

        // Borrowed params: completely skip all RC tracking.
        if ctx.borrowed_params.contains(&var) {
            if is_return {
                // Returning a borrowed param transfers ownership to caller.
                // Must Inc even for a borrowed param.
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy: rc_strategy(ctx, var),
                });
                new_spans.push(None);
            }
            continue;
        }

        // Borrowed-derived vars: Inc only in owned positions.
        if ctx.borrows.contains(&var) {
            if is_return {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy: rc_strategy(ctx, var),
                });
                new_spans.push(None);
            }
            continue;
        }

        // Borrowing Invoke arg: add to live (keeps value alive through the
        // call) but never Inc (callee borrows, doesn't consume).
        // The post-pass inserts Dec for last-use args.
        if invoke_arg_ownership.get(pos).copied() == Some(ArgOwnership::Borrowed) {
            live.insert(var);
            continue;
        }

        // Normal (non-borrowed) var — standard Perceus ownership transfer.
        if live.contains(&var) {
            new_body.push(ArcInstr::RcInc {
                var,
                count: 1,
                strategy: rc_strategy(ctx, var),
            });
            new_spans.push(None);
        }
        live.insert(var);
    }
}

/// Process uses of a single instruction for RC insertion.
///
/// For each used variable:
/// - If it's a borrowed param → skip entirely.
/// - If it's a borrowed-derived var in a non-owned position → skip.
/// - If it's a borrowed-derived var in an owned position → emit `RcInc`.
/// - If it's a normal var already in `live` → emit `RcInc` (multi-use).
/// - Add to `live` (unless borrowed).
///
/// "Owned positions" are instruction slots where the value will be stored
/// on the heap: `Construct` args, `PartialApply` args, `Apply`/`ApplyIndirect`
/// args (conservative for unknown callees).
///
/// **External Apply handling**: Like external Invoke, C runtime `Apply` calls
/// borrow their args. Args are added to `live` (kept alive through the call)
/// but never get `RcInc` (external callee doesn't consume/Dec).
fn process_instruction_uses(
    instr: &ArcInstr,
    live: &mut FxHashSet<ArcVarId>,
    new_body: &mut Vec<ArcInstr>,
    new_spans: &mut Vec<Option<ori_ir::Span>>,
    ctx: &RcContext<'_>,
) {
    // Borrowing uses: PrimOps and external calls don't consume their args.
    let is_borrowing = is_borrowing_instr(instr, ctx);

    // Collect unique vars and count occurrences to handle duplicate args.
    // For example, `Apply { args: [x, x] }` should emit exactly 1 Inc
    // (x appears twice, but one use is "free" and the second is Inc).
    let used = instr.used_vars();
    let mut seen = FxHashSet::default();

    for (pos, &var) in used.iter().enumerate() {
        if !ctx.classifier.needs_rc(ctx.func.var_type(var)) {
            continue;
        }

        // Borrowed params: completely skip all RC tracking.
        // No Inc, no Dec, not added to live set.
        if ctx.borrowed_params.contains(&var) {
            continue;
        }

        // Borrowed-derived vars: only emit Inc if in an owned position.
        if ctx.borrows.contains(&var) {
            if !is_borrowing
                && instr.is_owned_position(pos)
                && !is_borrowed_capture(instr, pos, ctx)
            {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy: rc_strategy(ctx, var),
                });
                new_spans.push(None);
            }
            continue;
        }

        // Borrowing uses (PrimOp, external Apply): add to live (keeps value
        // alive through the operation) but never Inc (operation borrows,
        // doesn't consume). The backward walk emits Dec for last-use args
        // (see process_block_rc's `is_borrowing_instr` check).
        if is_borrowing {
            live.insert(var);
            continue;
        }

        // Normal (non-borrowed) var.
        if !seen.insert(var) {
            // Duplicate arg in the same instruction — already handled below.
            // The first occurrence either adds to live or emits Inc.
            // The second occurrence always needs Inc.
            new_body.push(ArcInstr::RcInc {
                var,
                count: 1,
                strategy: rc_strategy(ctx, var),
            });
            new_spans.push(None);
            continue;
        }

        if live.contains(&var) {
            // Already live → multi-use, emit Inc.
            new_body.push(ArcInstr::RcInc {
                var,
                count: 1,
                strategy: rc_strategy(ctx, var),
            });
            new_spans.push(None);
        }
        live.insert(var);
    }
}

/// Check if a `PartialApply` capture position is a borrowed callee parameter
/// and the closure doesn't escape the block.
///
/// When capturing a borrowed-derived variable into a closure, we normally need
/// `RcInc` because the closure stores the value. But if:
/// 1. The callee expects this parameter as `Borrowed` (won't store/escape it)
/// 2. The closure doesn't escape the current block (consumed locally)
///
/// ...then the Inc can be safely skipped. The captured value remains alive
/// through its borrow root (a function parameter with lifetime spanning the
/// entire function).
///
/// Follows Lean 4's `Borrow.lean` pattern for closure captures.
#[inline]
fn is_borrowed_capture(instr: &ArcInstr, pos: usize, ctx: &RcContext<'_>) -> bool {
    let (Some(sigs), Some(live_out)) = (ctx.sigs, ctx.block_live_out) else {
        return false;
    };

    let ArcInstr::PartialApply {
        dst, func: callee, ..
    } = instr
    else {
        return false;
    };

    // Closure escapes the block → must Inc for safety.
    if live_out.contains(dst) {
        return false;
    }

    // Callee's parameter at this position is Borrowed → skip Inc.
    sigs.get(callee)
        .and_then(|sig| sig.params.get(pos))
        .is_some_and(|p| p.ownership == Ownership::Borrowed)
}
