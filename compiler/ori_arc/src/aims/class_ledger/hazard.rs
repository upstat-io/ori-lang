//! Field-view hazard detection and cures for released container classes.
//!
//! A cure reconciles recursive container release with the independent
//! liveness of field-path views. The result identifies any uncured view.

use ori_ir::Name;
use rustc_hash::FxHashSet;

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

/// Compact storage for independent analysis facts carried together in hot
/// per-class and per-hazard records.
#[derive(Clone, Copy, Debug, Default)]
struct CompactFlags(u8);

impl CompactFlags {
    const EMPTY: Self = Self(0);

    const fn with(mut self, flag: u8, enabled: bool) -> Self {
        if enabled {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
        self
    }

    const fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    fn insert(&mut self, flag: u8) {
        self.0 |= flag;
    }
}

/// Compact per-class hazard facts populated by the ledger planner.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClassHazardFlags(CompactFlags);

impl ClassHazardFlags {
    const RELEASED: u8 = 0b0000_0001;
    const HAS_DEMAND: u8 = 0b0000_0010;
    const SELF_FUNDED_CLEAN: u8 = 0b0000_0100;
    const HAS_CREDIT: u8 = 0b0000_1000;
    const BORROWED_ROOTED_CLEAN: u8 = 0b0001_0000;

    pub(crate) const EMPTY: Self = Self(CompactFlags::EMPTY);

    pub(crate) const fn with_released(mut self, released: bool) -> Self {
        self.0 = self.0.with(Self::RELEASED, released);
        self
    }

    pub(crate) const fn with_demand(mut self, has_demand: bool) -> Self {
        self.0 = self.0.with(Self::HAS_DEMAND, has_demand);
        self
    }

    pub(crate) const fn with_self_funded_clean(mut self, self_funded_clean: bool) -> Self {
        self.0 = self.0.with(Self::SELF_FUNDED_CLEAN, self_funded_clean);
        self
    }

    pub(crate) const fn with_credit(mut self, has_credit: bool) -> Self {
        self.0 = self.0.with(Self::HAS_CREDIT, has_credit);
        self
    }

    pub(crate) const fn with_borrowed_rooted_clean(mut self, borrowed_rooted_clean: bool) -> Self {
        self.0 = self
            .0
            .with(Self::BORROWED_ROOTED_CLEAN, borrowed_rooted_clean);
        self
    }
}

/// One endangered (view, container) pair consumed by the cure passes.
#[derive(Debug)]
pub(crate) struct FieldViewHazard {
    pub(crate) view: NodeIdx,
    pub(crate) container: NodeIdx,
    /// The container's legitimate move-in stores.
    construct_sites: Vec<(usize, EventSite)>,
    skip_fields: Vec<u32>,
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
    /// Transfer, path, container-shape, consume, and payload-presence facts.
    flags: CompactFlags,
}

/// Per-class facts the field-view hazard consumes.
#[derive(Debug)]
pub(crate) struct ClassHazardFacts {
    pub(crate) class: NodeIdx,
    /// Release, demand, funding, credit, and borrowed-root facts.
    flags: ClassHazardFlags,
    /// Planned funding `Inc` ops in the class's own outcome — each covers
    /// one consume beyond the birth-funded one (RL-1 duplication funding).
    pub(crate) planned_inc_count: usize,
    pub(crate) consume_sites: Vec<(usize, EventSite)>,
}

/// Immutable facts shared by every rung of the field-view cure ladder.
pub(crate) struct HazardCureInputs<'a> {
    func: &'a ArcFunction,
    classification: &'a LedgerClassification,
    preds: &'a [Vec<usize>],
    regions: &'a emit::CycleRegions,
    type_registry: &'a ori_types::TypeRegistry,
    interner: &'a ori_ir::StringInterner,
    full_move_arms: &'a [events::FullMoveArm],
}

// CFG regions and full-move carriers own separate diagnostic surfaces. Keep
// them opaque while exposing the cure input's structural dimensions.
impl std::fmt::Debug for HazardCureInputs<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HazardCureInputs")
            .field("predecessor_blocks", &self.preds.len())
            .field("full_move_arm_count", &self.full_move_arms.len())
            .finish_non_exhaustive()
    }
}

impl<'a> HazardCureInputs<'a> {
    pub(crate) fn new(
        func: &'a ArcFunction,
        classification: &'a LedgerClassification,
        preds: &'a [Vec<usize>],
        regions: &'a emit::CycleRegions,
        type_registry: &'a ori_types::TypeRegistry,
        interner: &'a ori_ir::StringInterner,
        full_move_arms: &'a [events::FullMoveArm],
    ) -> Self {
        Self {
            func,
            classification,
            preds,
            regions,
            type_registry,
            interner,
            full_move_arms,
        }
    }
}

/// Mutable whole-function accumulators shared by every cure attempt.
pub(crate) struct HazardCureState<'a> {
    partition: &'a mut BirthSitePartition,
    classes: &'a mut [ClassPlan],
    verdicts: &'a mut [(NodeIdx, ClassVerdict)],
    declined: &'a mut Vec<(NodeIdx, emit::DeclineReason)>,
}

// The mutable birth-site partition has its own diagnostic owner. Report the
// cure accumulators without requiring that owner to expose its internals.
impl std::fmt::Debug for HazardCureState<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HazardCureState")
            .field("class_count", &self.classes.len())
            .field("verdict_count", &self.verdicts.len())
            .field("declined_count", &self.declined.len())
            .finish_non_exhaustive()
    }
}

