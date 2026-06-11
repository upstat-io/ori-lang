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
    /// LOOP-EXIT MODE: a loop-INVARIANT root (definer outside the cycle) whose
    /// EVERY member read sits inside ONE CFG cycle — the lineage dies once, on
    /// the cycle's unique non-unwind exit block, NOT per iteration. The sole
    /// release lands at that block's entry (`dec_var` = the root; its definer
    /// dominates the exit). Without this mode the base walk's per-iteration
    /// alias release frees the buffer on iteration 1 — a use-after-free on
    /// every later iteration. Dying unwind edges stay owned by the Category-2
    /// `deadAtSucc` conjunct, as in dead-param mode. Spec: Annex E §AIMS RL-2
    /// + RL-4.
    LoopExit {
        site_block: usize,
        site_pos: ForwarderReleasePos,
        dec_var: ArcVarId,
    },
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
    allow_no_sink: bool,
    root: ArcVarId,
    allow_loop_exit: bool,
) -> Option<DeathPoint> {
    if let Some((site_block, dec_var)) = choose_dead_param_release_site(func, members, used) {
        return Some(DeathPoint::DeadParam {
            site_block,
            site_pos: ForwarderReleasePos::BlockEntry,
            dec_var,
        });
    }
    if let Some(death) = choose_no_sink_death(func, members, allow_no_sink) {
        return Some(death);
    }
    if !allow_loop_exit {
        return None;
    }
    let (site_block, dec_var) = choose_loop_exit_release_site(func, members, root)?;
    Some(DeathPoint::LoopExit {
        site_block,
        site_pos: ForwarderReleasePos::BlockEntry,
        dec_var,
    })
}

/// NO-SINK arm of [`choose_death_point`]: the carrier claim + the live
/// Project-extract decline gate. `None` when `allow_no_sink` is off or the
/// shape declines.
fn choose_no_sink_death(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    allow_no_sink: bool,
) -> Option<DeathPoint> {
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
    // Grow the extract's transitive aliases: `Let { Var(src) }` re-binds,
    // `Project`-of-extract chains (an extract of an extract still views the
    // same allocation tree), and Jump-arg → block-param hops. A Jump arg that
    // is an extract member flows into the target block-param (the live-across
    // receiver of the extracted buffer).
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
                if let ArcInstr::Project { dst, value, .. } = instr {
                    if extract.contains(value) && !members.contains(dst) && extract.insert(*dst) {
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

/// LOOP-EXIT MODE: a loop-INVARIANT lineage whose every member read sits inside
/// ONE CFG cycle dies once, at the cycle's unique non-unwind exit block — the
/// per-iteration alias "last use" inside the body is NOT the lineage's death
/// (the root is re-read on the next iteration). Returns `(exit_block, root)`;
/// the release lands at the exit block's entry. Spec: Annex E §AIMS RL-2 + RL-4.
///
/// `None` (decline; the conservative status quo) when ANY fails:
///  (l1) no member read, or some member-read block is NOT in a cycle (the
///       lineage is read outside the loop — a post-loop read after the exit
///       release would UAF);
///  (l2) the member-read blocks do not share ONE cycle (pairwise co-reachable);
///  (l3) the root's DEFINER sits inside the cycle — a per-iteration fresh root
///       needs a per-iteration release; one exit release would leak every
///       earlier iteration's instance;
///  (l4) the cycle has zero or multiple non-unwind exit blocks (a fork the
///       single release cannot cover; dying unwind edges are excluded — owned
///       by the Category-2 `deadAtSucc` per-edge release, disjoint);
///  (l5) the exit block itself re-reaches a cycle containing it (a re-reached
///       release double-frees);
///  (l6) the root's definer does not DOMINATE the exit (the release reads the
///       root var; a path reaching the exit without the definition is invalid).
pub(super) fn choose_loop_exit_release_site(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    root: ArcVarId,
) -> Option<(usize, ArcVarId)> {
    // (l1) member-read blocks, each sitting in a cycle.
    let read_blocks: Vec<usize> = func
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
    if read_blocks.is_empty() {
        return None;
    }
    let first = read_blocks[0];
    let first_reach = successor_reachable_blocks(func, first);
    if !first_reach.contains(&first) {
        return None;
    }
    // (l2) every read block is co-reachable with the first (one cycle).
    for &b in &read_blocks[1..] {
        if !first_reach.contains(&b) || !successor_reachable_blocks(func, b).contains(&first) {
            return None;
        }
    }
    // The cycle: blocks co-reachable with `first`.
    let cycle: FxHashSet<usize> = (0..func.blocks.len())
        .filter(|&b| {
            b == first
                || (first_reach.contains(&b)
                    && successor_reachable_blocks(func, b).contains(&first))
        })
        .collect();
    // (l3) loop-invariant root: definer outside the cycle.
    let def_block = defining_block_of(func, root)?;
    if cycle.contains(&def_block) {
        return None;
    }
    // (l4) the cycle's non-unwind exits.
    let mut exits: FxHashSet<usize> = FxHashSet::default();
    for &b in &cycle {
        let term = &func.blocks[b].terminator;
        let unwind_target = match term {
            ArcTerminator::Invoke { unwind, .. } | ArcTerminator::InvokeIndirect { unwind, .. } => {
                Some(unwind.index())
            }
            _ => None,
        };
        for s in crate::graph::successor_block_ids(term) {
            let s = s.index();
            if !cycle.contains(&s) && Some(s) != unwind_target {
                exits.insert(s);
            }
        }
    }
    if exits.len() != 1 {
        return None;
    }
    let site_block = exits.into_iter().next()?;
    // (l5) the exit must not itself sit in a cycle.
    if successor_reachable_blocks(func, site_block).contains(&site_block) {
        return None;
    }
    // (l6) the definer dominates the exit: with the definer's block removed,
    // the entry must no longer reach the exit.
    if reaches_avoiding(func, func.entry.index(), site_block, def_block) {
        return None;
    }
    Some((site_block, root))
}

/// The block whose body defines `var` (any defining instruction). `None` when
/// `var` is a function param / block param (declines loop-exit conservatively —
/// the loop-invariant root family is `Construct`-defined).
fn defining_block_of(func: &ArcFunction, var: ArcVarId) -> Option<usize> {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if instr.defined_var() == Some(var) {
                return Some(block_idx);
            }
        }
    }
    None
}

/// True iff `to` is reachable from `from` over successor edges without entering
/// `skip` — the non-domination witness for (l6).
fn reaches_avoiding(func: &ArcFunction, from: usize, to: usize, skip: usize) -> bool {
    if from == skip {
        return false;
    }
    if from == to {
        return true;
    }
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    visited.insert(from);
    let mut stack = vec![from];
    while let Some(b) = stack.pop() {
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        for s in crate::graph::successor_block_ids(&block.terminator) {
            let s = s.index();
            if s == skip || !visited.insert(s) {
                continue;
            }
            if s == to {
                return true;
            }
            stack.push(s);
        }
    }
    false
}
