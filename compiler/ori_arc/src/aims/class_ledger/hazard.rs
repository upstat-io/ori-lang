//! Field-view hazard detection + cure ladder over released container classes.
//!
//! A locally-released container class (a planned dec or a consume event) may
//! have a SIBLING field-path view class with events of its own: the
//! container's recursive release and the view's cross-class liveness are not
//! modeled together unless cured here. Consumed by `analyze_class_ledger`
//! (`class_ledger::mod`), whose `field_view_hazard` result reflects whether
//! any endangered view went uncured.

use ori_ir::Name;

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::ledger_events::{EventSite, LedgerClassification};
use crate::ir::ArcFunction;

mod decompose;
mod detect;
mod funding;
mod skip_derive;
mod sum_arm;

pub(crate) use detect::field_view_hazard_classes;

use super::emit::{self, ClassOutcome, PlannedOp};
use super::events;
use super::verify::{self, ClassVerdict};
use super::ClassPlan;

/// One endangered (view, container) pair the cure passes consume: the
/// container's construct sites (the legitimate move-in stores), the view's
/// top-level field indices under the container (the `DecPartial` skip set),
/// and whether the view carries a Consume outside those sites (the consume
/// mark the skip derivation requires per PV-6).
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent per-hazard shape facts; no two flags encode one state machine"
)]
pub(crate) struct FieldViewHazard {
    pub(crate) view: NodeIdx,
    pub(crate) container: NodeIdx,
    construct_sites: Vec<(usize, EventSite)>,
    skip_fields: Vec<u32>,
    nested_path: bool,
    sum_container: bool,
    /// `Some(ordinal)` when EVERY construct site of the container builds the
    /// SAME single-payload enum variant — the sole sum shape whose skip is
    /// arm-safe (the moved-out slot exists only in that variant; every other
    /// arm's skip is vacuous). The `DecPartial` skip then names this variant
    /// ordinal per the tag-switched enum drop glue.
    sum_variant: Option<u32>,
    /// The uniform ctor's enum name (with `sum_variant`): the niche-family
    /// gate compares it against `Option`/`Result`, whose wrapper IS the
    /// payload allocation (skipping their payload drops the only release).
    sum_enum_name: Option<Name>,
    consume_marked: bool,
    /// Every container construct site is a PAYLOAD-LESS variant: no payload
    /// of any variant exists at runtime, so the endangered view is VACUOUS
    /// (the whole-var release everywhere is already correct — no cure).
    all_payloadless: bool,
}

/// Per-class facts the field-view hazard consumes.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent per-class facts; no two flags encode one state machine"
)]
#[derive(Debug)]
pub(crate) struct ClassHazardFacts {
    pub(crate) class: NodeIdx,
    pub(crate) released: bool,
    pub(crate) has_demand: bool,
    /// The class's own books fund its demand: NOT externally funded
    /// (real floors, never the container-held floor-0 discipline) AND
    /// verified `Clean`. A self-funded view's demand rides its own
    /// acquired reference, so a container release cannot strand it.
    pub(crate) self_funded_clean: bool,
    /// The class carries a `Credit` / `SelectCredit` re-acquisition — a
    /// reference that post-dates any move-in consume, so its demand stays
    /// self-funded even after its birth reference moved into a container.
    pub(crate) has_credit: bool,
    /// BORROWED-origin class verified `Clean`: its demand rides the
    /// CALLER's reference (RL-2 borrowed-param discipline,
    /// `RL2_borrowed_param_emits_caller_dec` — the caller retains and
    /// releases after the call), which no callee-local container release
    /// can strand, and every container-store hand-off carries its own
    /// borrowed-rooted funding inc (`plan_incs`), so a released container
    /// frees only the funded duplicate.
    pub(crate) borrowed_rooted_clean: bool,
    /// Planned funding `Inc` ops in the class's own outcome — each covers
    /// one consume beyond the birth-funded one (RL-1 duplication funding).
    pub(crate) planned_inc_count: usize,
    pub(crate) consume_sites: Vec<(usize, EventSite)>,
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
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    full_move_arms: &[events::FullMoveArm],
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
        // Vacuous endangerment: every container construct is a payload-less
        // variant, so the viewed payload never exists at runtime — the
        // whole-var release everywhere is already correct, no cure needed.
        if hazard.all_payloadless {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?partition.node_key(hazard.view),
                "endangered view vacuous: every container construct is payload-less"
            );
            cured_views.push(hazard.view);
            continue;
        }
        let multi_container = hazards
            .iter()
            .filter(|other| other.view == hazard.view)
            .count()
            > 1;
        if !multi_container
            && decompose::cure_view_with_field_decomposition(
                func,
                classification,
                partition,
                preds,
                regions,
                type_registry,
                interner,
                hazard,
                classes,
                verdicts,
                declined,
            )
        {
            cured_views.push(hazard.view);
            continue;
        }
        if funding::cure_view_with_extraction_funding(
            func,
            classification,
            partition,
            preds,
            regions,
            type_registry,
            full_move_arms,
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

/// Plan `events` (seeded with `seeds`) and verify the result against the
/// owed invariant; traces the failing gate under `cure_label` and returns
/// `None` for the caller to decline, or the clean [`ClassOutcome`] to commit.
/// Shared plan-and-verify skeleton for every cure ladder rung
/// ([`cure_view_with_extraction_funding`], [`cure_view_with_field_decomposition`]).
#[expect(
    clippy::too_many_arguments,
    reason = "internal cure pass over analyze_class_ledger's own accumulators"
)]
pub(super) fn plan_and_verify_cure(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    regions: &emit::CycleRegions,
    partition: &mut BirthSitePartition,
    view: NodeIdx,
    cure_label: &'static str,
    events: &events::ClassEvents,
    seeds: &[PlannedOp],
) -> Option<ClassOutcome> {
    let outcome = emit::plan_class(func, preds, regions, events, seeds);
    let planned: &[PlannedOp] = match &outcome {
        ClassOutcome::Planned(ops) => ops,
        ClassOutcome::Declined(reason) => {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                cure = cure_label,
                view = ?partition.node_key(view),
                declined = ?reason,
                seeds = seeds.len(),
                events = ?events.per_block,
                "cure declined: plan declined"
            );
            return None;
        }
    };
    let verdict = verify::verify_class(func, preds, events, planned);
    if verdict != ClassVerdict::Clean {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            cure = cure_label,
            view = ?partition.node_key(view),
            verdict = ?verdict,
            planned = ?planned,
            events = ?events.per_block,
            "cure declined: plan verifies non-Clean"
        );
        return None;
    }
    Some(outcome)
}

/// Commit a cured view's clean plan into the whole-function accumulators:
/// replace its outcome, flip its verdict to `Clean`, and drop it from the
/// declined list. Leaves the accumulators untouched and returns `false` when
/// `view`'s plan entry is not found.
pub(super) fn commit_cured_view(
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
    view: NodeIdx,
    outcome: ClassOutcome,
) -> bool {
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
