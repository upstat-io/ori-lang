//! LOOP-EXIT death-point mode for the borrowed-`Invoke` lineage scan: a
//! loop-INVARIANT root borrow-read only inside one CFG cycle dies once, at the
//! cycle's unique non-unwind exit block. Spec: Annex E §AIMS RL-2 + RL-4.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

use super::super::super::successor_reachable_blocks;
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
pub(in super::super) fn choose_loop_exit_release_site(
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
