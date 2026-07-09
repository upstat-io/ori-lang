//! Gate (d)/(e) release-site placement for
//! [`super::compute_catch_recover_release_lineage`]: detecting whether an
//! existing dec already covers the normal path, and choosing the
//! execution-final genuine-read site for the single placed release.

use rustc_hash::FxHashSet;

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind};

use super::super::{
    compute_execution_final_read_site, successor_reachable_blocks, ForwarderReleasePos,
};

/// Gate (d): true iff any existing `BurdenDec` on a closure member is
/// forward-reachable to a `Return`-terminated normal exit — i.e. the lineage
/// ALREADY has a normal-path release and must NOT receive another (double-free).
/// Only an unwind-only release (every dec reaches only `Resume` / `Unreachable`)
/// is the MISSING-release shape this scan cures.
pub(super) fn lineage_has_normal_path_release(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> bool {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let has_member_dec = block
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::BurdenDec { var } if members.contains(var)));
        if !has_member_dec {
            continue;
        }
        // The dec's block reaches a normal-exit Return (itself or via successors)?
        if block_reaches_return(func, block_idx) {
            return true;
        }
    }
    false
}

/// True iff `start` is `Return`-terminated OR a `Return`-terminated block is
/// forward-reachable from it (the dec releases on a path that ends at a normal
/// exit).
fn block_reaches_return(func: &ArcFunction, start: usize) -> bool {
    if matches!(
        func.blocks.get(start).map(|b| &b.terminator),
        Some(ArcTerminator::Return { .. })
    ) {
        return true;
    }
    successor_reachable_blocks(func, start).iter().any(|&b| {
        matches!(
            func.blocks.get(b).map(|b| &b.terminator),
            Some(ArcTerminator::Return { .. })
        )
    })
}

/// Gate (e): the execution-final GENUINE borrow-read site for the single placed
/// release. Collects every GENUINE read of a member — a borrowed `Apply`/`Invoke`
/// arg or a scalar-result borrowed call, EXCLUDING the closure's own edges
/// (`Let { Var }` hops, niche-payload `Project` views, the `Construct` wrap,
/// `Jump` threading, and the keep-alive `BurdenInc`/`BurdenDec` churn) — then
/// picks the unique read every other genuine read reaches. Returns
/// `(block_idx, pos, read_var)`; the dec targets `read_var` (the member holding
/// the allocation at the site). Validates the site is single-pred (for a
/// `BlockEntry` site), not in a CFG cycle, and that every normal-exit `Return`
/// reachable from the root passes through it.
pub(super) fn catch_recover_final_read_site(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    root: ArcVarId,
    preds: &[Vec<usize>],
) -> Option<(usize, ForwarderReleasePos, ArcVarId)> {
    // Skip the closure's own edges + keep-alive churn — NOT genuine reads.
    let is_closure_own_edge = |instr: &ArcInstr| {
        matches!(
            instr,
            ArcInstr::Let {
                value: ArcValue::Var(_),
                ..
            } | ArcInstr::Project { .. }
                | ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. }
        ) || matches!(
            instr,
            ArcInstr::Construct {
                ctor: CtorKind::EnumVariant { .. },
                dst,
                ..
            } if members.contains(dst)
        )
    };
    let (site_block, site_pos, dec_var) =
        compute_execution_final_read_site(func, members, is_closure_own_edge)?;
    catch_recover_site_sound(func, members, root, site_block, site_pos, preds)
        .then_some((site_block, site_pos, dec_var))
}

/// Gate (e) site-soundness: the placed release site is execution-final, covers
/// every normal exit, is not in a CFG cycle (no re-reached double-free), and a
/// `BlockEntry` site is single-pred so the predecessor's terminator read
/// dominates the dec.
fn catch_recover_site_sound(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    root: ArcVarId,
    site_block: usize,
    site_pos: ForwarderReleasePos,
    preds: &[Vec<usize>],
) -> bool {
    // (e1) the site must not sit in a CFG cycle (a re-reached dec double-frees).
    if successor_reachable_blocks(func, site_block).contains(&site_block) {
        return false;
    }
    // (e2) a BlockEntry site must have a single predecessor so the var read at the
    // predecessor's terminator dominates the dec.
    if site_pos == ForwarderReleasePos::BlockEntry && preds.get(site_block).map(Vec::len) != Some(1)
    {
        return false;
    }
    // (e3) the site must be forward-reachable from every GENUINE-read block
    // (execution-final). Loop-carried keep-alive churn is NOT a genuine read; the
    // genuine-read set is the body `Apply`/`Invoke`-borrow uses (excluding the
    // closure's own edges), so the loop body's keep-alive ops do not constrain the
    // site.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let block_genuine_reads_member = block_has_genuine_member_read(block, members);
        if block_genuine_reads_member
            && block_idx != site_block
            && !successor_reachable_blocks(func, block_idx).contains(&site_block)
        {
            return false;
        }
    }
    // (e4) every normal exit (`Return`-terminated block) reachable from the root's
    // definition passes through the site — a bypassing normal path would never
    // release the allocation. Unwind exits (`Resume`) are exempt (status-quo leak
    // on unwind, identical to today's behavior).
    let def_block = catch_recover_root_def_block(func, root);
    let mut stack: Vec<usize> = vec![def_block];
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    while let Some(b) = stack.pop() {
        if b == site_block || !visited.insert(b) {
            continue;
        }
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        if matches!(block.terminator, ArcTerminator::Return { .. }) {
            return false;
        }
        for s in crate::graph::successor_block_ids(&block.terminator) {
            stack.push(s.index());
        }
    }
    true
}

/// True iff `block` carries a GENUINE borrow-read of a member — a body
/// `Apply`/`Invoke` borrow or a borrowed terminator-`Invoke` arg — EXCLUDING the
/// closure's own edges (`Let { Var }` hops, `Project` views, the `Construct`
/// wrap, `Jump` threading) and the keep-alive `BurdenInc`/`BurdenDec` churn.
fn block_has_genuine_member_read(block: &ArcBlock, members: &FxHashSet<ArcVarId>) -> bool {
    for instr in &block.body {
        match instr {
            ArcInstr::Let {
                value: ArcValue::Var(_),
                ..
            }
            | ArcInstr::Project { .. }
            | ArcInstr::BurdenInc { .. }
            | ArcInstr::BurdenDec { .. }
            | ArcInstr::Construct { .. } => continue,
            _ => {}
        }
        if instr.used_vars().iter().any(|v| members.contains(v)) {
            return true;
        }
    }
    if let ArcTerminator::Invoke { .. } = &block.terminator {
        let term = &block.terminator;
        for (pos, v) in term.used_vars().iter().enumerate() {
            if members.contains(v) && !term.is_owned_position(pos) {
                return true;
            }
        }
    }
    false
}

/// The block where `root`'s value becomes live: the defining block for a body
/// `Apply`; the NORMAL successor for a terminator `Invoke` (the result binds on
/// the normal edge). Falls back to the entry block.
fn catch_recover_root_def_block(func: &ArcFunction, root: ArcVarId) -> usize {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if let ArcInstr::Apply { dst, .. } = instr {
                if *dst == root {
                    return block_idx;
                }
            }
        }
        if let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator {
            if *dst == root {
                return normal.index();
            }
        }
    }
    func.entry.index()
}
