//! Shared CFG analysis utilities for ARC optimization passes.
//!
//! Functions in this module are generic graph operations on [`ArcFunction`]
//! that multiple independent passes need. They live here rather than in a
//! specific pass module so that passes do not import from each other —
//! keeping the dependency graph flat (all passes depend on `graph`, none
//! depend on each other).

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::{smallvec, SmallVec};

use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};

/// Compute the predecessor list for each block (deduplicated).
///
/// Returns a vector indexed by block index, where each entry is the
/// list of distinct predecessor block indices.
pub(crate) fn compute_predecessors(func: &ArcFunction) -> Vec<Vec<usize>> {
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
///
/// Returns `SmallVec<[ArcBlockId; 4]>` to avoid heap allocation for the
/// common case (max 2 successors except Switch with many cases).
fn successor_block_ids(terminator: &ArcTerminator) -> SmallVec<[ArcBlockId; 4]> {
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
            for &(_, b) in cases {
                targets.push(b);
            }
            targets.push(*default);
            targets
        }
        ArcTerminator::Invoke { normal, unwind, .. } => smallvec![*normal, *unwind],
    }
}

/// Collect Invoke `dst` definitions mapped to their normal successor blocks.
///
/// An `Invoke { dst, normal, .. }` defines `dst` at the entry of `normal`.
/// This is analogous to how LLVM's `invoke` instruction defines its result
/// in the normal successor only, not the unwind successor. We collect these
/// so `compute_gen_kill` can add them to the kill set of the normal block.
pub(crate) fn collect_invoke_defs(func: &ArcFunction) -> FxHashMap<ArcBlockId, Vec<ArcVarId>> {
    let mut map = FxHashMap::default();
    for block in &func.blocks {
        if let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator {
            map.entry(*normal).or_insert_with(Vec::new).push(*dst);
        }
    }
    map
}
