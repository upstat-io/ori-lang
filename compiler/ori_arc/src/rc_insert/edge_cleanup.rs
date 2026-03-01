//! Edge cleanup passes for RC insertion.
//!
//! After per-block RC insertion, variables that are live at a predecessor's
//! exit but not live at a successor's entry need `RcDec`. This module handles
//! both inter-block edge gaps and post-invoke borrowed-arg cleanup.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::graph::compute_predecessors;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership,
};
use crate::liveness::BlockLiveness;
use crate::ArcClassification;
use ori_types::{Idx, Pool};

use super::rc_strategy_direct;

// Edge cleanup
//
// After per-block RC insertion, variables that are live at a
// predecessor's exit but not live at a successor's entry need `RcDec`
// at the successor. This happens when a branch splits control flow and
// each path needs a different subset of the live variables.
//
// For example:
//   block_0: construct v0(str), v1(str); branch → b1, b2
//   block_1: return v0      ← v1 is live_out[b0] but not live_in[b1]
//   block_2: apply f(v0, v1)
//
// The gap `live_out[pred] - live_in[succ]` must be Dec'd.
//
// References:
//   - Lean 4: `addDecForDeadParams` in `src/Lean/Compiler/IR/RC.lean`
//   - Appel: "Modern Compiler Implementation" §10.2

/// Insert `RcDec` at block boundaries for variables that are live at a
/// predecessor's exit but not needed by a successor ("edge gap").
///
/// For single-predecessor blocks: inserts Decs at the block's start.
/// For multi-predecessor blocks with identical gaps: inserts at block start.
/// For multi-predecessor blocks with differing gaps: creates trampoline
/// blocks (edge splitting) to hold per-edge cleanup.
pub(super) fn insert_edge_cleanup(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    borrowed_params: &FxHashSet<ArcVarId>,
    global_borrows: &FxHashSet<ArcVarId>,
    pool: Option<&Pool>,
) {
    let num_blocks = func.blocks.len();
    let predecessors = compute_predecessors(func);

    // Collect edits before applying (to avoid index invalidation).
    // (block_idx, vars_to_dec) for blocks where Decs go at the start.
    let mut block_decs: Vec<(usize, Vec<ArcVarId>)> = Vec::new();
    // (pred_idx, succ_block_id, vars_to_dec) for edges that need splitting.
    let mut edge_splits: Vec<(usize, ArcBlockId, Vec<ArcVarId>)> = Vec::new();

    for (block_idx, preds) in predecessors.iter().enumerate().take(num_blocks) {
        if preds.is_empty() {
            continue;
        }

        let live_in_b = &liveness.live_in[block_idx];

        // Compute per-predecessor gaps, filtering out borrowed vars.
        let gaps: Vec<(usize, Vec<ArcVarId>)> = preds
            .iter()
            .map(|&pred_idx| {
                let mut gap: Vec<ArcVarId> = liveness.live_out[pred_idx]
                    .iter()
                    .copied()
                    .filter(|v| {
                        !live_in_b.contains(v)
                            && !borrowed_params.contains(v)
                            && !global_borrows.contains(v)
                            && classifier.needs_rc(func.var_type(*v))
                    })
                    .collect();
                gap.sort_by_key(|v| v.index()); // deterministic order
                (pred_idx, gap)
            })
            .collect();

        // Skip if all gaps are empty.
        if gaps.iter().all(|(_, g)| g.is_empty()) {
            continue;
        }

        if preds.len() == 1 {
            // Single predecessor: insert Decs at block start.
            let (_, ref gap) = gaps[0];
            if !gap.is_empty() {
                block_decs.push((block_idx, gap.clone()));
            }
        } else {
            // Multiple predecessors: check if all gaps are identical.
            let all_identical = gaps.windows(2).all(|w| w[0].1 == w[1].1);

            if all_identical {
                // All predecessors have the exact same gap → insert at block start.
                if !gaps[0].1.is_empty() {
                    block_decs.push((block_idx, gaps[0].1.clone()));
                }
            } else {
                // Different gaps per predecessor → edge split each non-empty gap.
                for &(pred_idx, ref gap) in &gaps {
                    if !gap.is_empty() {
                        edge_splits.push((pred_idx, func.blocks[block_idx].id, gap.clone()));
                    }
                }
            }
        }
    }

    // Apply block-start Decs (prepend to block body).
    for (block_idx, vars) in &block_decs {
        let decs: Vec<ArcInstr> = vars
            .iter()
            .map(|&v| ArcInstr::RcDec {
                var: v,
                strategy: rc_strategy_direct(func, pool, v),
            })
            .collect();
        let dec_spans: Vec<Option<ori_ir::Span>> = vec![None; decs.len()];

        let mut new_body = decs;
        new_body.append(&mut func.blocks[*block_idx].body);
        func.blocks[*block_idx].body = new_body;

        let mut new_spans = dec_spans;
        new_spans.append(&mut func.spans[*block_idx]);
        func.spans[*block_idx] = new_spans;
    }

    // Apply edge splits: create trampoline blocks with Dec instructions.
    for &(pred_idx, succ_block_id, ref vars) in &edge_splits {
        apply_edge_split(func, pred_idx, succ_block_id, vars, pool);
    }

    if !block_decs.is_empty() || !edge_splits.is_empty() {
        tracing::debug!(
            block_decs = block_decs.len(),
            edge_splits = edge_splits.len(),
            "edge cleanup applied"
        );
    }
}

