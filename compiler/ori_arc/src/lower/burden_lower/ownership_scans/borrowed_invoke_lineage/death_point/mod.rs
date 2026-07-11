//! Gate (e) death-point selection for the borrowed-`Invoke` lineage scan: the
//! DEAD-PARAM mode (a dead merge block-param sink) and the NO-SINK edge-death
//! mode (the borrowed-`Invoke` carrier's successor edges). Spec: Annex E §AIMS
//! RL-2 + RL-4 + RL-5.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcVarId};

use super::super::successor_reachable_blocks;
use super::ForwarderReleasePos;

mod carrier_succ;
mod loop_exit;
mod no_sink;
mod project_extract;

use carrier_succ::choose_carrier_normal_succ_release;
use loop_exit::choose_loop_exit_release_site;
use no_sink::choose_no_sink_death;
// test-only: `tests.rs` unit-tests `choose_no_sink_carrier` directly.
#[cfg(test)]
pub(super) use no_sink::choose_no_sink_carrier;
pub(super) use project_extract::collect_member_field_extract_seeds;

/// The lineage's death-point treatment, chosen by [`choose_death_point`].
#[derive(Debug, PartialEq, Eq)]
pub(super) enum DeathPoint {
    /// DEAD-PARAM MODE: the lineage reaches EXACTLY ONE dead merge block-param,
    /// execution-final; the sole release lands at that block's entry.
    DeadParam {
        site_block: usize,
        site_pos: ForwarderReleasePos,
        dec_var: ArcVarId,
    },
    /// NO-SINK MODE: the lineage has NO dead-param sink — the receiver dies on
    /// the borrowed-`Invoke` carrier's successor edges directly. The per-edge
    /// release is handed to the class-ledger per-edge `deadAtSucc` placement; the
    /// carrier var is claimed (`burden_emitted` set) so the ledger's paired
    /// `BurdenDec` is admitted. `claim` is the closure member that is the
    /// borrowed-`Invoke` terminator arg (the carrier whose per-edge release
    /// the ledger emits).
    NoSink { claim: ArcVarId },
    /// CARRIER-SUCC MODE (builtin-result roots only): the lineage's
    /// execution-final read is a borrowed may-unwind `Invoke` arg (the
    /// carrier); the sole release lands at the consuming `Invoke`'s
    /// NORMAL-successor entry, after the borrowed read completes
    /// (`RL2_borrowed_param_emits_caller_dec`). Dying unwind edges stay with
    /// the class-ledger per-edge `deadAtSucc` placement, as in dead-param mode. `dec_var`
    /// is the carrier var (defined at the carrier block, in scope at its
    /// single-predecessor normal successor). Spec: Annex E §AIMS RL-2 + RL-4.
    CarrierSucc {
        site_block: usize,
        site_pos: ForwarderReleasePos,
        dec_var: ArcVarId,
    },
    /// LOOP-EXIT MODE: a loop-INVARIANT root (definer outside the cycle) whose
    /// EVERY member read sits inside ONE CFG cycle — the lineage dies once, on
    /// the cycle's unique non-unwind exit block, NOT per iteration. The sole
    /// release lands at that block's entry (`dec_var` = the root; its definer
    /// dominates the exit). Without this mode the per-iteration alias release
    /// frees the buffer on iteration 1 — a use-after-free on every later
    /// iteration. Dying unwind edges stay owned by the class-ledger
    /// per-edge `deadAtSucc` placement, as in dead-param mode. Spec: Annex E §AIMS RL-2 + RL-4.
    LoopExit {
        site_block: usize,
        site_pos: ForwarderReleasePos,
        dec_var: ArcVarId,
    },
}

/// Per-root-family permissions for the gate-(e) death-point fallback chain
/// (dead-param mode is always tried first and needs no permission).
#[derive(Clone, Copy)]
pub(super) struct DeathPointModes {
    /// NO-SINK mode permitted (fresh-collection roots + provably-fresh
    /// contract-result roots).
    pub(super) no_sink: bool,
    /// LOOP-EXIT mode permitted (collection roots only).
    pub(super) loop_exit: bool,
    /// CARRIER-SUCC mode permitted (self-allocating-builtin result roots only).
    pub(super) carrier_succ: bool,
}

