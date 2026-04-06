//! Terminator RC emission for the unified realization pipeline.
//!
//! Contains [`emit_terminator_rc`] (Phase C), which handles terminator uses —
//! emitting `RcInc` for live-at-exit variables and `RcDec` for Branch/Switch
//! scrutinees that are not ownership-transferred.
//!
//! The legacy body forward walk (Phase B) has been removed — body RC emission
//! is now handled by `realize/walk.rs` via the unified `decide()` surface.

use rustc_hash::FxHashMap;

use crate::ir::{ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, ValueRepr};

use super::helpers::{is_live_at_exit, is_owned_at_entry, BlockCtx};
use super::rc_strategy;

/// Phase C: handle terminator uses and non-transfer `RcDec`.
pub(crate) fn emit_terminator_rc(
    ctx: &BlockCtx<'_>,
    block_idx: usize,
    mut uses_so_far: FxHashMap<ArcVarId, usize>,
    new_body: &mut Vec<ArcInstr>,
) {
    let terminator = &ctx.func.blocks[block_idx].terminator;

    // For Invoke/InvokeIndirect terminators, emit RcInc for project-borrowed
    // variables at owned argument positions. Same logic as the body instruction
    // fix in realize/walk.rs — Project-derived variables passed to owned
    // parameters need RcInc because the callee stores the data (creating
    // a new reference) but Project didn't increment RC.
    match terminator {
        ArcTerminator::Invoke {
            args,
            arg_ownership,
            ..
        }
        | ArcTerminator::InvokeIndirect {
            args,
            arg_ownership,
            ..
        } => {
            for (pos, &var) in args.iter().enumerate() {
                let is_owned = arg_ownership
                    .get(pos)
                    .is_some_and(|o| *o == ArgOwnership::Owned);
                if is_owned
                    && ctx.project_borrowed_defs.contains(&var)
                    && ctx.func.var_reprs[var.index()] != ValueRepr::Scalar
                {
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
        _ => {}
    }

    // RcInc for terminator uses with future (exit) liveness.
    for var in terminator.used_vars() {
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
