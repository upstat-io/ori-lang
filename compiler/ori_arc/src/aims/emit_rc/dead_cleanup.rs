//! Phase A and dead Invoke result cleanup for RC emission.
//!
//! - **Phase A** ([`emit_dead_at_entry_decs`]): emits `RcDec` for variables
//!   that are live at block entry, not used anywhere in the block, and dead
//!   at block exit.
//! - **Dead Invoke sweep** ([`emit_dead_invoke_dsts`]): scans Invoke
//!   terminators for result variables that escaped Phase A–C entirely (their
//!   backward demand was never propagated) and prepends the missing `RcDec`
//!   into the normal successor block.

use rustc_hash::FxHashSet;

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::Cardinality;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::helpers::{is_live_at_exit, is_owned_at_entry, BlockCtx};
use super::{block_id, rc_strategy};

/// Phase A: `RcDec` for variables live at entry, unused in block, dead at exit.
///
/// Two sources of dead-at-entry variables:
/// 1. Variables tracked by the backward analysis (`entry_states`) that have
///    non-Absent cardinality but are unused and dead at exit.
/// 2. Block parameters that generated no backward demand (absent from
///    `entry_states` entirely) — e.g., mutable-scope variables threaded
///    through loop exit blocks but never actually read. These are `RcPtr`
///    parameters that need cleanup even though the analysis didn't track them.
pub(crate) fn emit_dead_at_entry_decs(ctx: &BlockCtx<'_>, new_body: &mut Vec<ArcInstr>) {
    // Source 1: variables in entry_states.
    if let Some(entry_states) = ctx.state_map.block_entry_states(ctx.blk) {
        for (&var, &state) in entry_states {
            if state.is_scalar() || state.cardinality == Cardinality::Absent {
                continue;
            }
            // Use is_owned_at_entry to handle cross-block variables whose
            // access dimension is stuck at BOTTOM (Borrowed) due to backward
            // demand propagation not updating access.
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
            if ctx.use_info.contains_key(&var) || is_live_at_exit(ctx.state_map, ctx.blk, var) {
                continue;
            }
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcDec { var, strategy });
            }
        }
    }

    // Source 2: block parameters absent from entry_states.
    // These are parameters that generated no backward demand (never used in
    // this block or any successor). They still need RcDec if they carry
    // RC-managed values (e.g., mutable-scope list variables passed through
    // loop exit blocks).
    let entry_states = ctx.state_map.block_entry_states(ctx.blk);
    let block = &ctx.func.blocks[ctx.blk.index()];
    for &(param_var, _param_ty) in &block.params {
        // Skip if already handled by Source 1.
        if entry_states.is_some_and(|es| es.contains_key(&param_var)) {
            continue;
        }
        // Skip scalars and excluded variables.
        if ctx.state_map.is_excluded(param_var) {
            continue;
        }
        // Skip if the parameter is actually used in this block.
        if ctx.use_info.contains_key(&param_var) {
            continue;
        }
        // Skip if live at exit (used in successor blocks).
        if is_live_at_exit(ctx.state_map, ctx.blk, param_var) {
            continue;
        }
        // Skip iterator-element variables. These are borrowed from the
        // collection buffer — elem_dec_fn handles cleanup when the
        // collection is freed. Without this check, mutable-scope
        // threading of borrowed elements through inner loop exit blocks
        // generates spurious RcDec on the block param.
        if ctx.iter_element_defs.contains(&param_var) {
            continue;
        }
        if let Some(strategy) = rc_strategy(ctx.func, param_var, ctx.pool) {
            new_body.push(ArcInstr::RcDec {
                var: param_var,
                strategy,
            });
        }
    }
}

/// Sweep for dead Invoke result variables across all blocks.
///
/// An Invoke terminator's `dst` variable is "born" at the edge between the
/// predecessor (where the Invoke lives) and the successor blocks. The backward
/// analysis may never see demand for this variable (e.g., if the result is
/// unused or only used in the defining block), causing it to be absent from
/// all entry/exit state maps. Phases A–C only process variables they find in
/// `entry_states` or `use_info`, so they miss these orphaned definitions entirely.
///
/// This function scans every Invoke terminator and checks whether its `dst`
/// variable has an `RcDec` in the normal successor block. If not, and the
/// variable is an `RcPointer`, it prepends one. The same check is done for
/// the unwind successor.
pub(crate) fn emit_dead_invoke_dsts(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) {
    // Collect (successor_block_idx, var, strategy) tuples to avoid borrowing
    // func mutably while iterating.
    let mut pending_decs = Vec::new();

    for block in &func.blocks {
        let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator else {
            continue;
        };

        // Skip scalars and borrowed defs.
        if state_map.is_excluded(*dst) || all_borrowed_defs.contains(dst) {
            continue;
        }

        let Some(strategy) = rc_strategy(func, *dst, pool) else {
            continue;
        };

        // Only check the NORMAL successor. On the unwind path, the Invoke's
        // dst variable is never populated (the call threw), so decrementing it
        // would be a use-after-free of uninitialized memory.
        let succ_idx = normal.index();
        if succ_idx >= func.blocks.len() {
            continue;
        }

        let has_dec = func.blocks[succ_idx]
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::RcDec { var, .. } if *var == *dst));
        if has_dec {
            continue;
        }

        // Skip if the variable is used in the successor (Phase B/C
        // will have emitted appropriate RcDec already).
        let used_in_succ = func.blocks[succ_idx]
            .body
            .iter()
            .any(|instr| instr.used_vars().contains(dst))
            || func.blocks[succ_idx].terminator.used_vars().contains(dst);
        if used_in_succ {
            continue;
        }

        // Skip if the variable is live at the exit of the successor block
        // (it's still needed in downstream blocks).
        let succ_blk = block_id(succ_idx);
        if is_live_at_exit(state_map, succ_blk, *dst) {
            continue;
        }

        pending_decs.push((succ_idx, *dst, strategy));
    }

    // Prepend RcDec at the start of each successor block.
    for (succ_idx, var, strategy) in pending_decs {
        func.blocks[succ_idx]
            .body
            .insert(0, ArcInstr::RcDec { var, strategy });
    }
}