/// Gate (e): choose the lineage's death-point treatment. Tries DEAD-PARAM mode
/// first ([`choose_dead_param_release_site`]); falls back to NO-SINK mode
/// ([`choose_no_sink_carrier`]) when `allow_no_sink` AND the lineage has no
/// dead-param sink but dies on a borrowed-`Invoke` carrier's successor edges;
/// falls back to LOOP-EXIT mode ([`choose_loop_exit_release_site`]) when
/// `allow_loop_exit` AND the lineage is a loop-invariant root borrow-read only
/// inside one CFG cycle. `None` on an unsafe / un-modeled shape (fork, UAF
/// site, per-iteration root, or no carrier death point).
pub(super) fn choose_death_point(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    used: &FxHashSet<ArcVarId>,
    root: ArcVarId,
    modes: DeathPointModes,
    interner: &ori_ir::StringInterner,
) -> Option<DeathPoint> {
    if let Some((site_block, dec_var)) = choose_dead_param_release_site(func, members, used) {
        return Some(DeathPoint::DeadParam {
            site_block,
            site_pos: ForwarderReleasePos::BlockEntry,
            dec_var,
        });
    }
    if let Some(death) = choose_no_sink_death(func, members, modes.no_sink, interner) {
        return Some(death);
    }
    if modes.carrier_succ {
        if let Some((site_block, dec_var)) =
            choose_carrier_normal_succ_release(func, members, root, interner)
        {
            return Some(DeathPoint::CarrierSucc {
                site_block,
                site_pos: ForwarderReleasePos::BlockEntry,
                dec_var,
            });
        }
    }
    if !modes.loop_exit {
        return None;
    }
    let (site_block, dec_var) = choose_loop_exit_release_site(func, members, root)?;
    Some(DeathPoint::LoopExit {
        site_block,
        site_pos: ForwarderReleasePos::BlockEntry,
        dec_var,
    })
}

/// Gate (e) dead-param sub-case: the lineage's death point — a DEAD block-param
/// member (a member bound by a block-param AND unused: `Cardinality = Absent`,
/// its only appearance is its own binding slot) reached by the receiver's
/// Jump-arg hand-offs, that is EXECUTION-FINAL: forward-reachable from every
/// block that borrows a member, so every borrow-read completes before the
/// release.
///
/// # Returns / declines
///
/// Returns `(dead_param_block, dead_param_var)`; the release lands at
/// that block's entry (`ForwarderReleasePos::BlockEntry`). `None` when the
/// lineage has no dead block-param, or has more than one (a fork the single
/// release cannot cover), or the dead-param block is not forward-reachable
/// from some member-read block (a member borrowed AFTER the dec would UAF).
/// A CFG cycle re-reaching the dead-param block declines (a re-reached dec
/// double-frees).
pub(super) fn choose_dead_param_release_site(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    used: &FxHashSet<ArcVarId>,
) -> Option<(usize, ArcVarId)> {
    // Dead block-param members: a member that is a block-param of some block AND
    // is never used (the carried-to-merge sink). Collect (block_idx, param_var).
    let mut dead_params: Vec<(usize, ArcVarId)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for &(param, _) in &block.params {
            if members.contains(&param) && !used.contains(&param) {
                dead_params.push((block_idx, param));
            }
        }
    }
    // Exactly one dead block-param sink — a fork (two dead params on disjoint
    // paths) cannot be covered by a single release; decline.
    if dead_params.len() != 1 {
        return None;
    }
    let (site_block, dec_var) = dead_params[0];
    // (e1) the dead-param block must not sit in a CFG cycle (a re-reached dec
    // double-frees).
    if successor_reachable_blocks(func, site_block).contains(&site_block) {
        return None;
    }
    // (e2) every block that BORROW-READS a member must forward-reach the
    // dead-param block — every read completes before the release (no UAF).
    for (block_idx, block) in func.blocks.iter().enumerate() {
        if block_idx == site_block {
            continue;
        }
        let reads_member = block
            .body
            .iter()
            .any(|i| i.used_vars().iter().any(|v| members.contains(v)))
            || block
                .terminator
                .used_vars()
                .iter()
                .any(|v| members.contains(v));
        if reads_member && !successor_reachable_blocks(func, block_idx).contains(&site_block) {
            return None;
        }
    }
    Some((site_block, dec_var))
}
