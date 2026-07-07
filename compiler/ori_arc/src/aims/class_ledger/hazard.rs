//! Field-view hazard detection + cure ladder over released container classes.
//!
//! A locally-released container class (a planned dec or a consume event) may
//! have a SIBLING field-path view class with events of its own: the
//! container's recursive release and the view's cross-class liveness are not
//! modeled together unless cured here. Consumed by `analyze_class_ledger`
//! (`class_ledger::mod`), whose `field_view_hazard` result reflects whether
//! any endangered view went uncured.

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::ledger_events::{EventSite, LedgerClassification};
use crate::ir::ArcFunction;

use super::emit::{self, ClassOutcome, PlannedOp};
use super::events;
use super::verify::{self, ClassVerdict};
use super::ClassPlan;

/// One endangered (view, container) pair the cure passes consume: the
/// container's construct sites (the legitimate move-in stores), the view's
/// top-level field indices under the container (the `DecPartial` skip set),
/// and whether the view carries a Consume outside those sites (the consume
/// mark the skip derivation requires per PV-6).
pub(crate) struct FieldViewHazard {
    pub(crate) view: NodeIdx,
    pub(crate) container: NodeIdx,
    construct_sites: Vec<(usize, EventSite)>,
    skip_fields: Vec<u32>,
    nested_path: bool,
    sum_container: bool,
    consume_marked: bool,
}

/// Per-class facts the field-view hazard consumes.
pub(crate) struct ClassHazardFacts {
    class: NodeIdx,
    released: bool,
    has_demand: bool,
    consume_sites: Vec<(usize, EventSite)>,
}

impl ClassHazardFacts {
    pub(crate) fn new(
        class: NodeIdx,
        released: bool,
        has_demand: bool,
        consume_sites: Vec<(usize, EventSite)>,
    ) -> Self {
        Self {
            class,
            released,
            has_demand,
            consume_sites,
        }
    }
}

/// Run the cure ladder over every endangered (view, container) pair; the
/// views no cure lands for come back uncured (the replacement gate then
/// declines the function). A consume-marked single-container view's precise
/// cure is the per-field release decomposition (zero added RC traffic — the
/// container's release skips the moved field); seed-funding is the general
/// fallback and the sole cure for demand-only views (never skipped per
/// IA-T6 over-skip rejection).
#[expect(
    clippy::too_many_arguments,
    reason = "internal cure ladder over analyze_class_ledger's own accumulators"
)]
pub(crate) fn cure_endangered_views(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    preds: &[Vec<usize>],
    regions: &emit::CycleRegions,
    hazards: &[FieldViewHazard],
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
) -> Vec<NodeIdx> {
    let mut uncured = Vec::new();
    let mut cured_views: Vec<NodeIdx> = Vec::new();
    for hazard in hazards {
        if cured_views.contains(&hazard.view) {
            continue;
        }
        let multi_container = hazards
            .iter()
            .filter(|other| other.view == hazard.view)
            .count()
            > 1;
        if !multi_container
            && cure_view_with_field_decomposition(
                func,
                classification,
                partition,
                preds,
                regions,
                hazard,
                classes,
                verdicts,
                declined,
            )
        {
            cured_views.push(hazard.view);
            continue;
        }
        if cure_view_with_extraction_funding(
            func,
            classification,
            partition,
            preds,
            regions,
            hazard.view,
            classes,
            verdicts,
            declined,
        ) {
            cured_views.push(hazard.view);
            continue;
        }
        uncured.push(hazard.view);
    }
    uncured
}

