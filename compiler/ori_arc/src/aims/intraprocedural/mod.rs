//! Intraprocedural backward dataflow analysis for AIMS.
//!
//! Computes [`AimsState`](super::lattice::AimsState) for every variable at
//! every block boundary within a single function. The analysis direction is
//! BACKWARD: we discover how each value WILL be used (future demand) to
//! decide what RC operations to emit.
//!
//! # Entry point
//!
//! [`analyze_function`] runs the backward dataflow to fixed-point convergence,
//! returning an [`AimsStateMap`] that downstream passes (RC emission, reuse
//! emission, COW annotation) consume.
//!
//! # Module structure
//!
//! - [`state_map`] — [`AimsStateMap`] data structure + sparse events
//! - [`block`] — per-block backward analysis (instructions, terminators,
//!   control flow merge via `alt_join`, pattern match via `Project` transfer)
//!
//! # References
//!
//! - GHC demand analysis backward pass (`compiler/GHC/Core/Opt/DmdAnal.hs`)
//! - Lean 4 RC insertion (`src/Lean/Compiler/IR/RC.lean`)
//! - `ori_arc` liveness (`compiler/ori_arc/src/liveness/mod.rs`)

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use expect for clearer failure messages"
)]
mod tests;

pub mod block;
pub mod state_map;

pub use state_map::{AimsEvent, AimsStateMap, InvokeEdgeState};

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::contract::{ContextRegion, MemoryContract};
use super::lattice::AimsState;
use super::transfer::transfer_def;

/// Run backward dataflow analysis on a single function.
///
/// Iterates blocks in postorder (successors before predecessors in the
/// backward direction) until fixed-point convergence. Returns the
/// converged [`AimsStateMap`] with per-block entry/exit states.
///
/// # Parameters
///
/// - `func` — the ARC IR function to analyze
/// - `classifier` — type classification (scalar filtering)
/// - `sigs` — interprocedural contracts (from Section 03; empty in Stage 1)
/// - `context_regions` — TRMC metadata (from Stage 3; empty in Stage 1)
///
/// # Convergence
///
/// The lattice has finite chain height 15. Convergence is guaranteed in
/// at most `15 × |variables| × |blocks|` iterations. If exceeded, a
/// `tracing::warn!` is emitted and remaining variables are widened to TOP.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the project-wide hasher; no generic needed"
)]
pub fn analyze_function(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    _context_regions: &[ContextRegion],
) -> AimsStateMap {
    let mut state_map = AimsStateMap::new(func);

    // Mark scalar variables — excluded from analysis entirely.
    for (var_idx, &ty) in func.var_types.iter().enumerate() {
        if classifier.is_scalar(ty) {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR var counts fit in u32"
            )]
            state_map.set_permanent_scalar(ArcVarId::new(var_idx as u32));
        }
    }

    // Compute postorder for backward traversal: successors appear before
    // predecessors, so demand from successors is available when we compute
    // a block's exit/entry state.
    let postorder = crate::graph::compute_postorder(func);

    // Collect invoke definitions: Invoke { dst, normal, .. } defines `dst`
    // at the entry of the `normal` successor only (not unwind). We need to
    // remove these from the normal successor's entry state.
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    let iteration_limit = AimsState::iteration_limit(func.var_types.len(), func.blocks.len());
    let mut iteration = 0;

    loop {
        state_map.reset_changed();
        iteration += 1;

        // Process blocks in postorder (successors first for backward analysis).
        for &block_idx in &postorder {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR block counts fit in u32"
            )]
            let block_id = ArcBlockId::new(block_idx as u32);

            // Compute the block's exit state from successor entry states.
            let exit_state = block::compute_block_exit_state(func, block_id, &state_map);
            state_map.update_block_exit(block_id, exit_state);

            // Record per-edge demand for Invoke terminators: normal
            // and unwind successors carry different variable sets.
            let block = &func.blocks[block_idx];
            if let ArcTerminator::Invoke { normal, unwind, .. } = &block.terminator {
                let normal_entry = state_map
                    .block_entry_states(*normal)
                    .cloned()
                    .unwrap_or_default();
                let unwind_entry = state_map
                    .block_entry_states(*unwind)
                    .cloned()
                    .unwrap_or_default();
                state_map.set_invoke_edge_state(
                    block_id,
                    InvokeEdgeState {
                        normal: normal_entry,
                        unwind: unwind_entry,
                    },
                );
            }

            // Compute the block's entry state by walking instructions backward.
            let entry_state = block::compute_block_entry_state(
                func,
                block_id,
                &state_map,
                classifier,
                sigs,
                &invoke_defs,
            );
            state_map.update_block_entry(block_id, entry_state);
        }

        if state_map.is_converged() {
            break;
        }

        // Non-convergence safety net.
        if iteration >= iteration_limit {
            tracing::warn!(
                func = ?func.name,
                iterations = iteration,
                limit = iteration_limit,
                "AIMS analysis did not converge within bound — widening to TOP. \
                 This indicates a bug in transfer functions."
            );
            widen_to_top(&mut state_map, func);
            break;
        }
    }

    tracing::debug!(
        func = ?func.name,
        iterations = iteration,
        blocks = func.blocks.len(),
        vars = func.var_types.len(),
        "AIMS intraprocedural analysis converged"
    );

    // Post-convergence: populate borrow source side table.
    // In SSA form each variable has exactly one definition, so the borrow
    // source is per-variable (not per-point). Walk all instructions and
    // record BorrowSource for Project (and any future borrow-producing
    // instructions).
    populate_borrow_sources(&mut state_map, func);

    state_map
}

/// Populate the borrow source side table after analysis converges.
///
/// Each variable defined by a `Project` instruction gets
/// `BorrowSource::Exact(source_var)`. Block params that receive values
/// from multiple predecessors are left without a source (implicitly
/// `Unknown` if queried). In ARC IR's SSA form, each variable has exactly
/// one definition point, so this is per-variable, not per-point.
fn populate_borrow_sources(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for block in &func.blocks {
        for instr in &block.body {
            // Use the converged state to compute the transfer result.
            let get_state = |v: ArcVarId| -> AimsState {
                if state_map.is_scalar(v) {
                    return AimsState::SCALAR;
                }
                state_map.var_state_at_block_entry(block.id, v)
            };

            if let Some(def) = transfer_def(instr, &get_state) {
                if let (Some(dst), Some(source)) = (instr.defined_var(), def.borrow_source) {
                    state_map.set_borrow_source(dst, source);
                }
            }
        }
    }
}

/// Widen all non-converged variables to TOP (safety net for non-convergence).
fn widen_to_top(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for (block_idx, _block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);

        let mut entry = state_map
            .block_entry_states(block_id)
            .cloned()
            .unwrap_or_default();
        for state in entry.values_mut() {
            *state = AimsState::TOP;
        }
        state_map.update_block_entry(block_id, entry);

        let mut exit = state_map
            .block_exit_states(block_id)
            .cloned()
            .unwrap_or_default();
        for state in exit.values_mut() {
            *state = AimsState::TOP;
        }
        state_map.update_block_exit(block_id, exit);
    }
}