/// Create a trampoline block between `pred` and `succ` with `RcDec` instructions.
///
/// If the successor has block params, the trampoline accepts and forwards them
/// so the edge split is transparent to the rest of the IR.
fn apply_edge_split(
    func: &mut ArcFunction,
    pred_idx: usize,
    succ_block_id: ArcBlockId,
    vars: &[ArcVarId],
    pool: Option<&Pool>,
) {
    let trampoline_id = func.next_block_id();
    let trampoline_body: Vec<ArcInstr> = vars
        .iter()
        .map(|&v| ArcInstr::RcDec {
            var: v,
            strategy: rc_strategy_direct(func, pool, v),
        })
        .collect();

    // Forward successor block params through fresh trampoline params.
    // Clone param types first to release the immutable borrow on func.blocks
    // before calling fresh_var (which borrows func mutably).
    let succ_param_types: Vec<Idx> = func.blocks[succ_block_id.index()]
        .params
        .iter()
        .map(|&(_, ty)| ty)
        .collect();
    let (trampoline_params, forward_args): (Vec<_>, Vec<_>) = succ_param_types
        .iter()
        .map(|&ty| {
            let fresh = func.fresh_var(ty);
            ((fresh, ty), fresh)
        })
        .unzip();

    func.push_block(ArcBlock {
        id: trampoline_id,
        params: trampoline_params,
        body: trampoline_body,
        terminator: ArcTerminator::Jump {
            target: succ_block_id,
            args: forward_args,
        },
    });

    redirect_edges(
        &mut func.blocks[pred_idx].terminator,
        succ_block_id,
        trampoline_id,
    );
}

/// Redirect all edges in a terminator from `old_target` to `new_target`.
fn redirect_edges(terminator: &mut ArcTerminator, old_target: ArcBlockId, new_target: ArcBlockId) {
    match terminator {
        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            if *then_block == old_target {
                *then_block = new_target;
            }
            if *else_block == old_target {
                *else_block = new_target;
            }
        }
        ArcTerminator::Switch { cases, default, .. } => {
            for (_, target) in cases.iter_mut() {
                if *target == old_target {
                    *target = new_target;
                }
            }
            if *default == old_target {
                *default = new_target;
            }
        }
        ArcTerminator::Jump { target, .. } => {
            if *target == old_target {
                *target = new_target;
            }
        }
        ArcTerminator::Invoke { normal, unwind, .. } => {
            if *normal == old_target {
                *normal = new_target;
            }
            if *unwind == old_target {
                *unwind = new_target;
            }
        }
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
}

/// Insert `RcDec` for last-use args passed to borrowing `Invoke` positions.
///
/// Companion to the per-arg borrowing detection in [`process_terminator_uses`].
/// Together they implement correct RC at Invoke call sites:
///
/// 1. `process_terminator_uses` identifies per-arg borrowing:
///    - ALL args to external C runtime functions (`ori_*`)
///    - Args to Ori functions where the callee param is `Borrowed`
///
///    These args are added to `live` (kept alive) but skip `RcInc`.
///
/// 2. This post-pass inserts `RcDec` for borrowing args whose **last use**
///    is the Invoke — i.e., args NOT in `live_out` of the invoking block.
///    Args that ARE in `live_out` are still needed later and will be Dec'd
///    at their actual last-use point by the normal Perceus logic.
///
/// Uses the **original** (pre-RC-insertion) liveness data, which correctly
/// reflects user-code liveness without RC op interference.
///
/// Ref: Lean 4 `src/Lean/Compiler/IR/RC.lean` — caller inserts Dec for
/// args passed to borrowed positions at call sites.
pub fn insert_external_invoke_cleanup(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    liveness: &BlockLiveness,
    pool: &Pool,
) {
    // Collect: normal_block → [last-use RC-typed borrowing args from Invoke]
    let mut cleanups: FxHashMap<ArcBlockId, Vec<ArcVarId>> = FxHashMap::default();

    for block in &func.blocks {
        if let ArcTerminator::Invoke {
            args,
            arg_ownership,
            normal,
            ..
        } = &block.terminator
        {
            let block_live_out = &liveness.live_out[block.id.index()];

            // Read per-arg ownership from the IR (populated by annotate_arg_ownership).
            let mut seen = FxHashSet::default();
            let last_use_args: Vec<ArcVarId> = args
                .iter()
                .enumerate()
                .filter(|&(i, &arg)| {
                    arg_ownership.get(i).copied() == Some(ArgOwnership::Borrowed)
                        && classifier.needs_rc(func.var_type(arg))
                        && !block_live_out.contains(&arg)
                        && seen.insert(arg)
                })
                .map(|(_, &arg)| arg)
                .collect();

            if !last_use_args.is_empty() {
                cleanups.entry(*normal).or_default().extend(last_use_args);
            }
        }
    }

    if cleanups.is_empty() {
        return;
    }

    tracing::debug!(
        function = func.name.raw(),
        count = cleanups.values().map(Vec::len).sum::<usize>(),
        "inserting invoke borrowed-arg cleanup RcDecs"
    );

    // Prepend RcDec at the START of each normal successor block.
    for (block_id, vars) in cleanups {
        let block_idx = block_id.index();
        for (i, var) in vars.into_iter().enumerate() {
            let strategy = rc_strategy_direct(func, Some(pool), var);
            func.blocks[block_idx]
                .body
                .insert(i, ArcInstr::RcDec { var, strategy });
            func.spans[block_idx].insert(i, None);
        }
    }
}
