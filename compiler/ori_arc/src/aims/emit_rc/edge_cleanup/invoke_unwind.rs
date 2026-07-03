//! Phase-6.98 RL-4 release on dying Invoke UNWIND edges — the post-pipeline pass
//! that places a whole-var `BurdenDec` on a live unwind successor for vars whose
//! predecessor carries a self-canceling `BurdenInc`/`BurdenDec` pair (net 0).
//! Consumes the [`compute_invoke_edge_dead_set`] SSOT (sibling `invoke` module);
//! distinct from the per-edge dead-set computation it runs after. Spec: Annex E
//! §AIMS RL-4.

use std::sync::LazyLock;

use ori_types::Pool;
use rustc_hash::FxHashSet;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::invoke::compute_invoke_edge_dead_set;
use super::{compute_same_alloc_reps, EdgeCleanupEnv};

/// `ORI_DISABLE_INVOKE_UNWIND_PAIR_RELEASE=1` bypasses the Phase-6.98
/// invoke-unwind pair-net release pass
/// ([`emit_invoke_unwind_pair_net_releases`]). Read once at first access.
static INVOKE_UNWIND_PAIR_RELEASE_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_INVOKE_UNWIND_PAIR_RELEASE").as_deref() == Ok("1"));

/// Phase 6.98 — RL-4 release on DYING Invoke UNWIND edges for vars whose
/// predecessor carries a self-canceling whole-var `BurdenInc`/`BurdenDec`
/// pair (net 0 — the Phase-5 walk's bookkeeping for a value whose last use
/// is the predecessor's own terminator).
///
/// # Why
///
/// The pair does not release the lineage, and the presence-based
/// suppression in `release_burden_only_edge` (Phase 6.5) skips such edges,
/// so a panic landing in a LIVE unwind successor — `catch(expr:
/// opt.expect(msg:))` with the armed intercepted-unwind route — leaks the
/// value on the caught path (normal-path repair passes place releases on
/// normal successors only).
///
/// # Ordering and exclusions
///
/// Runs AFTER every normal-path repair pass (6.6 .. 6.97) so their verdicts
/// are computed against the unchanged baseline; touches ONLY unwind-successor
/// block fronts. Consumes the same [`compute_invoke_edge_dead_set`] SSOT as
/// Phase 6.5. Excludes: owned-transfer args of the predecessor terminator
/// (balanced at the transfer point per the Phase-6.5 gate); a predecessor
/// whose in-body ops NET a release (the walk's dead-out release covers both
/// paths); an unwind successor already carrying a whole-var `BurdenDec` for
/// the var (another pass placed it). Emitted `BurdenDec`s lower to `RcDec`
/// in Phase 7. Spec: Annex E §AIMS RL-4.
pub(crate) fn emit_invoke_unwind_pair_net_releases(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    take_move_facts: &super::super::take_project::TakeMoveFacts,
) {
    if *INVOKE_UNWIND_PAIR_RELEASE_DISABLED {
        return;
    }
    let same_alloc_reps = compute_same_alloc_reps(func, state_map.apply_result_aliases());
    let mut releases: Vec<(usize, ArcVarId)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let (ArcTerminator::Invoke { normal, unwind, .. }
        | ArcTerminator::InvokeIndirect { normal, unwind, .. }) = &block.terminator
        else {
            continue;
        };
        if normal == unwind {
            continue;
        }
        let unwind_idx = unwind.index();
        let env = EdgeCleanupEnv {
            func,
            state_map,
            pool,
            all_borrowed_defs,
            take_move_facts,
            same_alloc_reps: &same_alloc_reps,
        };
        for (_, succ, var) in compute_invoke_edge_dead_set(env, block_idx) {
            if succ != unwind_idx {
                continue;
            }
            if !super::super::carries_burden(func, var) {
                continue;
            }
            if super::super::is_owned_transfer_arg_at_terminator(func, block_idx, var) {
                continue;
            }
            if !pred_pair_nets_zero(func, block_idx, var) {
                continue;
            }
            if super::super::has_whole_var_burden_dec_in_block(func, succ, var) {
                continue;
            }
            if releases.iter().any(|&(s, v)| s == succ && v == var) {
                continue;
            }
            releases.push((succ, var));
        }
    }
    for &(succ, var) in &releases {
        if let Some(block) = func.blocks.get_mut(succ) {
            block.body.insert(0, ArcInstr::BurdenDec { var });
        }
        tracing::debug!(
            target: "ori_arc::aims::realize::edge_cleanup",
            succ,
            var = var.index(),
            "phase 6.98 invoke-unwind pair-net release",
        );
    }
}

/// True iff `pred_block` carries a self-canceling whole-var burden pair for
/// `var`: equal NONZERO `BurdenInc` and `BurdenDec` counts (net 0). A
/// dec-only block (genuine dead-out release) and an op-free block both
/// return false.
fn pred_pair_nets_zero(func: &ArcFunction, pred_block: usize, var: ArcVarId) -> bool {
    let Some(block) = func.blocks.get(pred_block) else {
        return false;
    };
    let mut incs: usize = 0;
    let mut decs: usize = 0;
    for instr in &block.body {
        match instr {
            ArcInstr::BurdenInc { var: v } if *v == var => incs += 1,
            ArcInstr::BurdenDec { var: v } if *v == var => decs += 1,
            _ => {}
        }
    }
    incs == decs && incs > 0
}
