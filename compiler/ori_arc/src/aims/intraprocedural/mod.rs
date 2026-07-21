//! Intraprocedural backward dataflow analysis for AIMS.
//!
//! [`analyze_function`] computes future ownership demand at every block
//! boundary to derive transfer, duplication, discharge, reuse, and COW facts.
//! It iterates the sparse [`AimsStateMap`] to convergence before deriving
//! borrow sources, shapes, TRMC facts, and FIP gates. Physical realization is
//! deliberately deferred.
//!
//! Diagnostic events from this subsystem use the shared
//! `ori_arc::aims::intraprocedural` target.

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use expect for clearer failure messages"
)]
mod tests;

pub(crate) mod apply_aliases;
pub(crate) mod birth_site_partition;
pub(crate) mod birth_site_population;
pub mod block;
mod block_state;
pub(crate) mod effects;
pub(crate) mod fip_balance;
pub(crate) mod ledger_events;
pub(crate) mod post_convergence;
pub(crate) mod project_aliases;
pub(crate) mod ssa_alias_classes;
pub mod state_map;

pub(crate) use fip_balance::compute_requires_unique_params;
pub use state_map::{AimsEvent, AimsStateMap, InvokeEdgeState};

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};
use crate::ArcClassification;

use super::contract::{ContextRegion, MemoryContract};
use super::lattice::AimsState;

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

/// Compute backward ownership demand to a fixed point for one ARC function.
///
/// Blocks run in successor-first postorder. The finite product lattice bounds
/// convergence by `15 × |variables| × |blocks|`; exceeding that bound widens
/// remaining states to TOP and emits a diagnostic.
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the project-wide hasher; no generic needed"
)]
pub fn analyze_function(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    context_regions: &[ContextRegion],
    immortals: Vec<bool>,
) -> AimsStateMap {
    let AnalysisSetup {
        mut state_map,
        postorder,
        invoke_defs,
        invoke_dst_to_owner,
        demand_sources,
        select_alias_dsts,
    } = initialize_analysis(func, classifier, sigs, immortals);
    let fixed_point = FixedPointContext {
        func,
        sigs,
        postorder: &postorder,
        invoke_defs: &invoke_defs,
        invoke_dst_to_owner: &invoke_dst_to_owner,
        demand_sources: &demand_sources,
        select_alias_dsts: &select_alias_dsts,
    };
    let iteration = fixed_point.run(&mut state_map);
    finish_analysis(func, sigs, context_regions, state_map, iteration)
}

struct AnalysisSetup {
    state_map: AimsStateMap,
    postorder: Vec<usize>,
    invoke_defs: FxHashMap<ArcBlockId, Vec<ArcVarId>>,
    invoke_dst_to_owner: FxHashMap<ArcVarId, ArcBlockId>,
    demand_sources: FxHashMap<ArcVarId, project_aliases::ProjectSources>,
    select_alias_dsts: FxHashSet<ArcVarId>,
}

fn initialize_analysis(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    immortals: Vec<bool>,
) -> AnalysisSetup {
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

    // Why: Backward demand traversal requires successors before predecessors.
    let postorder = crate::graph::compute_postorder(func);

    // INVARIANT: `Invoke` defines its destination only at the normal successor entry.
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    // SSA uniqueness lets pre-strip invoke-result demand route from its normal
    // successor back to the defining invoke's exit-state side table.
    let invoke_dst_to_owner = build_invoke_dst_to_owner(func);

    // PL-5 requires apply-result identities before project-alias sources so
    // the latter's seed sees the complete map.
    let apply_result_aliases = apply_aliases::populate_apply_result_aliases(func, sigs);
    state_map.set_apply_result_aliases(apply_result_aliases);

    // Build SSA alias classes after apply-result identities are available and
    // before the worklist begins its read-only use.
    let ssa_alias_output =
        ssa_alias_classes::compute_ssa_alias_classes(func, state_map.apply_result_aliases());
    state_map.set_ssa_alias_output(
        ssa_alias_output.class_table,
        ssa_alias_output.class_members,
        ssa_alias_output.class_apply_alias_source_candidates,
    );

    // Precompute the static same-allocation closure across projections, Let
    // aliases, jumps, merges, and selects; select origins gate §1.9 demand.
    let project_alias_table = project_aliases::compute_project_alias_table_for_state(
        func,
        state_map.apply_result_aliases(),
        &state_map,
    );
    let project_alias_sources = project_alias_table.sources;
    let demand_sources = project_alias_table.demand_sources;
    let select_alias_dsts = project_alias_table.select_alias_dsts;

    // Persist the full alias closure for realization, while backward demand
    // uses the narrower §1.9 sources; whole-var/select demand would overextend
    // merge-edge lifetimes. PL-5 makes this pre-pass state read-only thereafter.
    state_map.set_project_alias_sources(project_alias_sources);

    AnalysisSetup {
        state_map,
        postorder,
        invoke_defs,
        invoke_dst_to_owner,
        demand_sources,
        select_alias_dsts,
    }
}

