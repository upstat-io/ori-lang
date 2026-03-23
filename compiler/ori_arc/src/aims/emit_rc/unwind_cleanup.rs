//! Unwind cleanup for Invoke terminators.
//!
//! When a callee panics, iterator handles and RC-managed variables created
//! before the Invoke are not automatically cleaned up — the unwind path
//! must explicitly drop them. This pass adds `Apply @ori_iter_drop(iter)`
//! instructions to empty Resume unwind blocks so that:
//!
//! 1. Iterator handles release their reference to the source collection.
//! 2. The `downgrade_trivial_invokes` pass in `merge_blocks()` won't
//!    collapse the Invoke into an `Apply` (losing the unwind path).
//! 3. The LLVM codegen emits a proper cleanup landing pad.
//!
//! # Pipeline placement
//!
//! Runs after `emit_rc_ops()` / `realize_rc_reuse()` (RC instructions
//! present) and before `merge_blocks()` (which would downgrade the
//! Invoke). Specifically, it runs in `verify_and_merge()` right before
//! the `merge_blocks()` call.

use ori_types::Idx;
use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};

/// Add iterator drop calls to empty Resume unwind blocks.
///
/// For each Invoke whose unwind block is `Resume` with no effective body,
/// find all iterator variables live at the Invoke point and insert
/// `Apply @ori_iter_drop(iter_var)` before the Resume.
pub(crate) fn add_invoke_unwind_cleanup(func: &mut ArcFunction, interner: &ori_ir::StringInterner) {
    let iter_name = interner.intern("iter");
    let iter_drop_name = interner.intern("ori_iter_drop");

    // Phase 1: collect iterator creations, drops, and Invokes.
    let mut iter_create: Vec<(ArcVarId, usize)> = Vec::new(); // (dst, block_idx)
    let mut iter_drops: Vec<(ArcVarId, usize)> = Vec::new(); // (var, block_idx)
    let mut invokes: Vec<(usize, usize)> = Vec::new(); // (block_idx, unwind_idx)

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    dst, func: f, args, ..
                } if *f == iter_name => {
                    iter_create.push((*dst, block_idx));
                    let _ = args;
                }
                ArcInstr::Apply { func: f, args, .. } if *f == iter_drop_name => {
                    if let Some(&iter_var) = args.first() {
                        iter_drops.push((iter_var, block_idx));
                    }
                }
                _ => {}
            }
        }

        if let ArcTerminator::Invoke { unwind, normal, .. } = &block.terminator {
            let unwind_idx = unwind.index();
            if unwind_idx < func.blocks.len()
                && func.blocks[unwind_idx].terminator == ArcTerminator::Resume
                && normal != unwind
            {
                invokes.push((block_idx, unwind_idx));
            }
        }
    }

    // Phase 2: for each Invoke with Resume unwind, find live iterators.
    //
    // An iterator is "live" at an Invoke if:
    // - Created (Apply @iter) in a block that can reach the Invoke
    // - NOT dropped (Apply @ori_iter_drop) on any path FROM the creation
    //   TO the Invoke (i.e., the drop block must be reachable from entry
    //   and must itself reach the Invoke block)
    //
    // Simple heuristic: a drop "covers" an Invoke if the drop block can
    // reach the Invoke block via forward edges. If the drop is on a
    // different branch (e.g., loop exit vs loop body), it doesn't cover.
    let successors = compute_block_successors(func);
    for &(invoke_block_idx, unwind_idx) in &invokes {
        let dropped_covering: FxHashSet<ArcVarId> = iter_drops
            .iter()
            .filter(|&&(_, drop_block)| {
                // Drop covers the Invoke only if the drop block can reach
                // the Invoke block. (Simple BFS forward reachability.)
                can_reach(&successors, drop_block, invoke_block_idx)
            })
            .map(|&(var, _)| var)
            .collect();

        let live_iters: Vec<ArcVarId> = iter_create
            .iter()
            .filter(|&&(dst, create_block)| {
                create_block <= invoke_block_idx && !dropped_covering.contains(&dst)
            })
            .map(|&(dst, _)| dst)
            .collect();

        if live_iters.is_empty() {
            continue;
        }

        // Pre-allocate fresh variables and build instructions before
        // mutably borrowing the block (avoids double-borrow of func).
        let drop_instrs: Vec<ArcInstr> = live_iters
            .iter()
            .map(|&iter_var| {
                let dst = func.fresh_var(Idx::UNIT);
                ArcInstr::Apply {
                    dst,
                    ty: Idx::UNIT,
                    func: iter_drop_name,
                    args: vec![iter_var],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                }
            })
            .collect();

        let span_count = drop_instrs.len();
        func.blocks[unwind_idx].body.extend(drop_instrs);
        func.spans[unwind_idx].extend(std::iter::repeat_n(None, span_count));

        tracing::debug!(
            invoke_block = invoke_block_idx,
            unwind_block = unwind_idx,
            live_iters = live_iters.len(),
            "added iter drops to unwind block"
        );
    }
}

/// Build a successor list for each block from the function's terminators.
fn compute_block_successors(func: &ArcFunction) -> Vec<Vec<usize>> {
    func.blocks
        .iter()
        .map(|block| {
            crate::graph::successor_block_ids(&block.terminator)
                .iter()
                .map(|id| id.index())
                .collect()
        })
        .collect()
}

/// Check if `from` can reach `to` via forward edges (BFS).
fn can_reach(successors: &[Vec<usize>], from: usize, to: usize) -> bool {
    if from == to {
        return true;
    }
    let mut visited = FxHashSet::default();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(from);
    visited.insert(from);
    while let Some(current) = queue.pop_front() {
        for &succ in &successors[current] {
            if succ == to {
                return true;
            }
            if visited.insert(succ) {
                queue.push_back(succ);
            }
        }
    }
    false
}
