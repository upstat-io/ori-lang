//! Phase 5: Eliminate block params on single-predecessor blocks.
//!
//! After Phase 4 merges single-predecessor Jump chains, some blocks may
//! still have `params` with exactly one predecessor. Two cases:
//!
//! 1. **Jump predecessor** — Phase 4 should have merged these, but may
//!    miss them due to ordering (e.g., Phase 3/3b created new patterns
//!    that Phase 4's fixed-point didn't reach). Convert params to Let
//!    bindings via [`lower_parallel_copy`] and clear the Jump args.
//!
//! 2. **Non-Jump predecessor** (Branch/Switch/Invoke) — these terminators
//!    don't carry args to block params. The params are dead (zero incoming
//!    values). Clear them directly.
//!
//! In both cases, clearing params eliminates redundant LLVM phi nodes
//! that would otherwise have a single incoming edge.

use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcTerminator};

use super::merge::lower_parallel_copy;

/// Eliminate block params on blocks with exactly one predecessor.
///
/// Single-pass: computes predecessors once, then mutates `func`.
/// Mutations (add Let bindings, clear params, clear Jump args) do not
/// change block topology, so the predecessor snapshot stays valid.
#[tracing::instrument(skip_all, name = "phase5_single_pred_phi")]
pub(crate) fn eliminate_single_pred_params(func: &mut ArcFunction) {
    let predecessors = compute_predecessors(func);
    let entry_idx = func.entry.index();

    // We need mutable access to `func.blocks[a_idx]` and `func.blocks[b_idx]`
    // inside the loop, so we can't use an iterator over `predecessors`.
    #[expect(
        clippy::needless_range_loop,
        reason = "loop body mutates func.blocks at both a_idx and b_idx"
    )]
    for b_idx in 0..func.blocks.len() {
        if b_idx == entry_idx {
            continue;
        }
        if func.blocks[b_idx].params.is_empty() {
            continue;
        }
        if predecessors[b_idx].len() != 1 {
            continue;
        }

        let a_idx = predecessors[b_idx][0];

        // Check if the single predecessor is a Jump with args
        let is_jump_with_args = matches!(
            &func.blocks[a_idx].terminator,
            ArcTerminator::Jump { args, target, .. }
                if !args.is_empty() && target.index() == b_idx
        );

        if is_jump_with_args {
            // Jump predecessor: convert params to Let bindings.
            let b_params = func.blocks[b_idx].params.clone();
            let jump_args = match &func.blocks[a_idx].terminator {
                ArcTerminator::Jump { args, .. } => args.clone(),
                _ => unreachable!(),
            };

            debug_assert_eq!(
                b_params.len(),
                jump_args.len(),
                "Jump args/params arity mismatch: block {a_idx} → block {b_idx}",
            );
            if b_params.len() != jump_args.len() {
                continue;
            }

            // NOTE: Unlike Phase 4, Phase 5 does NOT merge B's body into A.
            // B's body stays in B, so COW annotations at (b_idx, *) remain
            // valid — no remap_block_merge needed.
            lower_parallel_copy(func, a_idx, &b_params, &jump_args);

            func.blocks[b_idx].params.clear();
            if let ArcTerminator::Jump { args, .. } = &mut func.blocks[a_idx].terminator {
                args.clear();
            }
        } else {
            // Non-Jump predecessor (Branch/Switch/Invoke): params have
            // zero incoming arg values — they are dead. Clear them.
            //
            // Branch/Switch don't carry args. Invoke defines `dst`
            // implicitly at the normal block entry, not via block params.
            tracing::debug!(
                block = b_idx,
                pred = a_idx,
                param_count = func.blocks[b_idx].params.len(),
                "clearing dead params on non-Jump-predecessor block"
            );
            func.blocks[b_idx].params.clear();
        }
    }
}
