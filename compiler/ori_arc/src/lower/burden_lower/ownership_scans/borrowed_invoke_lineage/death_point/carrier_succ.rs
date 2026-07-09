//! CARRIER-SUCC death-point arm (builtin-result roots only): the lineage's
//! sole release site lands at the execution-final borrowed-`Invoke` carrier's
//! normal-successor block entry. Spec: Annex E §AIMS RL-2 + RL-4.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

use super::super::super::successor_reachable_blocks;
use super::project_extract::has_live_project_extract;

/// CARRIER-SUCC MODE (builtin-result roots only): the lineage's sole release
/// site — the EXECUTION-FINAL borrowed may-unwind `Invoke` carrier's NORMAL
/// successor block entry.
///
/// # Returns
///
/// `(normal_succ_block, carrier_var)`; the release lands at that block's
/// entry (`ForwarderReleasePos::BlockEntry`), AFTER the borrowed read
/// completed on the predecessor's terminator.
///
/// # Declines (`None`, the conservative status-quo, never a double-free)
///
///  - no member is a borrowed arg of a may-unwind `Invoke`/`InvokeIndirect`
///    terminator (no carrier);
///  - a carrier's RESULT may be a same-allocation VIEW of the borrowed member
///    (a non-scalar result whose callee is not a known fresh-allocating
///    builtin — `@slice`/`@substring` slice the receiver's buffer; releasing
///    the buffer at the successor would free what the view still holds);
///  - more than one carrier block is execution-final (a fork the single placed
///    release cannot cover), or none is;
///  - the carrier's normal successor has any predecessor other than the
///    carrier block itself (the release would fire on a path that never
///    passed the borrowed read — the carrier var may not even be defined
///    there);
///  - the normal successor itself reads a member (its entry release would
///    precede the read), or a member-read block / the successor itself is
///    forward-reachable from the successor WITHOUT passing the root's
///    defining-`Invoke` block (a read after the release, or a re-reached
///    release with no intervening re-birth, within one allocation's
///    lifetime — the loop-carried double-free / use-after-free shapes; a
///    per-iteration root whose every re-reach passes its own re-birth is
///    SAFE: each iteration's release pairs with that iteration's fresh
///    allocation);
///  - a same-allocation `Project` extract of a member is live outside the
///    carrier block ([`has_live_project_extract`] — the release would free the
///    buffer the extract still views).
pub(super) fn choose_carrier_normal_succ_release(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    root: ArcVarId,
    interner: &ori_ir::StringInterner,
) -> Option<(usize, ArcVarId)> {
    // Carrier blocks: a may-unwind `Invoke`/`InvokeIndirect` terminator reading
    // a member at a BORROWED arg position. Record (block, carrier_var, normal).
    let mut carriers: Vec<(usize, ArcVarId, usize)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let term = &block.terminator;
        let (dst, normal, unwind) = match term {
            ArcTerminator::Invoke {
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
            } => (*dst, *normal, *unwind),
            _ => continue,
        };
        if !term.used_vars().iter().any(|v| members.contains(v)) {
            continue;
        }
        if normal == unwind {
            // A self-loop normal==unwind is not a genuine may-unwind edge split.
            return None;
        }
        // Result-aliasing gate: the carrier's result must be provably NOT a
        // same-allocation view of the borrowed member — a Scalar result reads
        // nothing of the buffer after the call; a known self-allocating-builtin
        // result (`fresh_rc_alloc_dst_terminator`) is a FRESH rc=1 buffer that
        // never aliases its operands. Anything else (sharing views, unknown
        // user callees, indirect calls with heap results) declines.
        let dst_scalar = func
            .var_reprs
            .get(dst.index())
            .is_some_and(|r| *r == crate::ir::ValueRepr::Scalar);
        let dst_fresh_builtin =
            crate::aims::realize::fresh_rc_alloc_dst_terminator(term, func, interner).is_some();
        if !dst_scalar && !dst_fresh_builtin {
            return None;
        }
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if members.contains(&v) && !term.is_owned_position(pos) {
                carriers.push((block_idx, v, normal.index()));
            }
        }
    }
    if carriers.is_empty() {
        return None;
    }
    // The execution-final carrier: every member-read block forward-reaches it
    // (the receiver's last read is the carrier itself); require it unique.
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
    let mut final_carrier: Option<(usize, ArcVarId, usize)> = None;
    for &(carrier_block, carrier_var, normal_idx) in &carriers {
        let is_final = member_read_blocks.iter().all(|&rb| {
            rb == carrier_block || successor_reachable_blocks(func, rb).contains(&carrier_block)
        });
        if is_final {
            if final_carrier.is_some() {
                return None;
            }
            final_carrier = Some((carrier_block, carrier_var, normal_idx));
        }
    }
    let (carrier_block, carrier_var, normal_idx) = final_carrier?;
    // The normal successor must be reached ONLY through the carrier (single
    // predecessor — guarantees the carrier var is defined at the release site
    // and the release fires only on the path that passed the borrowed read).
    let preds = crate::graph::compute_predecessors(func);
    if preds.get(normal_idx).map(Vec::as_slice) != Some(&[carrier_block]) {
        return None;
    }
    // The release site must not itself read a member (the entry release would
    // precede the read).
    if member_read_blocks.contains(&normal_idx) {
        return None;
    }
    // Per-allocation lifetime guard: from the release site, no member-read
    // block — and not the release site itself — may be forward-reachable
    // WITHOUT passing the root's defining-`Invoke` block. A path re-reaching
    // a read or the release without an intervening re-birth operates on the
    // ALREADY-RELEASED allocation (use-after-free / double-free); a path
    // that re-executes the defining `Invoke` first operates on a FRESH
    // allocation (the per-iteration template chain inside a loop body), where
    // the per-iteration release is exactly that iteration's RL-2 release.
    let root_def_block = func.blocks.iter().position(|block| {
        matches!(
            &block.terminator,
            ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. }
                if *dst == root
        )
    })?;
    let reach_without_rebirth = blocked_forward_reach(func, normal_idx, root_def_block);
    if reach_without_rebirth.contains(&normal_idx)
        || member_read_blocks
            .iter()
            .any(|rb| reach_without_rebirth.contains(rb))
    {
        return None;
    }
    // A member-derived same-allocation `Project` extract live outside the
    // carrier block would be freed by the placed release while still read.
    if has_live_project_extract(func, members, carrier_block) {
        return None;
    }
    Some((normal_idx, carrier_var))
}

/// Blocks forward-reachable from `start`'s successors without entering
/// `blocked` (the DFS never expands `blocked`'s successors and never inserts
/// it). `start` itself appears in the result only when a cycle re-reaches it.
fn blocked_forward_reach(func: &ArcFunction, start: usize, blocked: usize) -> FxHashSet<usize> {
    let mut reached: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = func
        .blocks
        .get(start)
        .map(|b| {
            crate::graph::successor_block_ids(&b.terminator)
                .iter()
                .map(|s| s.index())
                .collect()
        })
        .unwrap_or_default();
    while let Some(b) = stack.pop() {
        if b == blocked || !reached.insert(b) {
            continue;
        }
        if let Some(block) = func.blocks.get(b) {
            stack.extend(
                crate::graph::successor_block_ids(&block.terminator)
                    .iter()
                    .map(|s| s.index()),
            );
        }
    }
    reached
}