impl<'a> HazardCureState<'a> {
    pub(crate) fn new(
        partition: &'a mut BirthSitePartition,
        classes: &'a mut [ClassPlan],
        verdicts: &'a mut [(NodeIdx, ClassVerdict)],
        declined: &'a mut Vec<(NodeIdx, emit::DeclineReason)>,
    ) -> Self {
        Self {
            partition,
            classes,
            verdicts,
            declined,
        }
    }
}

impl FieldViewHazard {
    const CONTAINER_TRANSFERRED_OUT: u8 = 0b0000_0001;
    const NESTED_PATH: u8 = 0b0000_0010;
    const SUM_CONTAINER: u8 = 0b0000_0100;
    const CONSUME_MARKED: u8 = 0b0000_1000;
    const ALL_PAYLOADLESS: u8 = 0b0001_0000;

    fn is_container_transferred_out(&self) -> bool {
        self.flags.contains(Self::CONTAINER_TRANSFERRED_OUT)
    }

    fn is_nested_path(&self) -> bool {
        self.flags.contains(Self::NESTED_PATH)
    }

    fn mark_nested_path(&mut self) {
        self.flags.insert(Self::NESTED_PATH);
    }

    fn is_sum_container(&self) -> bool {
        self.flags.contains(Self::SUM_CONTAINER)
    }

    fn is_consume_marked(&self) -> bool {
        self.flags.contains(Self::CONSUME_MARKED)
    }

    fn mark_consume(&mut self, consume_marked: bool) {
        if consume_marked {
            self.flags.insert(Self::CONSUME_MARKED);
        }
    }

    fn is_all_payloadless(&self) -> bool {
        self.flags.contains(Self::ALL_PAYLOADLESS)
    }
}

impl ClassHazardFacts {
    pub(crate) fn new(
        class: NodeIdx,
        flags: ClassHazardFlags,
        planned_inc_count: usize,
        consume_sites: Vec<(usize, EventSite)>,
    ) -> Self {
        Self {
            class,
            flags,
            planned_inc_count,
            consume_sites,
        }
    }

    fn is_released(&self) -> bool {
        self.flags.0.contains(ClassHazardFlags::RELEASED)
    }

    fn has_demand(&self) -> bool {
        self.flags.0.contains(ClassHazardFlags::HAS_DEMAND)
    }

    fn is_self_funded_clean(&self) -> bool {
        self.flags.0.contains(ClassHazardFlags::SELF_FUNDED_CLEAN)
    }

    fn has_credit(&self) -> bool {
        self.flags.0.contains(ClassHazardFlags::HAS_CREDIT)
    }

    fn is_borrowed_rooted_clean(&self) -> bool {
        self.flags
            .0
            .contains(ClassHazardFlags::BORROWED_ROOTED_CLEAN)
    }
}

/// Run the cure ladder over every endangered (view, container) pair; the
/// views no cure lands for come back uncured (the replacement gate then
/// declines the function). A consume-marked single-container view's precise
/// cure is the per-field release decomposition (zero added RC traffic — the
/// container's release skips the moved field); seed-funding is the general
/// fallback and the sole cure for demand-only views (never skipped per
/// IA-T6 over-skip rejection).
pub(crate) fn cure_endangered_views(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    hazards: &[FieldViewHazard],
) -> Vec<NodeIdx> {
    let mut uncured = Vec::new();
    let mut seen_views = FxHashSet::default();
    let mut multi_container_views = FxHashSet::default();
    for hazard in hazards {
        if !seen_views.insert(hazard.view) {
            multi_container_views.insert(hazard.view);
        }
    }

    let mut cured_views = FxHashSet::default();
    for hazard in hazards {
        if cured_views.contains(&hazard.view) {
            continue;
        }
        // Vacuous endangerment: every container construct is a payload-less
        // variant, so the viewed payload never exists at runtime — the
        // whole-var release everywhere is already correct, no cure needed.
        if hazard.is_all_payloadless() {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?state.partition.node_key(hazard.view),
                "endangered view vacuous: every container construct is payload-less"
            );
            cured_views.insert(hazard.view);
            continue;
        }
        if !multi_container_views.contains(&hazard.view)
            && decompose::cure_view_with_field_decomposition(inputs, state, hazard)
        {
            cured_views.insert(hazard.view);
            continue;
        }
        if funding::cure_view_with_extraction_funding(inputs, state, hazard.view) {
            cured_views.insert(hazard.view);
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
pub(super) fn plan_and_verify_cure(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    view: NodeIdx,
    cure_label: &'static str,
    events: &events::ClassEvents,
    seeds: &[PlannedOp],
) -> Option<ClassOutcome> {
    let outcome = emit::plan_class(inputs.func, inputs.preds, inputs.regions, events, seeds);
    let planned: &[PlannedOp] = match &outcome {
        ClassOutcome::Planned(ops) => ops,
        ClassOutcome::Declined(reason) => {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                cure = cure_label,
                view = ?state.partition.node_key(view),
                declined = ?reason,
                seeds = seeds.len(),
                events = ?events.per_block,
                "cure declined: plan declined"
            );
            return None;
        }
    };
    let verdict = verify::verify_class(inputs.func, inputs.preds, events, planned);
    if verdict != ClassVerdict::Clean {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            cure = cure_label,
            view = ?state.partition.node_key(view),
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
    state: &mut HazardCureState<'_>,
    view: NodeIdx,
    outcome: ClassOutcome,
) -> bool {
    let Some(entry) = state.classes.iter_mut().find(|plan| plan.class == view) else {
        return false;
    };
    entry.outcome = outcome;
    if let Some(slot) = state.verdicts.iter_mut().find(|(class, _)| *class == view) {
        slot.1 = ClassVerdict::Clean;
    }
    state.declined.retain(|&(class, _)| class != view);
    true
}
