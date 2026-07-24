//! CFG traversal and predecessor utilities.

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::{smallvec, SmallVec};

use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};

/// Compute the predecessor list for each block (deduplicated).
pub fn compute_predecessors(func: &ArcFunction) -> Vec<Vec<usize>> {
    let num_blocks = func.blocks.len();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); num_blocks];

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen = FxHashSet::default();
        for succ_id in successor_block_ids(&block.terminator) {
            let succ_idx = succ_id.index();
            if succ_idx < num_blocks && seen.insert(succ_idx) {
                predecessors[succ_idx].push(block_idx);
            }
        }
    }

    predecessors
}

/// Extract successor block IDs from a terminator.
pub fn successor_block_ids(terminator: &ArcTerminator) -> SmallVec<[ArcBlockId; 4]> {
    match terminator {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {
            SmallVec::new()
        }
        ArcTerminator::Jump { target, .. } => smallvec![*target],
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => smallvec![*then_block, *else_block],
        ArcTerminator::Switch { cases, default, .. } => {
            let mut targets = SmallVec::with_capacity(cases.len() + 1);
            for &(_, block) in cases {
                targets.push(block);
            }
            targets.push(*default);
            targets
        }
        ArcTerminator::Invoke { normal, unwind, .. }
        | ArcTerminator::InvokeIndirect { normal, unwind, .. } => smallvec![*normal, *unwind],
    }
}

/// Collect Invoke `dst` definitions mapped to their normal successor blocks.
pub(crate) fn collect_invoke_defs(func: &ArcFunction) -> FxHashMap<ArcBlockId, Vec<ArcVarId>> {
    let mut map = FxHashMap::default();
    for block in &func.blocks {
        match &block.terminator {
            ArcTerminator::Invoke { dst, normal, .. }
            | ArcTerminator::InvokeIndirect { dst, normal, .. } => {
                map.entry(*normal).or_insert_with(Vec::new).push(*dst);
            }
            _ => {}
        }
    }
    map
}

/// Compute predecessor counts for each block (deduplicated).
pub(crate) fn compute_pred_counts(func: &ArcFunction) -> Vec<usize> {
    let num_blocks = func.blocks.len();
    let mut counts = vec![0usize; num_blocks];

    for block in &func.blocks {
        let mut seen = FxHashSet::default();
        for succ in successor_block_ids(&block.terminator) {
            let successor_index = succ.index();
            if successor_index < num_blocks && seen.insert(successor_index) {
                counts[successor_index] += 1;
            }
        }
    }

    counts
}

/// Compute a postorder traversal of reachable CFG blocks from the entry.
pub fn compute_postorder(func: &ArcFunction) -> Vec<usize> {
    let num_blocks = func.blocks.len();
    let mut visited = vec![false; num_blocks];
    let mut postorder = Vec::with_capacity(num_blocks);
    let mut stack: Vec<(usize, bool)> = vec![(func.entry.index(), false)];

    while let Some(&mut (block_idx, ref mut children_done)) = stack.last_mut() {
        if *children_done {
            postorder.push(block_idx);
            stack.pop();
            continue;
        }

        *children_done = true;
        if block_idx >= num_blocks || visited[block_idx] {
            stack.pop();
            continue;
        }
        visited[block_idx] = true;

        for succ_id in successor_block_ids(&func.blocks[block_idx].terminator) {
            let succ_idx = succ_id.index();
            if succ_idx < num_blocks && !visited[succ_idx] {
                stack.push((succ_idx, false));
            }
        }
    }

    postorder
}

/// CHK intersect: walk two immediate-dominator chains until they meet.
pub(crate) fn chk_intersect(
    mut a: usize,
    mut b: usize,
    idom: &[Option<usize>],
    rpo_pos: &[usize],
) -> usize {
    while a != b {
        while rpo_pos[a] > rpo_pos[b] {
            let Some(next) = idom[a] else {
                debug_assert!(false, "intersect: broken idom chain at {a}");
                return a;
            };
            a = next;
        }
        while rpo_pos[b] > rpo_pos[a] {
            let Some(next) = idom[b] else {
                debug_assert!(false, "intersect: broken idom chain at {b}");
                return b;
            };
            b = next;
        }
    }
    a
}