/// The field-path VIEW classes endangered by a locally-released container:
/// the container's recursive release would free the view's allocation while
/// the view still uses it (a demand event), or after the view moved OUT to a
/// new owner (a Consume anywhere but the container's own Construct sites).
/// Deduplicated, deterministic order.
pub(crate) fn field_view_hazard_classes(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class_facts: &[ClassHazardFacts],
) -> Vec<FieldViewHazard> {
    use crate::ir::ArcInstr;

    let released: Vec<NodeIdx> = class_facts
        .iter()
        .filter(|facts| facts.released)
        .map(|facts| facts.class)
        .collect();
    if released.is_empty() {
        return Vec::new();
    }
    let nodes = partition.nodes_snapshot();
    let mut hazards: Vec<FieldViewHazard> = Vec::new();
    for &container in &released {
        let container_rep = partition.rep_of(container);
        // Member vars of the container class (whole-var nodes).
        let member_vars: Vec<_> = nodes
            .iter()
            .filter(|(_, path, _)| path.is_whole_var())
            .filter(|&&(_, _, node)| partition.rep_of(node) == container_rep)
            .map(|&(var, _, _)| var)
            .collect();
        // The container class's own Construct sites: a view's Consume at one
        // of these is the move-in store the container's release pays for.
        let mut construct_sites: Vec<(usize, EventSite)> = Vec::new();
        let mut sum_container = false;
        for (block_idx, arc_block) in func.blocks.iter().enumerate() {
            for (index, instr) in arc_block.body.iter().enumerate() {
                let ArcInstr::Construct { dst, ctor, .. } = instr else {
                    continue;
                };
                if member_vars.contains(dst) {
                    construct_sites.push((block_idx, EventSite::Body(index)));
                    sum_container |= matches!(ctor, crate::ir::CtorKind::EnumVariant { .. });
                }
            }
        }
        for (var, path, node) in &nodes {
            if path.is_whole_var() || !member_vars.contains(var) {
                continue;
            }
            let view_rep = partition.rep_of(*node);
            if view_rep == container_rep {
                continue;
            }
            let consume_marked = class_facts.iter().any(|facts| {
                partition.rep_of(facts.class) == view_rep
                    && facts
                        .consume_sites
                        .iter()
                        .any(|site| !construct_sites.contains(site))
            });
            let endangered = consume_marked
                || class_facts
                    .iter()
                    .any(|facts| partition.rep_of(facts.class) == view_rep && facts.has_demand);
            if !endangered {
                continue;
            }
            if let Some(hazard) = hazards
                .iter_mut()
                .find(|hazard| hazard.view == view_rep && hazard.container == container_rep)
            {
                if let Some(index) = path.single_index() {
                    if !hazard.skip_fields.contains(&index) {
                        hazard.skip_fields.push(index);
                    }
                } else {
                    hazard.nested_path = true;
                }
                hazard.consume_marked |= consume_marked;
                continue;
            }
            let mut skip_fields = Vec::new();
            let mut nested_path = false;
            match path.single_index() {
                Some(index) => skip_fields.push(index),
                None => nested_path = true,
            }
            hazards.push(FieldViewHazard {
                view: view_rep,
                container: container_rep,
                construct_sites: construct_sites.clone(),
                skip_fields,
                nested_path,
                sum_container,
                consume_marked,
            });
        }
    }
    hazards.sort_unstable_by_key(|hazard| (hazard.view, hazard.container));
    hazards
}

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
fn cure_view_with_extraction_funding(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    preds: &[Vec<usize>],
    regions: &emit::CycleRegions,
    view: NodeIdx,
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
) -> bool {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::ArcInstr;

    let mut seeds = Vec::new();
    for (block_idx, arc_block) in func.blocks.iter().enumerate() {
        for (index, instr) in arc_block.body.iter().enumerate() {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = partition.register_node(*dst, FieldPath::whole_var());
            if partition.rep_of(node) != view {
                continue;
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
    if seeds.is_empty() {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(view),
            "view cure declined: no member-defining Project seeds"
        );
        return false;
    }
    let funded_events =
        events::extract_class_events_with(func, classification, partition, view, true);
    let outcome = emit::plan_class(func, preds, regions, &funded_events, &seeds);
    let planned: &[PlannedOp] = match &outcome {
        ClassOutcome::Planned(ops) => ops,
        ClassOutcome::Declined(reason) => {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?partition.node_key(view),
                declined = ?reason,
                seeds = seeds.len(),
                events = ?funded_events.per_block,
                "view cure declined: funded plan declined"
            );
            return false;
        }
    };
    let verdict = verify::verify_class(func, preds, &funded_events, planned);
    if verdict != ClassVerdict::Clean {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(view),
            verdict = ?verdict,
            planned = ?planned,
            events = ?funded_events.per_block,
            "view cure declined: funded plan verifies non-Clean"
        );
        return false;
    }
    let Some(entry) = classes.iter_mut().find(|plan| plan.class == view) else {
        return false;
    };
    entry.outcome = outcome;
    if let Some(slot) = verdicts.iter_mut().find(|(class, _)| *class == view) {
        slot.1 = ClassVerdict::Clean;
    }
    declined.retain(|&(class, _)| class != view);
    true
}

