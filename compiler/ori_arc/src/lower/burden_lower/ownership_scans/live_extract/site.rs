//! Release-site placement for the fresh-sum live-extract treatment
//! (`live_extract.rs` gate (f)): execution-ordered site selection +
//! site-soundness vetting for the single placed `BurdenDec`.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::{
    compute_execution_final_read_site, successor_reachable_blocks, ForwarderReleasePos,
};

/// Execution-ordered release-site selection for the (branching) live-extract
/// closure: collect every terminal READ of a member — excluding HOPS, where
/// the value continues at the hop's destination (`Let { Var }` re-binds into
/// the closure, member-dst `Project` views, `Jump` member-args) — then pick
/// the unique read every other read reaches. `None` when no read is
/// execution-final. Block INDEX order is not execution order here (the merge
/// block commonly precedes the arm blocks), so the straight-line
/// `lineage_release_site` walk would land mid-arm. Returns
/// `(block_idx, pos, read_var)`; the dec targets `read_var` (the member
/// holding the allocation at the site, valid on every path through it).
pub(super) fn choose_release_site(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> Option<(usize, ForwarderReleasePos, ArcVarId)> {
    let is_closure_own_edge = |instr: &ArcInstr| {
        matches!(
            instr,
            ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } if members.contains(src) && members.contains(dst)
        ) || matches!(
            instr,
            ArcInstr::Project { dst, value, .. }
                if members.contains(value) && members.contains(dst)
        )
    };
    compute_execution_final_read_site(func, members, is_closure_own_edge)
}

/// Gate (f): the placed release site is execution-final and covers every
/// normal exit.
pub(super) fn release_site_sound(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    root: ArcVarId,
    site_block: usize,
    site_pos: ForwarderReleasePos,
    preds: &[Vec<usize>],
) -> bool {
    // (f1) the site must not sit in a CFG cycle (a re-reached dec double-frees).
    if successor_reachable_blocks(func, site_block).contains(&site_block) {
        return false;
    }
    // (f2) a BlockEntry site must have a single predecessor so the var read at
    // the predecessor's terminator dominates the dec.
    if site_pos == ForwarderReleasePos::BlockEntry && preds.get(site_block).map(Vec::len) != Some(1)
    {
        return false;
    }
    // (f3) the site must be forward-reachable from every member-use block
    // (execution-final, not merely textually-final).
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let block_uses_member = block
            .body
            .iter()
            .any(|i| i.used_vars().iter().any(|v| members.contains(v)))
            || block
                .terminator
                .used_vars()
                .iter()
                .any(|v| members.contains(v));
        if block_uses_member
            && block_idx != site_block
            && !successor_reachable_blocks(func, block_idx).contains(&site_block)
        {
            return false;
        }
    }
    // (f4) every normal exit (`Return`-terminated block) reachable from the
    // root's definition passes through the site — a bypassing normal path
    // would never release the allocation. Unwind exits (`Resume`) are exempt
    // (status-quo leak on unwind, identical to today's behavior).
    let def_block = root_def_block(func, root);
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

/// The block where `root`'s value becomes live: the defining block for a body
/// `Construct` / `Apply`; the NORMAL successor for a terminator `Invoke`
/// (the result binds on the normal edge). Falls back to the entry block.
fn root_def_block(func: &ArcFunction, root: ArcVarId) -> usize {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            let defines = match instr {
                ArcInstr::Construct { dst, .. } | ArcInstr::Apply { dst, .. } => *dst == root,
                _ => false,
            };
            if defines {
                return block_idx;
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
