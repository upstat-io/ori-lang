//! Extraction-funding cure: fund an endangered view at its member-defining
//! `Project` sites with RL-1 dup `BurdenInc` seeds (full-move arms become
//! bookkeeping credits), then re-plan + re-verify under OWNED semantics.
//! Trace events share the `ori_arc::aims::class_ledger` target across the cure ladder.

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::ir::ArcFunction;

use super::super::emit::{self, PlannedOp};
use super::super::events;
use super::{commit_cured_view, plan_and_verify_cure, HazardCureInputs, HazardCureState};

struct ExtractionSeeds {
    seeds: Vec<PlannedOp>,
    credit_sites: Vec<(usize, usize)>,
}

/// Cure one endangered view class by funding it at its extraction sites: an
/// RL-1 dup `BurdenInc` right after each `Project` that defines a member
/// var, with the class re-planned and re-verified under OWNED semantics
/// (the container's recursive release and the view's independent reference
/// each balance). Returns `true` when the cured plan verifies `Clean`;
/// `false` leaves the original outcome in place (the replacement gate then
/// declines the function).
pub(super) fn cure_view_with_extraction_funding(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    view: NodeIdx,
) -> bool {
    let Some(ExtractionSeeds {
        seeds,
        credit_sites,
    }) = collect_extraction_seeds(
        inputs.func,
        state.partition,
        inputs.type_registry,
        inputs.full_move_arms,
        view,
    )
    else {
        return false;
    };
    // Contract-boundary arrival: a call result whose callee contract proves
    // `return_alias = Project` over a borrowed arg (assert_some /
    // field-accessor shapes) books a `call_result_event` CREDIT (the PV-4
    // sharing-view producer: the callee minted the returned reference), so
    // the class's own books may already fund its demand with ZERO added
    // ops — try the un-seeded plan FIRST (a read-only local Project rides
    // the credited count; seeding it inflates the residue past what the
    // release placer can pair at a terminator-read block).
    let credited_arrival = inputs.classification.blocks.iter().flatten().any(|ci| {
        matches!(ci,
            crate::aims::intraprocedural::ledger_events::ClassInstr::Credit { class }
                if state.partition.rep_of(*class) == view)
    });
    if credited_arrival {
        let credited_events = events::extract_class_events_with(
            inputs.func,
            inputs.classification,
            state.partition,
            view,
            events::EventFunding::ExtractionOwned,
        );
        if let Some(outcome) = plan_and_verify_cure(
            inputs,
            state,
            view,
            "credited-arrival",
            &credited_events,
            &[],
        ) {
            return commit_cured_view(state, view, outcome);
        }
    }
    if seeds.is_empty() && credit_sites.is_empty() {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?state.partition.node_key(view),
            "view cure declined: no member-defining Project seeds"
        );
        return false;
    }
    let mut funded_events = if credit_sites.is_empty() {
        events::extract_class_events_with(
            inputs.func,
            inputs.classification,
            state.partition,
            view,
            events::EventFunding::ExtractionOwned,
        )
    } else {
        events::extract_class_events_with_extraction_credits(
            inputs.func,
            inputs.classification,
            state.partition,
            view,
            &credit_sites,
            events::EventFunding::ExtractionOwned,
        )
    };
    // A seed funds only a LIVE extract: a `Project` whose dst (through its
    // `Let` alias closure) carries NO event in the view class is a dead
    // binding (a match arm that never reads its payload) — seeding it
    // leaves an unreleasable +1 on that arm (merge disagreement); the
    // container's own release covers the untouched payload.
    let event_vars: rustc_hash::FxHashSet<crate::ir::ArcVarId> = funded_events
        .per_block
        .iter()
        .flatten()
        .filter_map(|ev| ev.var)
        .collect();
    let seeds = live_seeds(inputs.func, seeds, &event_vars, state.partition, view);
    // Pure-seed funding: every positive book entry is backed by a REAL
    // seed `Inc` this cure emits, so the books are runtime-grounded by
    // construction — a multi-owed dead-edge releases one front dec per
    // funded reference (the `books_runtime_grounded && exit > 1` path in
    // `plan_edge_releases`). Credit-mixed books stay fail-closed.
    if credit_sites.is_empty() && !seeds.is_empty() {
        funded_events.books_runtime_grounded = true;
    }
    let Some(outcome) = plan_and_verify_cure(
        inputs,
        state,
        view,
        "extraction-funding",
        &funded_events,
        &seeds,
    ) else {
        return false;
    };
    commit_cured_view(state, view, outcome)
}

