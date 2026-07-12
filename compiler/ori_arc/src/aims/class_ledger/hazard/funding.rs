//! Extraction-funding cure: fund an endangered view at its member-defining
//! `Project` sites with RL-1 dup `BurdenInc` seeds (full-move arms become
//! bookkeeping credits), then re-plan + re-verify under OWNED semantics.

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::ledger_events::LedgerClassification;
use crate::ir::ArcFunction;

use super::super::emit::{self, PlannedOp};
use super::super::events;
use super::super::verify::ClassVerdict;
use super::super::ClassPlan;
use super::{commit_cured_view, plan_and_verify_cure};

/// Cure one endangered view class by funding it at its extraction sites: an
/// RL-1 dup `BurdenInc` right after each `Project` that defines a member
/// var, with the class re-planned and re-verified under OWNED semantics
/// (the container's recursive release and the view's independent reference
/// each balance). Returns `true` when the cured plan verifies `Clean`;
/// `false` leaves the original outcome in place (the replacement gate then
/// declines the function).
#[expect(
    clippy::too_many_arguments,
    reason = "internal cure pass over analyze_class_ledger's own accumulators"
)]
pub(super) fn cure_view_with_extraction_funding(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    preds: &[Vec<usize>],
    regions: &emit::CycleRegions,
    type_registry: &ori_types::TypeRegistry,
    full_move_arms: &[events::FullMoveArm],
    view: NodeIdx,
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
) -> bool {
    let Some((seeds, credit_sites)) =
        collect_extraction_seeds(func, partition, type_registry, full_move_arms, view)
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
    let credited_arrival = classification.blocks.iter().flatten().any(|ci| {
        matches!(ci,
            crate::aims::intraprocedural::ledger_events::ClassInstr::Credit { class }
                if partition.rep_of(*class) == view)
    });
    if credited_arrival {
        let credited_events =
            events::extract_class_events_with(func, classification, partition, view, true);
        if let Some(outcome) = plan_and_verify_cure(
            func,
            preds,
            regions,
            partition,
            view,
            "credited-arrival",
            &credited_events,
            &[],
        ) {
            return commit_cured_view(classes, verdicts, declined, view, outcome);
        }
    }
    if seeds.is_empty() && credit_sites.is_empty() {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(view),
            "view cure declined: no member-defining Project seeds"
        );
        return false;
    }
    let mut funded_events = if credit_sites.is_empty() {
        events::extract_class_events_with(func, classification, partition, view, true)
    } else {
        events::extract_class_events_with_extraction_credits(
            func,
            classification,
            partition,
            view,
            &credit_sites,
            true,
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
    let seeds = live_seeds(func, seeds, &event_vars, partition, view);
    // Pure-seed funding: every positive book entry is backed by a REAL
    // seed `Inc` this cure emits, so the books are runtime-grounded by
    // construction — a multi-owed dead-edge releases one front dec per
    // funded reference (the `books_runtime_grounded && exit > 1` path in
    // `plan_edge_releases`). Credit-mixed books stay fail-closed.
    if credit_sites.is_empty() && !seeds.is_empty() {
        funded_events.books_runtime_grounded = true;
    }
    let Some(outcome) = plan_and_verify_cure(
        func,
        preds,
        regions,
        partition,
        view,
        "extraction-funding",
        &funded_events,
        &seeds,
    ) else {
        return false;
    };
    commit_cured_view(classes, verdicts, declined, view, outcome)
}

/// Collect the funding seeds (one `Inc` per member-defining `Project`) and
/// the full-move CREDIT sites for `view`; `None` when a seed type carries
/// no burden (the inc cannot fund — decline).
#[expect(
    clippy::type_complexity,
    reason = "one caller; a named pair adds nothing"
)]
fn collect_extraction_seeds(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    full_move_arms: &[events::FullMoveArm],
    view: NodeIdx,
) -> Option<(Vec<PlannedOp>, Vec<(usize, usize)>)> {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::ArcInstr;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    let mut seeds = Vec::new();
    let mut credit_sites: Vec<(usize, usize)> = Vec::new();
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
            if full_move_arms.iter().any(|arm| {
                arm.block == block_idx && arm.projections.iter().any(|&(i, _)| i == index)
            }) {
                credit_sites.push((block_idx, index));
                continue;
            }
            // A seed inc funds ONLY a refcount-managed allocation. A view
            // type with no burden (an iterator handle: freed by destructor,
            // never by refcount) lowers the inc to nothing, so the container
            // release still destroys the extracted payload — decline. A
            // `FatValue` seed (str / closure) is ALWAYS refcount-managed —
            // its inc lowers unconditionally — so it stays fundable even
            // when the burden lookup cannot resolve a monomorphized-generic
            // pool alias of `str` (the generic-pair tuple-field shape).
            let fundable = matches!(func.var_repr(*dst), Some(crate::ir::ValueRepr::FatValue))
                || func.var_types.get(dst.index()).is_some_and(|&ty| {
                    lookup_burden(idx_to_type_ref(ty, type_registry), type_registry).is_some()
                });
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
    Some((seeds, credit_sites))
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
    seeds
        .into_iter()
        .filter(|seed| {
            let mut closure =
                emit::close_over_let_aliases(func, std::iter::once(seed.var).collect());
            loop {
                let mut grew = false;
                for arc_block in &func.blocks {
                    let crate::ir::ArcTerminator::Jump { target, args } = &arc_block.terminator
                    else {
                        continue;
                    };
                    let Some(target_block) = func.blocks.iter().find(|b| b.id == *target) else {
                        continue;
                    };
                    for (position, arg) in args.iter().enumerate() {
                        if closure.contains(arg) {
                            if let Some(&(param, _)) = target_block.params.get(position) {
                                if closure.insert(param) {
                                    grew = true;
                                }
                            }
                        }
                    }
                }
                if !grew {
                    break;
                }
                closure = emit::close_over_let_aliases(func, closure);
            }
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
