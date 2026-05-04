//! Terminator RC emission for the unified realization pipeline.
//!
//! Contains [`emit_terminator_rc`] (Phase C), which handles terminator uses —
//! emitting `RcInc` for variables that are live at exit, variables that
//! appear multiple times in the terminator's `used_vars()` multiset (each
//! additional owned transfer needs its own reference), and `RcDec` for
//! `Branch`/`Switch` scrutinees that are not ownership-transferred.
//!
//! The legacy body forward walk (Phase B) has been removed — body RC emission
//! is now handled by `realize/walk.rs` via the unified `decide()` surface.

use rustc_hash::FxHashMap;

use crate::ir::{ArcInstr, ArcTerminator, ArcVarId, ValueRepr};

use super::helpers::{is_live_at_exit, is_owned_at_entry, BlockCtx};
use super::rc_strategy;

/// Phase C: handle terminator uses and non-transfer `RcDec`.
pub(crate) fn emit_terminator_rc(
    ctx: &BlockCtx<'_>,
    block_idx: usize,
    new_body: &mut Vec<ArcInstr>,
) {
    let terminator = &ctx.func.blocks[block_idx].terminator;

    emit_invoke_project_borrowed_owned_incs(ctx, terminator, new_body);
    emit_terminator_duplicate_use_incs(ctx, terminator, new_body);
    emit_branch_switch_cond_dec(ctx, block_idx, new_body);
}

/// For `Invoke`/`InvokeIndirect` terminators, emit `RcInc` for
/// project-borrowed variables at owned argument positions. Same logic as the
/// body instruction fix in `realize/walk.rs` — `Project`-derived variables
/// passed to owned parameters need `RcInc` because the callee stores the data
/// (creating a new reference) but `Project` didn't increment RC.
fn emit_invoke_project_borrowed_owned_incs(
    ctx: &BlockCtx<'_>,
    terminator: &ArcTerminator,
    new_body: &mut Vec<ArcInstr>,
) {
    let (ArcTerminator::Invoke { args, .. } | ArcTerminator::InvokeIndirect { args, .. }) =
        terminator
    else {
        return;
    };

    for (pos, &var) in args.iter().enumerate() {
        let is_owned = terminator.is_owned_position(pos);
        if !is_owned
            || !ctx.project_borrowed_defs.contains(&var)
            || ctx.func.var_reprs[var.index()] == ValueRepr::Scalar
        {
            continue;
        }
        if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
            new_body.push(ArcInstr::RcInc {
                var,
                count: 1,
                strategy,
            });
        }
    }
}

/// `RcInc` for terminator uses: aggregate per distinct var.
///
/// Counts how many times each RC-managed var appears in the terminator's
/// `used_vars()` multiset. For `k` occurrences of var `v` with
/// `is_live_at_exit == L`, emits `(k + L).saturating_sub(1)` `RcInc`
/// operations (coalesced into a single `RcInc { count }` for efficiency).
///
/// Rationale: the baseline refcount provides one "free" transfer. Each
/// additional owned transfer of the same underlying reference needs its own
/// `RcInc`. If `v` is also live after this block (used in a successor),
/// one more `RcInc` hands the reference to the successor's liveness chain.
/// This is Phase C's dual of the body walk's per-use `has_future_use`
/// semantics in `realize/walk.rs::compute_has_future_use` — expressed here as
/// a terminator-local aggregated count because terminators are a single
/// emission point, not an instruction stream.
///
/// The predecessor to this function only checked `is_live_at_exit` and
/// emitted zero `RcInc` for e.g. `Jump { args: [v, v] }` with `v` dead at
/// exit, producing a latent double-free at the target block's per-param
/// `RcDec`.
fn emit_terminator_duplicate_use_incs(
    ctx: &BlockCtx<'_>,
    terminator: &ArcTerminator,
    new_body: &mut Vec<ArcInstr>,
) {
    #[cfg(debug_assertions)]
    let term_phase_start = new_body.len();

    let mut term_counts: FxHashMap<ArcVarId, usize> = FxHashMap::default();
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
        *term_counts.entry(var).or_insert(0) += 1;
    }

    // BUG-04-090 §05 Hypothesis D component #4 (Session H wiring):
    // unwind-block awareness for the apply-aliased gate.
    let is_unwind_block = matches!(
        ctx.func.blocks[ctx.blk.index()].terminator,
        crate::ir::ArcTerminator::Resume,
    );
    for (&var, &k) in &term_counts {
        // BUG-04-090 §05 Hypothesis D component #4 (Session H wiring):
        // suppress the terminator-duplicate-use Inc on vars whose
        // ownership is being transferred to an Apply/Invoke result via
        // `apply_result_aliases`. The Inc's purpose is unwind-cleanup
        // protection (the unwind block's dec needs a balancing inc); but
        // for apply-aliased args, the dec is also suppressed at the
        // unwind block (per the apply-aliased gate at `emit_dead_at_entry_decs`).
        // Symmetric suppression keeps the inc/dec pair balanced at zero.
        if super::should_suppress_apply_aliased_dec(ctx.state_map, var, is_unwind_block) {
            continue;
        }
        let live = usize::from(is_live_at_exit(ctx.state_map, ctx.blk, var));
        let count = (k + live).saturating_sub(1);
        if count == 0 {
            continue;
        }
        let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) else {
            continue;
        };
        // Terminator arg multiplicity is bounded by `terminator.used_vars()`
        // length (a `SmallVec<[ArcVarId; 4]>`) — always well under `u32::MAX`.
        // `try_from` documents the bound explicitly instead of relying on it.
        let count_u32 = u32::try_from(count).unwrap_or(u32::MAX);
        new_body.push(ArcInstr::RcInc {
            var,
            count: count_u32,
            strategy,
        });
    }

    #[cfg(debug_assertions)]
    assert_terminator_rc_balance(ctx, new_body, term_phase_start, &term_counts);
}

