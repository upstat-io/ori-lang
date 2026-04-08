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
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

use super::helpers::{is_live_at_exit, is_owned_at_entry, BlockCtx, LastUse};
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
///
/// Returns `(deferred_parents, merge_edge_decs)`.
#[expect(
    clippy::type_complexity,
    reason = "two-part return: deferred parents + merge-edge decs"
)]
pub(crate) fn emit_dead_at_entry_decs(
    ctx: &BlockCtx<'_>,
    new_body: &mut Vec<ArcInstr>,
) -> (
    Vec<(ArcVarId, RcStrategy, LastUse)>,
    Vec<(ArcVarId, RcStrategy)>,
) {
    let mut deferred_parents = Vec::new();
    let mut merge_edge_decs = Vec::new();

    // Precompute predecessor count for merge-block filtering.
    let predecessors = crate::graph::compute_predecessors(ctx.func);
    let pred_count = predecessors.get(ctx.blk.index()).map_or(0, Vec::len);
    let is_block_param = |v: ArcVarId| -> bool {
        ctx.func.blocks[ctx.blk.index()]
            .params
            .iter()
            .any(|&(p, _)| p == v)
    };

    // TPR-07-017 / TPR-07-019: track which take-project
    // lineages have already received a bypass-safe scope-exit dec in
    // this block, so multiple alias siblings (e.g., `%5` and its Let
    // alias `%19`, or a phi-merged param and one of its incoming
    // args) in `entry_states` do not all emit a redundant drop on
    // the same underlying value. A single `RcDec` on any lineage
    // member walks the tagged-pointer encoding once and drops the
    // payload — emitting one per alias would double-free.
    //
    // Dedup is per-LINEAGE, not per-membership-class. Two vars share
    // a lineage iff they are SSA-equivalent (Let alias chain or phi
    // merge at the same param). Vars in the same membership class
    // but with different lineages (e.g., a singleton-lineage source
    // var and a mixed-lineage phi param in the same class) are NOT
    // SSA-equivalent and may legitimately need separate drops at
    // distinct bypass-safe entries.
    let mut lineages_dec_emitted: FxHashSet<usize> = FxHashSet::default();

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
            // TPR-07-017: take-project alias-class
            // members get a single scope-exit drop at the entry edge
            // of the per-class bypass-safe region.
            //
            // A block is BYPASS-SAFE for a class iff it is NEITHER
            // forward- nor backward-reachable from any take-project
            // in that class. A block is a BYPASS-SAFE ENTRY iff it
            // is bypass-safe AND at least one of its CFG predecessors
            // is NOT bypass-safe — i.e., it is the first block on a
            // CFG path where the source enum becomes definitively
            // unreachable from the take-project. The dec is emitted
            // exactly here: downstream bypass-safe blocks inherit the
            // drop via SSA flow and emitting at every bypass-safe
            // block would produce N duplicate decs for sequential
            // bypass-safe regions, double-freeing the shared payload.
            //
            // The check is per-class (not function-global): a block
            // can be a bypass-safe entry for class `A` while being
            // reachable from an unrelated class `B`. Per-class
            // partitioning is what TPR-07-017 added on top of the
            // initial fix.
            //
            // The check fires BEFORE `use_info`/`is_live_at_exit`
            // because alias-chain "uses" on bypass-safe blocks are
            // necessarily SSA-only (Let alias / Jump-arg propagation)
            // — they don't dereference the value. The dec walks the
            // tagged-pointer encoding (`ori_iter_drop` on the
            // payload) without invalidating the source variable's
            // bit pattern, so subsequent alias reads stay safe.
            // Edge cleanup (`collect_branch_edge_decs` /
            // `collect_invoke_edge_decs`) is taught via
            // `take_move_facts.is_in_class` to skip in-class vars
            // entirely, so it never produces a duplicate dec.
            //
            // Per-class dedup: only the FIRST alias-class member we
            // encounter in entry_states gets a dec — emitting for
            // subsequent siblings (e.g., `%5` then its Let alias
            // `%19`) would double-free the same underlying value.
            if ctx
                .take_move_facts
                .is_bypass_safe_entry_for_var(var, ctx.blk.index())
            {
                // The predicate guarantees `lineage_of(var)` is
                // `Some`, but use `if let` to satisfy
                // `clippy::unwrap_used` and stay panic-free even if
                // the invariant ever weakens.
                if let Some(lineage_idx) = ctx.take_move_facts.lineage_of(var) {
                    if lineages_dec_emitted.insert(lineage_idx) {
                        if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                            new_body.push(ArcInstr::RcDec { var, strategy });
                        }
                    }
                }
                continue;
            }
            // In-class but NOT bypass-safe-entry: the dec is handled
            // either upstream at the bypass-safe-region entry, or by
            // the take-project's `is_ownership_transfer` at the
            // `Project` site. Skip here.
            if ctx.take_move_facts.is_in_class(var) {
                continue;
            }
            if ctx.use_info.contains_key(&var) || is_live_at_exit(ctx.state_map, ctx.blk, var) {
                continue;
            }
            // Merge-block filter: at a block with >1 predecessor, variables
            // that are NOT block params may come from project-source demand
            // propagation and exist only on some predecessor paths. Emitting
            // a block-level RcDec for them would fire on paths where they
            // don't exist (double-free or undefined-var skip). Check that
            // the variable is defined in ALL predecessor blocks (or received
            // as their block param); if not, skip — the defining predecessor's
            // forward walk handles cleanup via the deferred parent mechanism.
            if pred_count > 1 && !is_block_param(var) && !ctx.defined_in_block.contains(&var) {
                let all_preds_define_it = predecessors[ctx.blk.index()]
                    .iter()
                    .all(|&pred_idx| ctx.func.blocks[pred_idx].defines_var(var));
                if !all_preds_define_it {
                    // Route to per-predecessor edge cleanup instead of
                    // block-level RcDec. The edge cleanup will use
                    // trampolines to emit RcDec only on the edges where
                    // the variable actually exists.
                    if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                        merge_edge_decs.push((var, strategy));
                    }
                    continue;
                }
            }
            // Defer if this variable has live borrowed children (projections
            // or their aliases still used in this block). The deferred dec is
            // seeded into the forward walk, which emits it after the children die.
            if let Some(&child_last) = ctx.child_effective_last_use.get(&var) {
                if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                    deferred_parents.push((var, strategy, child_last));
                }
                continue;
            }
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcDec { var, strategy });
            }
        }
    }

    // Source 2: block parameters absent from entry_states.
    emit_dead_block_param_decs(ctx, new_body);

    (deferred_parents, merge_edge_decs)
}