/// Cure one consume-marked endangered view by decomposing the container's
/// release per named owned field: the container's planned `Dec`s become
/// `DecPartial(skip = the view's field indices)` and the view's events are
/// RE-BOOKED with the move-in store non-consuming (ownership never enters
/// the container's release path), then re-planned + re-verified. The skip
/// set derives from the partition's consume marks and nothing else — the
/// UNIQUE clause-preserving skip set per IA-T6 `FD_skipset_sound`
/// (`aims-rules.md §12` PV-6). A merely-read (demand-endangered) view is
/// never consume-marked, so it is never skipped (over-skip = leak).
#[expect(
    clippy::too_many_arguments,
    reason = "internal cure pass over analyze_class_ledger's own accumulators"
)]
fn cure_view_with_field_decomposition(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    preds: &[Vec<usize>],
    regions: &emit::CycleRegions,
    hazard: &FieldViewHazard,
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
) -> bool {
    // A sum container's skip is a variant ordinal whose safety is
    // discriminant- and arm-conditional (the payload-less arm's books are
    // asymmetric); the per-class walk does not model per-arm variant state,
    // so sum containers stay on the funding/decline path fail-closed.
    if !hazard.consume_marked
        || hazard.nested_path
        || hazard.sum_container
        || hazard.skip_fields.is_empty()
    {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(hazard.view),
            consume_marked = hazard.consume_marked,
            nested_path = hazard.nested_path,
            sum_container = hazard.sum_container,
            skip_fields = ?hazard.skip_fields,
            "field-decomposition cure declined: view not skip-derivable"
        );
        return false;
    }
    let rebooked = events::extract_class_events_rebooked(
        func,
        classification,
        partition,
        hazard.view,
        &hazard.construct_sites,
    );
    let outcome = emit::plan_class(func, preds, regions, &rebooked, &[]);
    let planned: &[PlannedOp] = match &outcome {
        ClassOutcome::Planned(ops) => ops,
        ClassOutcome::Declined(reason) => {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?partition.node_key(hazard.view),
                declined = ?reason,
                events = ?rebooked.per_block,
                "field-decomposition cure declined: rebooked plan declined"
            );
            return false;
        }
    };
    let verdict = verify::verify_class(func, preds, &rebooked, planned);
    if verdict != ClassVerdict::Clean {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(hazard.view),
            verdict = ?verdict,
            planned = ?planned,
            events = ?rebooked.per_block,
            "field-decomposition cure declined: rebooked plan verifies non-Clean"
        );
        return false;
    }
    let container = hazard.container;
    let Some(container_entry) = classes
        .iter_mut()
        .find(|plan| partition.rep_of(plan.class) == container)
    else {
        return false;
    };
    let ClassOutcome::Planned(container_ops) = &mut container_entry.outcome else {
        return false;
    };
    for op in container_ops.iter_mut() {
        match &mut op.kind {
            emit::PlannedOpKind::Dec => {
                op.kind = emit::PlannedOpKind::DecPartial {
                    skip_fields: hazard.skip_fields.clone(),
                };
            }
            emit::PlannedOpKind::DecPartial { skip_fields } => {
                for &field in &hazard.skip_fields {
                    if !skip_fields.contains(&field) {
                        skip_fields.push(field);
                    }
                }
                skip_fields.sort_unstable();
            }
            emit::PlannedOpKind::Inc => {}
        }
    }
    let Some(entry) = classes.iter_mut().find(|plan| plan.class == hazard.view) else {
        return false;
    };
    entry.outcome = outcome;
    if let Some(slot) = verdicts.iter_mut().find(|(class, _)| *class == hazard.view) {
        slot.1 = ClassVerdict::Clean;
    }
    declined.retain(|&(class, _)| class != hazard.view);
    tracing::debug!(
        target: "ori_arc::aims::class_ledger",
        view = ?partition.node_key(hazard.view),
        container = ?partition.node_key(container),
        skip_fields = ?hazard.skip_fields,
        "field-decomposition cure applied: container releases skip consume-marked fields"
    );
    true
}
