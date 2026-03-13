//! RC emission from converged AIMS state map.
//!
//! Reads the [`AimsStateMap`] produced by intraprocedural analysis and emits
//! minimal `RcInc`/`RcDec` operations into the `ArcFunction`. Replaces
//! `rc_insert`, `rc_identity`, and `rc_elim` from the old pipeline.
//!
//! # Algorithm
//!
//! Forward walk per block. For each owned, non-scalar variable:
//! - **`RcInc`** before each use where a future use (or exit continuation) exists
//! - **`RcDec`** after the last use if the variable is dead at block exit
//! - **`RcDec`** at block entry for variables live at entry but unused and dead at exit
//!
//! Edge cleanup handles variables that die on specific edges (live in predecessor
//! but dead in a particular successor).
//!
//! # References
//!
//! - Perceus (Reinking et al., PLDI 2021): dup/drop = contraction/weakening
//! - Lean 4 `RC.lean`: backward liveness-driven insertion with last-use opt

pub mod arg_ownership;
mod coalesce;
pub mod cow;
mod dead_cleanup;
pub mod drop_hints;
mod edge_cleanup;
mod forward_walk;
mod helpers;
mod queries;
#[cfg(test)]
mod tests;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Pool;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};

/// Edge-specific RC decrement: variable + strategy.
pub(crate) type EdgeDec = (ArcVarId, RcStrategy);

// Re-export for cow/drop_hints that import via `super::collect_rc_incremented_vars`.
pub(crate) use queries::collect_rc_incremented_vars;

// Re-exports for `realize/` unified annotation walk (Section 10.3).
pub(crate) use cow::is_borrow_disjoint_from_siblings;
pub(crate) use drop_hints::{collect_borrowed_call_args, is_collection_var};

// Re-exports for `realize/` unified forward walk (Section 10.2).
pub(crate) use coalesce::coalesce_block_rc;
pub(crate) use dead_cleanup::{emit_dead_at_entry_decs, emit_dead_invoke_dsts};
pub(crate) use edge_cleanup::emit_edge_cleanup;
pub(crate) use forward_walk::emit_terminator_rc;
pub(crate) use helpers::{
    collect_all_borrowed_defs, collect_borrowed_defs, collect_defined_vars,
    compute_child_effective_last_use, is_consuming_primop, is_live_at_exit, is_owned_at_entry,
    is_ownership_transfer, precompute_block_uses, BlockCtx, LastUse,
};

/// Compute `RcStrategy` for a variable, returning `None` for scalars.
///
/// Visible to all sibling submodules (`edge_cleanup`, `dead_cleanup`,
/// `forward_walk`, `helpers`) via `super::rc_strategy`, and to
/// `realize/` via `pub(crate)` re-export.
#[inline]
pub(crate) fn rc_strategy(func: &ArcFunction, var: ArcVarId, pool: &Pool) -> Option<RcStrategy> {
    use crate::ir::ValueRepr;
    let repr = func.var_reprs[var.index()];
    if repr == ValueRepr::Scalar {
        return None;
    }
    Some(RcStrategy::from_var(
        repr,
        pool,
        func.var_types[var.index()],
    ))
}

// Public result types

/// Result of RC emission, including auxiliary hints.
pub struct EmitRcResult {
    /// Variables identified as candidates for local allocation (v1: hints only).
    pub local_alloc_candidates: Vec<LocalAllocCandidate>,
}

/// A variable identified as a local-allocation candidate.
pub struct LocalAllocCandidate {
    pub block: ArcBlockId,
    pub instr: usize,
    pub var: ArcVarId,
}

// Entry point

