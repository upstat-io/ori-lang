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
//! - [`post_convergence`] — post-convergence passes (borrow sources, sparse
//!   events, shapes, TRMC, FIP balance, FIP gates)
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

pub(crate) mod apply_aliases;
pub mod block;
pub(crate) mod effects;
pub(crate) mod fip_balance;
pub(crate) mod post_convergence;
pub(crate) mod project_aliases;
pub(crate) mod ssa_alias_classes;
pub mod state_map;

pub(crate) use fip_balance::compute_requires_unique_params;
pub use state_map::{AimsEvent, AimsStateMap, InvokeEdgeState};

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::contract::{ContextRegion, MemoryContract};
use super::lattice::AimsState;

///  — IC-7 convergence bound counter (debug-only).
///
/// Records the max iteration count observed across `analyze_function` calls
/// during the current process. Used by the burden-lattice smoke harness to
/// verify burden-emitted IR does not inflate iteration count beyond
/// `CHAIN_HEIGHT × |vars| × |blocks|` per `IA-7`.
/// Release builds skip the counter (zero overhead).
#[cfg(debug_assertions)]
static MAX_ITERATIONS_OBSERVED: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Reset the IC-7 watermark; tests call this before driving fixtures so the
/// observed maximum reflects the test scenario alone. Debug-only, test-only.
#[cfg(all(debug_assertions, test))]
pub(crate) fn reset_max_iterations_observed() {
    MAX_ITERATIONS_OBSERVED.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Read the IC-7 watermark. Debug-only, test-only.
#[cfg(all(debug_assertions, test))]
pub(crate) fn max_iterations_observed() -> usize {
    MAX_ITERATIONS_OBSERVED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Build the inverse map: Invoke-defined var → owner block whose terminator
/// is the Invoke that defines it.
///
/// SSA invariant: each var is defined at most once, so this map is a simple
/// `var → block` lookup. Used by [`analyze_function`] to route the
/// `BlockAnalysisResult.invoke_def_demand` (captured at the SUCCESSOR's strip
/// site in `compute_block_entry_state`) back to the PREDECESSOR Invoke block,
/// where `var_state_at_block_exit(invoke_block, dst)` consults it via the
/// `invoke_def_demand` side table on `AimsStateMap`.
fn build_invoke_dst_to_owner(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcBlockId> {
    func.blocks
        .iter()
        .enumerate()
        .filter_map(|(idx, block)| {
            if let ArcTerminator::Invoke { dst, .. } = &block.terminator {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "ARC IR block counts fit in u32"
                )]
                Some((*dst, ArcBlockId::new(idx as u32)))
            } else {
                None
            }
        })
        .collect()
}

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
/// - `sigs` — interprocedural contracts (empty when no interprocedural info available)
/// - `context_regions` — TRMC metadata (from normalize pass; empty when no TRMC candidates detected)
///
/// # Convergence
///
/// The lattice has finite chain height 15. Convergence is guaranteed in
/// at most `15 × |variables| × |blocks|` iterations. If exceeded, a
/// `tracing::warn!` is emitted and remaining variables are widened to TOP.
///
/// This is mathematically stronger than GHC's demand analysis, which uses an
/// empirical `n > 10` iteration cutoff in `dmdFix` with `reuseEnv` demand
/// stabilization for recursive bindings. AIMS derives its bound from the
/// product lattice's finite chain height (15 = sum of per-dimension heights),
/// giving a provable upper bound rather than an empirical safety net. GHC
/// needs `reuseEnv` and weak-demand splitting to improve convergence in lazy
/// contexts; AIMS's strict evaluation model avoids these because all demands
/// are strict by definition.
/// (See: Literature Review GHC Demand Analysis)
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the project-wide hasher; no generic needed"
)]
#[expect(
    clippy::too_many_lines,
    reason = "pre-existing; intraprocedural analysis entry point is inherently long"
)]
pub fn analyze_function(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    // TRMC context regions from Stage 3 normalize pass.
    // Empty slice when no TRMC candidates detected.
    context_regions: &[ContextRegion],
    immortals: Vec<bool>,
) -> AimsStateMap {
    let mut state_map = AimsStateMap::new(func);

    // Set immortal variables — excluded from analysis and emission.
    state_map.set_immortals(immortals);

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

    // Collect invoke definitions: Invoke { dst, normal,.. } defines `dst`
    // at the entry of the `normal` successor only (not unwind). We need to
    // remove these from the normal successor's entry state.
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    // Inverse map: Invoke-defined var → owner block whose terminator is
    // the Invoke that defines it. SSA invariant: each var is defined at
    // most once, so this map is a simple var → block lookup. Used to
    // route the BlockAnalysisResult's pre-strip demand (captured at the
    // SUCCESSOR's strip site) back to the PREDECESSOR Invoke block, where
    // `var_state_at_block_exit(invoke_block, dst)` consults it.
    let invoke_dst_to_owner = build_invoke_dst_to_owner(func);

    // BUG-04-090: pre-walk Apply-result allocation-identity side-table.
    // Ordering load-bearing per PL-5 — populate BEFORE
    // `compute_project_alias_sources` so its Step 1b seed sees the converged
    // map. See `apply_aliases.rs` and `project_aliases.rs` for full design.
    let apply_result_aliases = apply_aliases::populate_apply_result_aliases(func, sigs);
    state_map.set_apply_result_aliases(apply_result_aliases);

    // pre-walk SSA-alias equivalence-class table. Ordering
    // load-bearing per PL-5 — populate AFTER apply_result_aliases (this pass
    // unions the apply-alias edges into the classes) and BEFORE the worklist
    // (read-only thereafter).
    let ssa_alias_output =
        ssa_alias_classes::compute_ssa_alias_classes(func, state_map.apply_result_aliases());
    state_map.set_ssa_alias_output(
        ssa_alias_output.class_table,
        ssa_alias_output.class_members,
        ssa_alias_output.class_apply_alias_source_candidates,
    );

    // Precompute the unified same-allocation alias table: Project destination +
    // transitive Let alias (R2 generalized to whole-var non-projected identity)
    // + Jump-arg + CFG-merge + Select (R5) → source map. Static structure —
    // computed once, reused across worklist iterations. The R5 Select-origin dst
    // set gates the backward-demand consumer per §1.9.
    let project_alias_table =
        project_aliases::compute_project_alias_table(func, state_map.apply_result_aliases());
    let project_alias_sources = project_alias_table.sources;
    let demand_sources = project_alias_table.demand_sources;
    let select_alias_dsts = project_alias_table.select_alias_dsts;

    // Persist the UNIFIED alias closure (R1 + R2-gen + R3 + R4 + R5 + R6) on
    // AimsStateMap for the post-convergence consumer (the project-alias chain
    // walk in `realize/cleanup_redundant.rs`). The backward-demand
    // worklist consumes `demand_sources` (the ORIGINAL §1.9 closure — no R2-gen,
    // no R5) instead; widening DEMAND with whole-var identity would over-extend
    // lifetimes (the merge-edge scoped-cleanup leak). The local maps remain for
    // `propagate_project_source_demand` during the worklist loop; the clone is
    // the persistent copy consumed AFTER convergence (PL-5: read-only after
    // install). AIMS Invariant #5(c) — extends the unified model via typed
    // pre-pass input on AimsStateMap.
    state_map.set_project_alias_sources(project_alias_sources);

    let iteration_limit = AimsState::iteration_limit(func.var_types.len(), func.blocks.len());
    let mut iteration = 0;

    loop {
        state_map.reset_changed();
        // Clear pre-strip Invoke-def demand at iteration start so the
        // capture reflects the CURRENT iteration's converged successor
        // entry states (which propagate across iterations to fixpoint).
        // Stale entries from prior iterations would mix demand from
        // different convergence states and break monotonicity.
        state_map.clear_invoke_def_demand();
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

            // Record per-edge demand for Invoke and InvokeIndirect
            // terminators: normal and unwind successors carry different
            // variable sets, and the per-edge cleanup machinery consults
            // this to determine RcDec emission on the unwind path.
            // Earlier this only matched Invoke, leaving
            // InvokeIndirect terminators without recorded edge state —
            // a borrowed closure receiver dying on the unwind edge had
            // no entry-state to consult. Ref: site 4.
            let block = &func.blocks[block_idx];
            match &block.terminator {
                ArcTerminator::Invoke { normal, unwind, .. }
                | ArcTerminator::InvokeIndirect { normal, unwind, .. } => {
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
                _ => {}
            }

            // Compute the block's entry state by walking instructions backward.
            // Also accumulates block-level effects.
            let result = block::compute_block_entry_state(
                func,
                block_id,
                &state_map,
                sigs,
                &invoke_defs,
                &demand_sources,
                &select_alias_dsts,
            );
            state_map.accumulate_effect(result.effects);
            state_map.update_block_entry(block_id, result.entry_state);

            // Propagate pre-strip Invoke-def demand to the predecessor Invoke
            // block's exit-state side table. The demand was captured BEFORE
            // the strip removed `dst` from this block's entry state; routing
            // it to the OWNER (predecessor Invoke block) makes
            // `var_state_at_block_exit(invoke_block, dst)` return the
            // post-def demand instead of BOTTOM.
            for (var, state) in result.invoke_def_demand {
                if let Some(&owner_block) = invoke_dst_to_owner.get(&var) {
                    state_map.set_invoke_def_demand(owner_block, var, state);
                }
            }

            // Propagate pre-strip intra-block instruction-def demand keyed by
            // THIS block (the defining block). Captured before the strip in
            // `compute_block_entry_state`, it carries the converged
            // `seqAdd`-accumulated cardinality so `var_state_at_definition`
            // gives DP-3 / DP-2 their proven input for a var consumed entirely
            // within its defining block.
            for (var, state) in result.def_demand {
                state_map.set_def_demand(block_id, var, state);
            }
        }

        if state_map.is_converged() {
            #[cfg(debug_assertions)]
            {
                //  — record watermark for IC-7 bound assertion.
                let prev = MAX_ITERATIONS_OBSERVED.load(std::sync::atomic::Ordering::Relaxed);
                if iteration > prev {
                    MAX_ITERATIONS_OBSERVED.store(iteration, std::sync::atomic::Ordering::Relaxed);
                }
            }
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
            #[cfg(debug_assertions)]
            {
                let prev = MAX_ITERATIONS_OBSERVED.load(std::sync::atomic::Ordering::Relaxed);
                if iteration > prev {
                    MAX_ITERATIONS_OBSERVED.store(iteration, std::sync::atomic::Ordering::Relaxed);
                }
            }
            break;
        }
    }

    // Post-convergence: verify canonical fixed point and detect
    // cross-dimension chaining (Convergence Feedback).
    verify_canonical_fixed_point(&mut state_map, func);

    tracing::debug!(
        func = ?func.name,
        iterations = iteration,
        blocks = func.blocks.len(),
        vars = func.var_types.len(),
        cross_dimension = state_map.cross_dimension_detected(),
        "AIMS intraprocedural analysis converged"
    );

    // Post-convergence passes (see post_convergence.rs).
    // Position-load-bearing ordering — `populate_call_result_states` MUST
    // run BEFORE `populate_sparse_events` so the side tables are populated
    // when sparse_events queries `effective_locality_at_block_exit` for
    // `LocalAllocCandidate` emission.
    //
    // Materialize singleton `class_members` entries for transitive-drop
    // payload edges so the `class_members(class_id)` consumers in the realize
    // walks (`cleanup_redundant.rs`, `emit_rc/edge_cleanup/`) succeed for
    // singleton parents/children.
    post_convergence::materialize_transitive_drop_singleton_classes(func, sigs, &mut state_map);
    post_convergence::populate_borrow_sources(&mut state_map, func);
    post_convergence::populate_call_result_states(&mut state_map, func, sigs);
    post_convergence::populate_sparse_events(&mut state_map, func);
    post_convergence::populate_var_shapes(&mut state_map, func);
    // TF-2 carrier: propagate call-result forward state (uniqueness/locality/
    // shape) across `Let { Var }` aliases. MUST run after both
    // `populate_call_result_states` and `populate_var_shapes` so every source
    // side table is fully populated before alias inheritance.
    post_convergence::propagate_alias_forward_state(&mut state_map, func);

    // effect_summary.may_share is available post-convergence.
    // Passed to TRMC gates for logging (not enforced in v1).
    let may_share = state_map.effect_summary().may_share;
    post_convergence::detect_trmc_candidates(&mut state_map, func, may_share);
    post_convergence::populate_context_events(&mut state_map, func, context_regions, may_share);
    fip_balance::populate_fip_balance(&mut state_map, func);
    fip_balance::populate_fip_gate_events(&mut state_map, func, sigs);

    state_map
}