struct FixedPointContext<'a> {
    func: &'a ArcFunction,
    sigs: &'a FxHashMap<Name, MemoryContract>,
    postorder: &'a [usize],
    invoke_defs: &'a FxHashMap<ArcBlockId, Vec<ArcVarId>>,
    invoke_dst_to_owner: &'a FxHashMap<ArcVarId, ArcBlockId>,
    demand_sources: &'a FxHashMap<ArcVarId, project_aliases::ProjectSources>,
    select_alias_dsts: &'a FxHashSet<ArcVarId>,
}

impl FixedPointContext<'_> {
    fn run(&self, state_map: &mut AimsStateMap) -> usize {
        let func = self.func;
        let sigs = self.sigs;
        let postorder = self.postorder;
        let invoke_defs = self.invoke_defs;
        let invoke_dst_to_owner = self.invoke_dst_to_owner;
        let demand_sources = self.demand_sources;
        let select_alias_dsts = self.select_alias_dsts;

        let iteration_limit = AimsState::iteration_limit(func.var_types.len(), func.blocks.len());
        let mut iteration = 0;

        loop {
            state_map.reset_changed();
            // Clear invoke-result demand each iteration so stale successor
            // states cannot mix convergence stages and break monotonicity.
            state_map.clear_invoke_def_demand();
            iteration += 1;

            // Process blocks in postorder (successors first for backward analysis).
            for &block_idx in postorder {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "ARC IR block counts fit in u32"
                )]
                let block_id = ArcBlockId::new(block_idx as u32);

                // Compute the block's exit state from successor entry states.
                let exit_state = block::compute_block_exit_state(func, block_id, state_map);
                state_map.update_block_exit(block_id, exit_state.demands);
                state_map.update_scalar_live_at_exit(block_id, exit_state.live_scalars);

                // Direct and indirect invokes need distinct normal/unwind
                // demand so cleanup can materialize the correct edge release.
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
                    state_map,
                    sigs,
                    invoke_defs,
                    demand_sources,
                    select_alias_dsts,
                );
                state_map.accumulate_effect(result.effects);
                state_map.update_block_entry(block_id, result.entry_state);
                state_map.update_scalar_live_at_entry(block_id, result.scalar_live_at_entry);

                // Route demand captured before successor stripping to the
                // defining invoke so its exit query sees post-definition demand.
                for (var, state) in result.invoke_def_demand {
                    if let Some(&owner_block) = invoke_dst_to_owner.get(&var) {
                        state_map.set_invoke_def_demand(owner_block, var, state);
                    }
                }

                // Preserve pre-strip intra-block definition demand so DP-2/DP-3
                // see converged cardinality for locally consumed variables.
                for (var, state) in result.def_demand {
                    state_map.set_def_demand(block_id, var, state);
                }
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
                widen_to_top(state_map, func);
                break;
            }
        }

        iteration
    }
}

fn finish_analysis(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    context_regions: &[ContextRegion],
    mut state_map: AimsStateMap,
    iteration: usize,
) -> AimsStateMap {
    verify_canonical_fixed_point(&mut state_map, func);

    tracing::debug!(
        func = ?func.name,
        iterations = iteration,
        blocks = func.blocks.len(),
        vars = func.var_types.len(),
        cross_dimension = state_map.cross_dimension_detected(),
        "AIMS intraprocedural analysis converged"
    );

    // Why: Placement candidates require effective exit locality before sparse events are populated.
    post_convergence::materialize_transitive_drop_singleton_classes(func, sigs, &mut state_map);
    post_convergence::populate_borrow_sources(&mut state_map, func);
    post_convergence::populate_call_result_states(&mut state_map, func, sigs);
    post_convergence::populate_sparse_events(&mut state_map, func);
    post_convergence::populate_var_shapes(&mut state_map, func);
    // TF-2 alias inheritance requires call-result and shape tables first.
    post_convergence::propagate_alias_forward_state(&mut state_map, func);

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