/// Emit RC operations into the function based on converged AIMS analysis.
///
/// Walks each block forward, inserting `RcInc` before each non-last use of
/// owned variables, and `RcDec` after the last use (or at block entry for
/// unused dead variables). Edge cleanup inserts `RcDec` on edges where a
/// variable is live in the predecessor but dead in the successor.
///
/// # Panics
///
/// Debug-panics if `func.var_reprs` is empty (must be populated before
/// RC emission — pipeline step 3: `compute_var_reprs`).
pub fn emit_rc_ops(func: &mut ArcFunction, state_map: &AimsStateMap, pool: &Pool) -> EmitRcResult {
    debug_assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    // Build function-level set of all Project-defined (borrowed) variables.
    // Used to distinguish owned vs borrowed for cross-block live variables
    // whose lattice access dimension is stuck at BOTTOM (Borrowed) because
    // backward demand propagation doesn't update access.
    let all_borrowed_defs = collect_all_borrowed_defs(func);

    // Phase 1: per-block RC emission (body + terminator uses).
    // Collect deferred parent decs (per block) for edge cleanup — these are
    // parent aggregates whose RcDec was skipped because a borrowed child
    // (from Project) is used in the block terminator.
    let mut block_deferred: FxHashMap<usize, Vec<(ArcVarId, RcStrategy)>> = FxHashMap::default();
    for block_idx in 0..func.blocks.len() {
        let deferred = emit_block_rc(func, block_idx, state_map, pool, &all_borrowed_defs);
        if !deferred.is_empty() {
            block_deferred.insert(block_idx, deferred);
        }
    }

    // Phase 1.5: dead Invoke result cleanup.
    //
    // Invoke dst variables are defined at the boundary between predecessor
    // and successor blocks. The backward analysis may never propagate demand
    // for them (e.g., if the result is unused), so they don't appear in any
    // block's entry_states. Without this sweep, such variables leak.
    dead_cleanup::emit_dead_invoke_dsts(func, state_map, pool, &all_borrowed_defs);

    // Phase 2: inter-block edge cleanup (with deferred parent decs).
    edge_cleanup::emit_edge_cleanup(func, state_map, pool, &all_borrowed_defs, &block_deferred);

    // Phase 3: RC coalescing peephole — merge adjacent RC ops per block.
    for block in &mut func.blocks {
        coalesce::coalesce_block_rc(&mut block.body);
    }

    // Phase 4: locality hint collection (v1: hints only, no stack alloc).
    let local_alloc_candidates = queries::collect_local_alloc_candidates(func, state_map);

    EmitRcResult {
        local_alloc_candidates,
    }
}

// Helpers

/// Convert a `usize` block index to `ArcBlockId`.
#[inline]
pub(crate) fn block_id(idx: usize) -> ArcBlockId {
    ArcBlockId::new(
        u32::try_from(idx).unwrap_or_else(|_| panic!("block index {idx} exceeds u32::MAX")),
    )
}

// Per-block emission

/// Emit RC operations for a single block.
///
/// Forward walk with three phases:
/// - A: `RcDec` for variables live at entry, unused in block, dead at exit
/// - B: Forward walk through body with `RcInc`/`RcDec` interleaving
/// - C: Terminator uses and cleanup
///
/// Returns deferred parent `RcDec` operations for variables whose borrowed
/// children are used in the block terminator. These must be placed on
/// successor edges by edge cleanup (the parent must outlive its children).
fn emit_block_rc(
    func: &mut ArcFunction,
    block_idx: usize,
    state_map: &AimsStateMap,
    pool: &Pool,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) -> Vec<(ArcVarId, RcStrategy)> {
    let blk = block_id(block_idx);
    let use_info = precompute_block_uses(&func.blocks[block_idx]);
    let defined_in_block = collect_defined_vars(&func.blocks[block_idx]);
    let borrowed_defs = collect_borrowed_defs(&func.blocks[block_idx]);
    let child_elu = compute_child_effective_last_use(&func.blocks[block_idx], &use_info);

    let old_body = std::mem::take(&mut func.blocks[block_idx].body);
    let mut new_body: Vec<ArcInstr> = Vec::with_capacity(old_body.len() * 2);

    let ctx = BlockCtx {
        func,
        blk,
        state_map,
        defined_in_block: &defined_in_block,
        borrowed_defs: &borrowed_defs,
        all_borrowed_defs,
        use_info: &use_info,
        pool,
        child_effective_last_use: &child_elu,
    };

    // Phase A: RcDec for variables live at entry, unused in block, dead at exit.
    dead_cleanup::emit_dead_at_entry_decs(&ctx, &mut new_body);

    // Phase B: forward walk through body instructions.
    let (uses_so_far, terminator_deferred) =
        forward_walk::emit_body_forward_walk(&ctx, &old_body, &mut new_body);

    // Phase C: terminator uses and cleanup.
    forward_walk::emit_terminator_rc(&ctx, block_idx, uses_so_far, &mut new_body);

    // For terminators without successors (Return/Resume/Unreachable),
    // emit deferred parent decs in the body. For terminators with
    // successors, return them for edge cleanup.
    let edge_deferred = match &func.blocks[block_idx].terminator {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {
            for &(var, strategy) in &terminator_deferred {
                new_body.push(ArcInstr::RcDec { var, strategy });
            }
            Vec::new()
        }
        _ => terminator_deferred,
    };

    func.blocks[block_idx].body = new_body;
    edge_deferred
}