/// Verify that all converged states are at a canonical fixed point.
///
/// Runs [`AimsState::canonicalize_with_feedback`] on every block entry/exit
/// state. Converged states should already be canonical (`rounds == 0`).
/// If any state is NOT canonical (`rounds > 0`), this indicates a bug in
/// the analysis — some path didn't call `canonicalize` after a state
/// update. If cross-dimension chaining is detected (`rounds > 1`), the
/// `cross_dimension_detected` flag is set.
///
/// With current rules, this should always pass.
///
/// Convergence Feedback.
fn verify_canonical_fixed_point(state_map: &mut AimsStateMap, func: &ArcFunction) {
    let mut max_rounds: u8 = 0;

    for (block_idx, _) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);

        // Check entry states.
        if let Some(entry) = state_map.block_entry_states(block_id) {
            for (var, state) in entry {
                let mut copy = *state;
                let feedback = copy.canonicalize_with_feedback();
                if feedback.rounds > max_rounds {
                    max_rounds = feedback.rounds;
                }
                debug_assert_eq!(
                    copy, *state,
                    "converged state is not canonical: block={block_idx}, var={var:?}"
                );
            }
        }

        // Check exit states.
        if let Some(exit) = state_map.block_exit_states(block_id) {
            for (var, state) in exit {
                let mut copy = *state;
                let feedback = copy.canonicalize_with_feedback();
                if feedback.rounds > max_rounds {
                    max_rounds = feedback.rounds;
                }
                debug_assert_eq!(
                    copy, *state,
                    "converged state is not canonical: block={block_idx}, var={var:?}"
                );
            }
        }
    }

    if max_rounds > 0 {
        tracing::warn!(
            func = ?func.name,
            max_rounds,
            "converged state was not canonical — analysis bug"
        );
    }

    if max_rounds > 1 {
        state_map.set_cross_dimension_detected();
        tracing::warn!(
            func = ?func.name,
            max_rounds,
            "cross-dimension canonicalize chaining detected in converged states"
        );
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
