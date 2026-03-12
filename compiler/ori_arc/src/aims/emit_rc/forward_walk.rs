//! Phase B and C forward walk for RC emission.
//!
//! - **Phase B** ([`emit_body_forward_walk`]): walks body instructions in
//!   order, emitting `RcInc` before each non-last use of owned variables and
//!   `RcDec` after the last use. Parent variables with borrowed children are
//!   deferred until all children are dead.
//! - **Phase C** ([`emit_terminator_rc`]): handles terminator uses, emitting
//!   `RcInc` for live-at-exit variables and `RcDec` for Branch/Switch
//!   scrutinees that are not ownership-transferred.

use rustc_hash::FxHashMap;

use crate::ir::{ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

use super::helpers::{
    is_consuming_primop, is_live_at_exit, is_owned_at_entry, is_ownership_transfer, BlockCtx,
    LastUse,
};
use super::rc_strategy;

/// Phase B: forward walk through body, emitting `RcInc`/`RcDec` around each
/// instruction. Returns the accumulated use counts for Phase C and any
/// deferred parent `RcDec` operations whose borrowed children are used in
/// the block terminator.
pub(super) fn emit_body_forward_walk(
    ctx: &BlockCtx<'_>,
    old_body: &[ArcInstr],
    new_body: &mut Vec<ArcInstr>,
) -> (FxHashMap<ArcVarId, usize>, Vec<(ArcVarId, RcStrategy)>) {
    let mut uses_so_far: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    // Deferred parent decs: parent vars whose RcDec was skipped because
    // a borrowed child has a later use. (var, strategy, effective_last_use).
    let mut deferred: Vec<(ArcVarId, RcStrategy, LastUse)> = Vec::new();

    for (instr_idx, instr) in old_body.iter().enumerate() {
        emit_pre_instr_incs(ctx, instr, instr_idx, &mut uses_so_far, new_body);
        new_body.push(instr.clone());
        emit_post_instr_decs(ctx, instr, instr_idx, new_body, &mut deferred);

        // Emit deferred parent decs whose children's last use is this instruction.
        deferred.retain(|&(var, strategy, effective_last)| {
            if effective_last == LastUse::Body(instr_idx)
                && !is_live_at_exit(ctx.state_map, ctx.blk, var)
            {
                new_body.push(ArcInstr::RcDec { var, strategy });
                return false;
            }
            true
        });
    }

    // Remaining deferred parents: children used in the terminator.
    let terminator_deferred = deferred
        .into_iter()
        .map(|(var, strategy, _)| (var, strategy))
        .collect();

    (uses_so_far, terminator_deferred)
}

/// Emit `RcInc` before each use in an instruction where a future use exists.
fn emit_pre_instr_incs(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    uses_so_far: &mut FxHashMap<ArcVarId, usize>,
    new_body: &mut Vec<ArcInstr>,
) {
    for var in instr.used_vars() {
        let owned = is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
            ctx.all_borrowed_defs,
        );
        if !owned {
            continue;
        }

        let count = uses_so_far.entry(var).or_insert(0);
        *count += 1;

        let has_future_use = if let Some(&(total_uses, last_use)) = ctx.use_info.get(&var) {
            let remaining_in_block = total_uses - *count;
            let live = is_live_at_exit(ctx.state_map, ctx.blk, var);
            remaining_in_block > 0
                || (matches!(last_use, LastUse::Terminator) && LastUse::Body(instr_idx) != last_use)
                || live
        } else {
            false
        };

        if has_future_use {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy,
                });
            }
        }
    }
}