/// Emit `RcDec` for block parameters that generated no backward demand
/// (never used in this block or any successor) but still carry RC-managed
/// values (e.g., mutable-scope list variables passed through loop exit
/// blocks). Variables that the take-project dataflow proves moved are
/// skipped per In-class block params are SKIPPED entirely
/// rather than routed: they are SSA aliases of the take-project source
/// (via Jump-arg → block-param propagation), and the source enum's
/// natural scope-exit drops in the predecessors already handle cleanup.
/// Routing the param's `ArcVarId` to a predecessor would emit an `RcDec`
/// using a name that has no SSA definition reachable from the
/// predecessor — the LLVM emitter resolves the param ID to the merge
/// block's phi node, producing a phi-dominance violation.
fn emit_dead_block_param_decs(ctx: &BlockCtx<'_>, new_body: &mut Vec<ArcInstr>) {
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
        // take-project must-move handling for block params.
        //
        // Skip in-class block params entirely. A block param that
        // belongs to a take-project alias class is just an SSA name
        // for whichever incoming Jump arg (also in the class) flows
        // through this merge. The source enum's actual scope-exit
        // drops are emitted at natural death sites in the
        // predecessors (e.g., the non-projecting branch's
        // `RcDec %5`), and the take-project at its `Project` site
        // is already an `is_ownership_transfer` so its source isn't
        // re-dropped. Emitting another drop here would
        // be a redundant double-free on the bypass path.
        if ctx.take_move_facts.is_in_class(param_var) {
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
/// This function scans every Invoke/InvokeIndirect terminator and checks whether its `dst`
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
        let (ArcTerminator::Invoke { dst, normal, .. }
        | ArcTerminator::InvokeIndirect { dst, normal, .. }) = &block.terminator
        else {
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
