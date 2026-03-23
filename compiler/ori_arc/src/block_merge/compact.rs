//! Phase 1: Remove blocks unreachable from the entry block.
//!
//! Computes reachability via DFS, builds an old→new block ID remap for
//! surviving blocks, filters out dead blocks, and rewrites all block
//! references in surviving terminators. Also remaps `cow_annotations`
//! block indices and drops annotations for dead blocks.

use crate::graph::successor_block_ids;
use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator};

use super::usize_to_block_id;

/// Remove blocks unreachable from the entry block.
#[tracing::instrument(skip_all, name = "block_merge_compact")]
pub(crate) fn compact_blocks(func: &mut ArcFunction) {
    let num_blocks = func.blocks.len();
    if num_blocks == 0 {
        return;
    }

    // DFS reachability from entry.
    let mut reachable = vec![false; num_blocks];
    let mut stack = vec![func.entry.index()];
    while let Some(idx) = stack.pop() {
        if idx >= num_blocks || reachable[idx] {
            continue;
        }
        reachable[idx] = true;
        for succ in successor_block_ids(&func.blocks[idx].terminator) {
            let si = succ.index();
            if si < num_blocks && !reachable[si] {
                stack.push(si);
            }
        }
    }

    // Check if all blocks are reachable — early exit.
    if reachable.iter().all(|&r| r) {
        return;
    }

    // Build remap: old index → Some(new index) for reachable, None for dead.
    let mut remap: Vec<Option<usize>> = vec![None; num_blocks];
    let mut counter = 0usize;
    for (i, &is_reachable) in reachable.iter().enumerate() {
        if is_reachable {
            remap[i] = Some(counter);
            counter += 1;
        }
    }

    // Filter to reachable blocks, assigning new sequential IDs.
    // We drain blocks/spans to avoid needing Default on ArcBlock.
    let old_blocks: Vec<_> = func.blocks.drain(..).collect();
    let old_spans: Vec<_> = func.spans.drain(..).collect();
    let mut new_blocks = Vec::with_capacity(counter);
    let mut new_spans = Vec::with_capacity(counter);
    debug_assert_eq!(
        old_blocks.len(),
        old_spans.len(),
        "compact: blocks/spans length mismatch: {} blocks vs {} spans",
        old_blocks.len(),
        old_spans.len()
    );
    for (i, (mut block, spans)) in old_blocks.into_iter().zip(old_spans).enumerate() {
        if reachable[i] {
            block.id = remap_to_block_id(remap[i]);
            new_blocks.push(block);
            new_spans.push(spans);
        }
    }

    // Rewrite targets in surviving blocks.
    for block in &mut new_blocks {
        remap_terminator_targets(&mut block.terminator, &remap);
    }

    #[cfg(debug_assertions)]
    {
        assert_eq!(
            new_blocks.len(),
            counter,
            "compact: expected {counter} blocks but got {}",
            new_blocks.len()
        );
    }

    func.blocks = new_blocks;
    func.spans = new_spans;
    func.entry = remap_to_block_id(remap[func.entry.index()]);
    func.cow_annotations.remap_block_indices(&remap);
}

/// Convert a remap entry to an `ArcBlockId`.
///
/// # Panics
///
/// Panics if the entry is `None` (unreachable block used where
/// reachable was expected) or exceeds `u32::MAX`.
fn remap_to_block_id(entry: Option<usize>) -> ArcBlockId {
    let idx = entry.unwrap_or_else(|| panic!("block remap entry is None for a required block"));
    usize_to_block_id(idx)
}

/// Rewrite all `ArcBlockId` references in a terminator using a remap table.
fn remap_terminator_targets(term: &mut ArcTerminator, remap: &[Option<usize>]) {
    fn remap_id(id: &mut ArcBlockId, remap: &[Option<usize>]) {
        let idx = id.index();
        assert!(
            idx < remap.len(),
            "compact: block {idx} references block beyond function ({} blocks). \
             This indicates a terminator pointing to a non-existent block.",
            remap.len()
        );
        *id = remap_to_block_id(remap[idx]);
    }

    match term {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
        ArcTerminator::Jump { target, .. } => remap_id(target, remap),
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            remap_id(then_block, remap);
            remap_id(else_block, remap);
        }
        ArcTerminator::Switch { cases, default, .. } => {
            for (_, target) in cases {
                remap_id(target, remap);
            }
            remap_id(default, remap);
        }
        ArcTerminator::Invoke { normal, unwind, .. } => {
            remap_id(normal, remap);
            remap_id(unwind, remap);
        }
    }
}