/// Emit `RcDec` after an instruction for defined-but-dead variables and
/// variables whose last use was this instruction.
///
/// Parent variables with borrowed children that have later uses are deferred
/// rather than decremented immediately (the parent must outlive its children).
fn emit_post_instr_decs(
    ctx: &BlockCtx<'_>,
    instr: &ArcInstr,
    instr_idx: usize,
    new_body: &mut Vec<ArcInstr>,
    deferred: &mut Vec<(ArcVarId, RcStrategy, LastUse)>,
) {
    // RcDec for defined-but-dead variables.
    if let Some(dst) = instr.defined_var() {
        if !ctx.state_map.is_excluded(dst)
            && ctx.func.var_reprs[dst.index()] != crate::ir::ValueRepr::Scalar
            && !ctx.use_info.contains_key(&dst)
            && !is_live_at_exit(ctx.state_map, ctx.blk, dst)
        {
            if let Some(strategy) = rc_strategy(ctx.func, dst, ctx.pool) {
                new_body.push(ArcInstr::RcDec { var: dst, strategy });
            }
        }
    }

    // Skip operand drops for consuming PrimOp instructions (e.g., list/string
    // concat). These produce RcPtr results via COW runtime functions that
    // handle operand RC internally (realloc or copy+dec). Emitting separate
    // RcDec here would double-free.
    if is_consuming_primop(instr, ctx.func) {
        return;
    }

    // Skip last-use RcDec for alias assignments (`Let { dst, Var(src) }`).
    // Ownership transfers from `src` to `dst` — `dst` will have its own
    // RcDec when it dies. Emitting RcDec for `src` here would double-dec
    // the same refcount.
    if is_ownership_transfer(instr, ctx.func) {
        return;
    }

    // RcDec for variables whose last use was this instruction.
    for (pos, var) in instr.used_vars().into_iter().enumerate() {
        if !is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
            ctx.all_borrowed_defs,
        ) {
            continue;
        }
        // Skip RcDec for variables at owned positions in call instructions.
        // Ownership transfers to the callee — the callee handles the dec.
        if instr.is_owned_position(pos) {
            continue;
        }
        if let Some(&(_total, last_use)) = ctx.use_info.get(&var) {
            if last_use == LastUse::Body(instr_idx) && !is_live_at_exit(ctx.state_map, ctx.blk, var)
            {
                // Defer RcDec if this variable has borrowed children with
                // later uses. The parent must outlive all its children.
                if let Some(&child_last) = ctx.child_effective_last_use.get(&var) {
                    let child_is_later = match child_last {
                        LastUse::Body(c) => c > instr_idx,
                        LastUse::Terminator => true,
                    };
                    if child_is_later {
                        if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                            deferred.push((var, strategy, child_last));
                        }
                        continue;
                    }
                }

                if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                    new_body.push(ArcInstr::RcDec { var, strategy });
                }
            }
        }
    }
}

/// Phase C: handle terminator uses and non-transfer `RcDec`.
pub(super) fn emit_terminator_rc(
    ctx: &BlockCtx<'_>,
    block_idx: usize,
    mut uses_so_far: FxHashMap<ArcVarId, usize>,
    new_body: &mut Vec<ArcInstr>,
) {
    // RcInc for terminator uses with future (exit) liveness.
    for var in ctx.func.blocks[block_idx].terminator.used_vars() {
        if !is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
            ctx.all_borrowed_defs,
        ) {
            continue;
        }
        *uses_so_far.entry(var).or_insert(0) += 1;

        if is_live_at_exit(ctx.state_map, ctx.blk, var) {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy,
                });
            }
        }
    }

    // RcDec for Branch/Switch scrutinee — read but not ownership-transferred.
    // Return/Jump/Invoke transfer ownership; Resume/Unreachable have nothing.
    match &ctx.func.blocks[block_idx].terminator {
        ArcTerminator::Branch { cond, .. }
        | ArcTerminator::Switch {
            scrutinee: cond, ..
        } => {
            if !ctx.state_map.is_excluded(*cond) && !is_live_at_exit(ctx.state_map, ctx.blk, *cond)
            {
                if let Some(strategy) = rc_strategy(ctx.func, *cond, ctx.pool) {
                    new_body.push(ArcInstr::RcDec {
                        var: *cond,
                        strategy,
                    });
                }
            }
        }
        _ => {}
    }
}