/// Collect the funding seeds (one `Inc` per member-defining `Project`) and
/// the full-move CREDIT sites for `view`; `None` when a seed type carries
/// no burden (the inc cannot fund — decline).
fn collect_extraction_seeds(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    full_move_arms: &[events::FullMoveArm],
    view: NodeIdx,
) -> Option<ExtractionSeeds> {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::ArcInstr;

    let mut seeds = Vec::new();
    let mut credit_sites: Vec<(usize, usize)> = Vec::new();
    let full_move_projections: rustc_hash::FxHashSet<(usize, usize)> = full_move_arms
        .iter()
        .flat_map(|arm| {
            arm.projections
                .iter()
                .map(move |&(index, _)| (arm.block, index))
        })
        .collect();

    for (block_idx, arc_block) in func.blocks.iter().enumerate() {
        for (index, instr) in arc_block.body.iter().enumerate() {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = partition.register_node(*dst, FieldPath::whole_var());
            if partition.rep_of(node) != view {
                continue;
            }
            // A full-move arm's projection is NEVER seeded: the aggregate's
            // reference transfers whole (`apply_full_move_rebook`), so the
            // extraction re-acquires the transferred reference for free — a
            // bookkeeping CREDIT, not a runtime inc (a seed here bumps the
            // count once per arm execution with no matching release, since
            // the moved-out aggregate is never dropped).
            if full_move_projections.contains(&(block_idx, index)) {
                credit_sites.push((block_idx, index));
                continue;
            }
            // Transitional projection leak: the current `BurdenInc` carrier
            // can disappear during compiled-shaped lowering for a type with
            // no registered burden. Until logical events and physical-plan
            // satisfaction are separate carriers, decline rather than claim a
            // credit that the shipped adapter will not realize. `FatValue`
            // is retained here only as compatibility evidence for unresolved
            // monomorphized str/closure aliases; it is not an AIMS fact.
            let fundable = super::increment_is_materializable(func, *dst, type_registry);
            if !fundable {
                tracing::trace!(
                    target: "ori_arc::aims::class_ledger",
                    view = ?partition.node_key(view),
                    seed_var = ?dst,
                    seed_ty = ?func.var_types.get(dst.index()),
                    "view cure declined: seed type carries no burden (inc cannot fund)"
                );
                return None;
            }
            seeds.push(PlannedOp {
                slot: emit::PlanSlot::AfterBody {
                    block: block_idx,
                    index,
                },
                kind: emit::PlannedOpKind::Inc,
                var: *dst,
            });
        }
    }
    Some(ExtractionSeeds {
        seeds,
        credit_sites,
    })
}

/// Keep only seeds funding a LIVE extract: a `Project` whose dst — through
/// `Let` aliases AND `Jump`-arg -> block-param hand-offs — carries no event
/// in the view class is a dead binding (a match arm that never reads its
/// payload); seeding it leaves an unreleasable +1 on that arm.
fn live_seeds(
    func: &ArcFunction,
    seeds: Vec<PlannedOp>,
    event_vars: &rustc_hash::FxHashSet<crate::ir::ArcVarId>,
    partition: &mut BirthSitePartition,
    view: NodeIdx,
) -> Vec<PlannedOp> {
    let alias_flow = emit::AliasFlowGraph::new(func);
    seeds
        .into_iter()
        .filter(|seed| {
            let closure = alias_flow.close_let_aliases_and_jumps(std::iter::once(seed.var));
            let live = closure.iter().any(|member| event_vars.contains(member));
            if !live {
                tracing::trace!(
                    target: "ori_arc::aims::class_ledger",
                    view = ?partition.node_key(view),
                    seed_var = ?seed.var,
                    "extraction-funding seed skipped: dead extract (no event)"
                );
            }
            live
        })
        .collect()
}
