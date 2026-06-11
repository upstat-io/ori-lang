//! Gate (e) death-point selection for the borrowed-`Invoke` lineage scan: the
//! DEAD-PARAM mode (a dead merge block-param sink) and the NO-SINK edge-death
//! mode (the borrowed-`Invoke` carrier's successor edges). Split
//! from `borrowed_invoke_lineage.rs` for the 500-line cap. Spec: Annex E §AIMS
//! RL-2 + RL-4 + RL-5.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::successor_reachable_blocks;
use super::ForwarderReleasePos;

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
    /// release is handed to the landed Category-2 `deadAtSucc` machinery; the
    /// carrier var is claimed (`burden_emitted` set) so Cat-2's paired
    /// `BurdenDec` is admitted. `claim` is the closure member that is the
    /// borrowed-`Invoke` terminator arg (the carrier whose per-edge release
    /// Cat-2 emits).
    NoSink { claim: ArcVarId },
}

/// Gate (e): choose the lineage's death-point treatment. Tries DEAD-PARAM mode
/// first ([`choose_dead_param_release_site`]); falls back to NO-SINK mode
/// ([`choose_no_sink_carrier`]) when `allow_no_sink` AND the lineage has no
/// dead-param sink but dies on a borrowed-`Invoke` carrier's successor edges.
/// `None` on an unsafe / un-modeled shape (CFG cycle, fork, UAF site, or no
/// carrier death point).
pub(super) fn choose_death_point(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    used: &FxHashSet<ArcVarId>,
    allow_no_sink: bool,
) -> Option<DeathPoint> {
    if let Some((site_block, dec_var)) = choose_dead_param_release_site(func, members, used) {
        return Some(DeathPoint::DeadParam {
            site_block,
            site_pos: ForwarderReleasePos::BlockEntry,
            dec_var,
        });
    }
    if !allow_no_sink {
        return None;
    }
    let claim = choose_no_sink_carrier(func, members)?;
    // Live Project-extract decline gate: `same_alloc_closure_vetted` does NOT
    // grow the closure to include `Project` results (a buffer element / niche
    // payload extracted from a member is a DISTINCT allocation the result-lineage
    // owns). Such a same-alloc view extracted from a member can be LIVE across the
    // carrier's successor edge where the Cat-2 release fires — a no-sink edge
    // release would then double-free the buffer the extract still holds. DECLINE
    // no-sink (dead-param mode only); a DECLINE, never a same_alloc closure union.
    // Spec: Annex E §AIMS RL-2.
    let carrier_block = carrier_block_of(func, members, claim)?;
    if has_live_project_extract(func, members, carrier_block) {
        return None;
    }
    Some(DeathPoint::NoSink { claim })
}

/// The block index whose may-unwind `Invoke`/`InvokeIndirect` terminator reads
/// `claim` (the carrier) at a borrowed arg position. `None` when not found
/// (declines the no-sink claim conservatively).
fn carrier_block_of(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    claim: ArcVarId,
) -> Option<usize> {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let term = &block.terminator;
        if !matches!(
            term,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        ) {
            continue;
        }
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if v == claim && members.contains(&v) && !term.is_owned_position(pos) {
                return Some(block_idx);
            }
        }
    }
    None
}

/// True iff a same-allocation `Project` extracted from a lineage member is LIVE
/// across the carrier's release edges — read in a block OTHER than the carrier's
/// own block. The extract's transitive flow (`Let { Var }` aliases + `Jump`-arg →
/// block-param hops) is grown FIRST (the same discipline `same_alloc_closure_vetted`
/// applies to members), then any extract-closure member used in a non-carrier
/// block declines the no-sink claim — an edge release would double-free the
/// buffer the extract still holds.
fn has_live_project_extract(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    carrier_block: usize,
) -> bool {
    // Seed: every `Project` dst whose SOURCE is a lineage member (the extracted
    // same-alloc view). Members themselves are excluded — they are the borrowed
    // receiver, not an extract.
    let mut extract: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if members.contains(value) && !members.contains(dst) {
                    extract.insert(*dst);
                }
            }
        }
    }
    if extract.is_empty() {
        return false;
    }
    // Grow the extract's transitive aliases: `Let { Var(src) }` re-binds + Jump-arg
    // → block-param hops. A Jump arg that is an extract member flows into the
    // target block-param (the live-across receiver of the extracted buffer).
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if extract.contains(src) && !members.contains(dst) && extract.insert(*dst) {
                        grew = true;
                    }
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_idx = target.index();
                for (pos, &arg) in args.iter().enumerate() {
                    if !extract.contains(&arg) {
                        continue;
                    }
                    if let Some(&(param, _)) = func.blocks[target_idx].params.get(pos) {
                        if !members.contains(&param) && extract.insert(param) {
                            grew = true;
                        }
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    // Any extract member used in a block OTHER than the carrier's own block is
    // live across the carrier's release edges.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        if block_idx == carrier_block {
            continue;
        }
        let used_here = block
            .body
            .iter()
            .any(|i| i.used_vars().iter().any(|v| extract.contains(v)))
            || block
                .terminator
                .used_vars()
                .iter()
                .any(|v| extract.contains(v));
        if used_here {
            return true;
        }
    }
    false
}