/// Debug-only invariant: emitted terminator-phase `RcInc` count per var
/// must equal the aggregated formula `(k + live).saturating_sub(1)`. Catches
/// any future regression that under- or over-emits at this site. Zero
/// release overhead (`#[cfg(debug_assertions)]` at call site).
#[cfg(debug_assertions)]
fn assert_terminator_rc_balance(
    ctx: &BlockCtx<'_>,
    new_body: &[ArcInstr],
    term_phase_start: usize,
    term_counts: &FxHashMap<ArcVarId, usize>,
) {
    let mut actual: FxHashMap<ArcVarId, u64> = FxHashMap::default();
    for instr in &new_body[term_phase_start..] {
        if let ArcInstr::RcInc { var, count, .. } = instr {
            *actual.entry(*var).or_insert(0) += u64::from(*count);
        }
    }
    // BUG-04-090 §05 Hypothesis D component #4 (Session H wiring):
    // unwind-block awareness mirrors the suppression-site formula.
    let is_unwind_block = matches!(
        ctx.func.blocks[ctx.blk.index()].terminator,
        crate::ir::ArcTerminator::Resume,
    );
    for (&var, &k) in term_counts {
        // Vars without an RC strategy emit nothing; skip the assertion.
        if rc_strategy(ctx.func, var, ctx.pool).is_none() {
            continue;
        }
        // BUG-04-090 §05 Hypothesis D component #4 (Session H wiring):
        // skip vars whose Inc was suppressed by the apply-aliased gate.
        // Mirrors the suppression formula in `emit_terminator_duplicate_use_incs`.
        if super::should_suppress_apply_aliased_dec(ctx.state_map, var, is_unwind_block) {
            continue;
        }
        let live = usize::from(is_live_at_exit(ctx.state_map, ctx.blk, var));
        let expected = (k + live).saturating_sub(1) as u64;
        let got = actual.get(&var).copied().unwrap_or(0);
        debug_assert_eq!(
            got,
            expected,
            "emit_terminator_rc: RC balance violated for var={var:?} at \
             block {}: expected {expected} RcInc(s), got {got} \
             (occurrences={k}, live_at_exit={live})",
            ctx.blk.raw()
        );
    }
}

/// `RcDec` for `Branch`/`Switch` scrutinee — read but not ownership-
/// transferred. `Return`/`Jump`/`Invoke` transfer ownership;
/// `Resume`/`Unreachable` have nothing to decrement.
fn emit_branch_switch_cond_dec(ctx: &BlockCtx<'_>, block_idx: usize, new_body: &mut Vec<ArcInstr>) {
    let cond = match &ctx.func.blocks[block_idx].terminator {
        ArcTerminator::Branch { cond, .. }
        | ArcTerminator::Switch {
            scrutinee: cond, ..
        } => *cond,
        _ => return,
    };

    if ctx.state_map.is_excluded(cond) || is_live_at_exit(ctx.state_map, ctx.blk, cond) {
        return;
    }
    if let Some(strategy) = rc_strategy(ctx.func, cond, ctx.pool) {
        // BUG-04-090 §05 Step 8: defensive parity gate. Branch/Switch
        // scrutinees are typically scalars (booleans/enums) which never
        // hit `transfers_through_return`, but the four-emitter coverage
        // rule (TPR-04-005) gates every RcDec emitter against the helper.
        let is_unwind_block = matches!(
            ctx.func.blocks[ctx.blk.index()].terminator,
            crate::ir::ArcTerminator::Resume
        );
        if super::should_suppress_return_transfer_dec(ctx, cond, ctx.blk, is_unwind_block) {
            return;
        }
        new_body.push(ArcInstr::RcDec {
            var: cond,
            strategy,
        });
    }
}

#[cfg(test)]
mod tests;
