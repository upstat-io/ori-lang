//! Core RC insertion logic — the backward-walk Perceus algorithm.
//!
//! Contains the two entry points ([`insert_rc_ops`] for tests,
//! [`insert_rc_ops_with_ownership`] for production). The shared inner
//! implementation that processes each block lives in [`block_rc`].
//!
//! [`block_rc`]: super::block_rc

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcInstr, ArcVarId};
use crate::liveness::BlockLiveness;
use crate::ownership::{DerivedOwnership, Ownership};
use crate::ArcClassification;

use super::block_rc::process_block_rc;
#[cfg(test)]
use super::compute_borrows;
use super::RcContext;

/// Insert `RcInc`/`RcDec` operations into an ARC IR function.
///
/// Modifies `func.blocks` in-place, inserting RC operations based on
/// liveness analysis. Borrowed parameters (from §06.2) and variables
/// derived from them skip RC tracking entirely.
///
/// # Arguments
///
/// * `func` — the ARC IR function to transform (mutated in place).
/// * `classifier` — type classifier for `needs_rc()` checks.
/// * `liveness` — precomputed liveness (from [`compute_liveness`](crate::compute_liveness)).
#[cfg(test)]
pub(crate) fn insert_rc_ops(
    func: &mut crate::ir::ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
) {
    // Precondition: RC insertion should run on fresh IR with no existing RC ops.
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. })),
        "insert_rc_ops: IR already contains RcInc/RcDec — pipeline ordering error"
    );

    tracing::debug!(function = func.name.raw(), "inserting RC operations");

    // Collect borrowed function parameters.
    let borrowed_params: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();

    let entry_idx = func.entry.index();
    let num_blocks = func.blocks.len();

    // Precompute Invoke dst definitions for each normal successor.
    // See liveness.rs `collect_invoke_defs` — same concept: an Invoke's
    // dst is defined at the normal successor's entry, like a block param.
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    // Collect per-block borrow sets for reuse by insert_edge_cleanup,
    // avoiding the redundant recomputation that compute_global_borrows
    // would perform.
    let mut per_block_borrows: Vec<FxHashSet<ArcVarId>> = Vec::with_capacity(num_blocks);

    for block_idx in 0..num_blocks {
        let borrows = compute_borrows(&func.blocks[block_idx], &borrowed_params);
        per_block_borrows.push(borrows);

        let (new_body, new_spans) = {
            let ctx = RcContext {
                func,
                classifier,
                pool: None, // test-only path — no Pool available
                borrowed_params: &borrowed_params,
                borrows: &per_block_borrows[block_idx],
                sigs: None,
                block_live_out: None,
            };
            process_block_rc(
                &ctx,
                block_idx,
                &liveness.live_out[block_idx],
                &invoke_defs,
                block_idx == entry_idx,
            )
        };

        func.blocks[block_idx].body = new_body;
        func.spans[block_idx] = new_spans;
    }

    // Step 5: Edge cleanup
    //
    // After per-block RC insertion, handle "stranded" variables that are
    // live at a predecessor's exit but not needed by a successor.
    // See `insert_edge_cleanup` for details.
    //
    // Build global borrow set from pre-collected per-block sets (avoids
    // redundant recomputation via compute_global_borrows).
    let global_borrows: FxHashSet<ArcVarId> = per_block_borrows
        .into_iter()
        .flat_map(FxHashSet::into_iter)
        .collect();
    super::edge_cleanup::insert_edge_cleanup(
        func,
        classifier,
        liveness,
        &borrowed_params,
        &global_borrows,
        None, // test-only path — no Pool
    );
}

/// Insert `RcInc`/`RcDec` operations using global [`DerivedOwnership`] analysis.
///
/// Enhanced version of [`insert_rc_ops`] that uses the whole-function
/// `DerivedOwnership` vector (from [`infer_derived_ownership`](crate::borrow::infer_derived_ownership))
/// instead of per-block `compute_borrows`. This captures cross-block borrow
/// propagation that the per-block approach misses.
///
/// When a variable derived from a borrowed parameter flows across a block
/// boundary (e.g., defined in B0 but used in B1), the per-block approach
/// loses track and treats it as owned in B1 — potentially omitting the
/// `RcInc` needed at owned positions. The `DerivedOwnership` vector has
/// global knowledge, ensuring correct RC ops in all blocks.
///
/// With `sigs`, also performs closure capture analysis (Step 2.4):
/// `PartialApply` captures of borrowed-derived vars at `Borrowed` callee
/// positions skip `RcInc` when the closure doesn't escape the block.
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn insert_rc_ops_with_ownership(
    func: &mut crate::ir::ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    ownership: &[DerivedOwnership],
    sigs: &FxHashMap<ori_ir::Name, crate::ownership::AnnotatedSig>,
    pool: &ori_types::Pool,
) {
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. })),
        "insert_rc_ops_with_ownership: IR already contains RcInc/RcDec"
    );

    tracing::debug!(
        function = func.name.raw(),
        "inserting RC operations (ownership-enhanced)"
    );

    let borrowed_params: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();

    // Global borrow set from DerivedOwnership — covers cross-block propagation.
    let global_borrows: FxHashSet<ArcVarId> = ownership
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, DerivedOwnership::BorrowedFrom(_)))
        .map(|(i, _)| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR var counts fit in u32"
            )]
            ArcVarId::new(i as u32)
        })
        .collect();

    let entry_idx = func.entry.index();
    let num_blocks = func.blocks.len();
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    for block_idx in 0..num_blocks {
        let (new_body, new_spans) = {
            let ctx = RcContext {
                func,
                classifier,
                pool: Some(pool),
                borrowed_params: &borrowed_params,
                borrows: &global_borrows,
                sigs: Some(sigs),
                block_live_out: Some(&liveness.live_out[block_idx]),
            };
            process_block_rc(
                &ctx,
                block_idx,
                &liveness.live_out[block_idx],
                &invoke_defs,
                block_idx == entry_idx,
            )
        };

        func.blocks[block_idx].body = new_body;
        func.spans[block_idx] = new_spans;
    }

    super::edge_cleanup::insert_edge_cleanup(
        func,
        classifier,
        liveness,
        &borrowed_params,
        &global_borrows,
        Some(pool),
    );
}