/// NO-SINK MODE: the lineage has no dead-param sink (the receiver
/// dies on the borrowed-`Invoke` carrier's successor edges directly). Returns
/// the carrier var to CLAIM for the landed Category-2 `deadAtSucc` per-edge
/// release.
///
/// The carrier is the closure member used at a BORROWED arg position of a
/// MAY-UNWIND `Invoke` / `InvokeIndirect` terminator that is the lineage's
/// EXECUTION-FINAL borrowed-`Invoke` read — every other member-read block
/// forward-reaches the carrier block, so the receiver's last read is the
/// carrier itself (a live-across receiver's later `.len()` borrow IS such a
/// later carrier, so the cure naturally walks to the post-call last read).
///
/// `None` when:
///  - no member is a borrowed may-unwind `Invoke` arg (no carrier),
///  - more than one borrowed-`Invoke` carrier block is execution-final (a fork
///    the single per-class Cat-2 release cannot disambiguate — conservatively
///    declined to avoid an under/over-release on a phi-merged shape),
///  - a member is read AFTER the chosen carrier on some path without itself
///    being a later borrowed-`Invoke` carrier (a non-carrier live-across use
///    Cat-2's `deadAtSucc` would phantom-suppress → leak; decline conservatively).
pub(super) fn choose_no_sink_carrier(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> Option<ArcVarId> {
    // Carrier blocks: a block whose may-unwind `Invoke`/`InvokeIndirect`
    // terminator reads a member at a BORROWED arg position. Record (block_idx,
    // carrier_var).
    let mut carriers: Vec<(usize, ArcVarId)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let term = &block.terminator;
        let (ArcTerminator::Invoke {
            dst,
            normal,
            unwind,
            ..
        }
        | ArcTerminator::InvokeIndirect {
            dst,
            normal,
            unwind,
            ..
        }) = term
        else {
            continue;
        };
        // May-unwind requires a genuine edge split (the Cat-2 per-edge release
        // fires on the dying normal + unwind edges). A self-loop normal==unwind
        // is not a may-unwind carrier.
        if normal == unwind {
            continue;
        }
        // A heap-typed result may be a same-allocation VIEW of the carrier (a
        // slice): the view keeps the allocation live at the successor, the
        // per-edge probe suppresses the release on both edges, and the
        // suppressed inline pair releases nothing. Only a provably-Scalar
        // result is safe for the no-sink claim (an unpopulated repr declines
        // conservatively). Spec: Annex E §AIMS RL-2.
        if func
            .var_reprs
            .get(dst.index())
            .is_none_or(|r| *r != crate::ir::ValueRepr::Scalar)
        {
            continue;
        }
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if members.contains(&v) && !term.is_owned_position(pos) {
                carriers.push((block_idx, v));
            }
        }
    }
    if carriers.is_empty() {
        return None;
    }
    // The execution-final carrier: a carrier block forward-reachable from EVERY
    // other member-read block (so the receiver's last read is this carrier).
    // Among the carriers, exactly one must be final; pick the carrier whose
    // block every member-read block reaches; require it unique.
    let member_read_blocks: Vec<usize> = func
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| {
            block
                .body
                .iter()
                .any(|i| i.used_vars().iter().any(|v| members.contains(v)))
                || block
                    .terminator
                    .used_vars()
                    .iter()
                    .any(|v| members.contains(v))
        })
        .map(|(idx, _)| idx)
        .collect();
    let mut final_carrier: Option<(usize, ArcVarId)> = None;
    for &(carrier_block, carrier_var) in &carriers {
        let is_final = member_read_blocks.iter().all(|&rb| {
            rb == carrier_block || successor_reachable_blocks(func, rb).contains(&carrier_block)
        });
        if is_final {
            if final_carrier.is_some() {
                // Two execution-final carriers — a fork the single per-class
                // Cat-2 release cannot disambiguate. Decline conservatively.
                return None;
            }
            final_carrier = Some((carrier_block, carrier_var));
        }
    }
    let (carrier_block, carrier_var) = final_carrier?;
    // (n1) the carrier block must not sit in a CFG cycle re-reaching itself (a
    // re-reached per-edge dec double-frees).
    if successor_reachable_blocks(func, carrier_block).contains(&carrier_block) {
        return None;
    }
    Some(carrier_var)
}

/// Gate (e) dead-param sub-case: the lineage's death point — a DEAD block-param
/// member (a member bound by a block-param AND unused: `Cardinality = Absent`,
/// its only appearance is its own binding slot) reached by the receiver's
/// Jump-arg hand-offs, that is EXECUTION-FINAL: forward-reachable from every
/// block that borrows a member, so every borrow-read completes before the
/// release. Returns `(dead_param_block, dead_param_var)`; the release lands at
/// that block's entry (`ForwarderReleasePos::BlockEntry`). `None` when the
/// lineage has no dead block-param, or has more than one (a fork the single
/// release cannot cover), or the dead-param block is not forward-reachable from
/// some member-read block (a member borrowed AFTER the dec would UAF). A CFG
/// cycle re-reaching the dead-param block declines (a re-reached dec
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
