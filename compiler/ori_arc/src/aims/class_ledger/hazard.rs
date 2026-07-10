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
        if cure_view_with_extraction_funding(
            func,
            classification,
            partition,
            preds,
            regions,
            type_registry,
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
        let scan = container_construct_sites(func, &member_vars);
        let (construct_sites, sum_container, uniform_variant) =
            (scan.sites, scan.sum_container, scan.uniform_variant);
        let sum_enum_name = uniform_variant.map(|(name, _)| name);
        let sum_variant = uniform_variant.map(|(_, variant)| variant);
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
            // Demand endangers ONLY a view whose floors ride the
            // container's reference; a self-funded Clean view's demand is
            // covered by its own acquired reference (a credit / birth the
            // per-class verify already floored), so the container's
            // release cannot strand it. A birth CONSUMED at the container's
            // own construct site is NOT self-funding — the reference moved
            // INTO the container, so post-move demand rides the container's
            // reference after all. Consume marks endanger regardless
            // (double-ownership is about the move-out, not funding).
            let endangered = consume_marked
                || class_facts.iter().any(|facts| {
                    if partition.rep_of(facts.class) != view_rep || !facts.has_demand {
                        return false;
                    }
                    // Scoped to SUM containers: a variant arm's whole-var
                    // release lands on the extracting arm before the sibling
                    // views' reads (the multi-payload match shape); nested
                    // STRUCT chains interleave fund-before-release per level
                    // and stay balanced without the extra cure.
                    let funding_moved_in = sum_container
                        && !facts.has_credit
                        && facts
                            .consume_sites
                            .iter()
                            .any(|site| construct_sites.contains(site));
                    !facts.self_funded_clean || funding_moved_in
                });
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
                sum_variant,
                sum_enum_name,
                consume_marked,
            });
        }
    }
    hazards.sort_unstable_by_key(|hazard| (hazard.view, hazard.container));
    hazards
}

/// The container class's own `Construct` surface: the sites (a view's
/// Consume at one is the move-in store the container's release pays for),
/// whether any site is a sum-variant ctor, and — when EVERY site builds the
/// SAME single-payload variant — that uniform variant's identity.
struct ContainerConstructScan {
    sites: Vec<(usize, EventSite)>,
    sum_container: bool,
    uniform_variant: Option<(Name, u32)>,
}

fn container_construct_sites(
    func: &ArcFunction,
    member_vars: &[crate::ir::ArcVarId],
) -> ContainerConstructScan {
    use crate::ir::ArcInstr;

    let mut construct_sites: Vec<(usize, EventSite)> = Vec::new();
    let mut sum_container = false;
    // Unset -> Some(site) on the first ctor; a divergent later site (or a
    // non-variant / multi-payload ctor) poisons to Some(None).
    let mut uniform_variant: Option<Option<(Name, u32)>> = None;
    for (block_idx, arc_block) in func.blocks.iter().enumerate() {
        for (index, instr) in arc_block.body.iter().enumerate() {
            let ArcInstr::Construct {
                dst, ctor, args, ..
            } = instr
            else {
                continue;
            };
            if member_vars.contains(dst) {
                construct_sites.push((block_idx, EventSite::Body(index)));
                sum_container |= matches!(ctor, crate::ir::CtorKind::EnumVariant { .. });
                let site_variant = match ctor {
                    crate::ir::CtorKind::EnumVariant { enum_name, variant } if args.len() == 1 => {
                        Some((*enum_name, *variant))
                    }
                    _ => None,
                };
                uniform_variant = Some(match uniform_variant {
                    None => site_variant,
                    Some(prev) if prev == site_variant => prev,
                    Some(_) => None,
                });
            }
        }
    }
    ContainerConstructScan {
        sites: construct_sites,
        sum_container,
        uniform_variant: uniform_variant.flatten(),
    }
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
    type_registry: &ori_types::TypeRegistry,
    view: NodeIdx,
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
) -> bool {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::ArcInstr;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

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
            // A seed inc funds ONLY a refcount-managed allocation. A view
            // type with no burden (an iterator handle: freed by destructor,
            // never by refcount) lowers the inc to nothing, so the container
            // release still destroys the extracted payload — decline.
            let fundable = func.var_types.get(dst.index()).is_some_and(|&ty| {
                lookup_burden(idx_to_type_ref(ty, type_registry), type_registry).is_some()
            });
            if !fundable {
                tracing::trace!(
                    target: "ori_arc::aims::class_ledger",
                    view = ?partition.node_key(view),
                    seed_var = ?dst,
                    "view cure declined: seed type carries no burden (inc cannot fund)"
                );
                return false;
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

/// Plan `events` (seeded with `seeds`) and verify the result against the
/// owed invariant; traces the failing gate under `cure_label` and returns
/// `None` for the caller to decline, or the clean [`ClassOutcome`] to commit.
/// Shared plan-and-verify skeleton for every cure ladder rung
/// ([`cure_view_with_extraction_funding`], [`cure_view_with_field_decomposition`]).
#[expect(
    clippy::too_many_arguments,
    reason = "internal cure pass over analyze_class_ledger's own accumulators"
)]
fn plan_and_verify_cure(
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

/// Whether the container's type carries a USER enum burden with variant
/// entries — the explicit-slot sum layout whose payload is a separate
/// allocation the tag-switched drop glue walks. Builtin `Option`/`Result`
/// are same-allocation niche wrappers (the wrapper's dec IS the payload's
/// release), so skipping their payload drops the class's only release.
fn container_is_user_variant_enum(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    hazard: &FieldViewHazard,
) -> bool {
    use crate::lower::burden::BurdenRef;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    let Some((var, _)) = partition.node_key(hazard.container) else {
        return false;
    };
    let Some(&ty) = func.var_types.get(var.index()) else {
        return false;
    };
    match lookup_burden(idx_to_type_ref(ty, type_registry), type_registry) {
        Some(BurdenRef::User(user)) => !user.variant_burdens.is_empty(),
        _ => false,
    }
}

/// The `DecPartial` skip set for a SUM container — the moved-out variant's
/// ordinal — or `None` for a struct/tuple container. `Err(())` declines:
/// a sum container's skip is discriminant- and arm-conditional, so sums
/// decline (fail-closed) EXCEPT the uniform single-payload-variant shape
/// (every construct site builds the SAME one-payload variant and the view
/// is its sole payload slot; slot 0 is the tag), whose skip names the
/// variant ordinal per the tag-switched enum drop glue.
fn derive_sum_skip(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    hazard: &FieldViewHazard,
) -> Result<Option<Vec<u32>>, ()> {
    // Niche-family wrappers: the wrapper IS the payload allocation, so its
    // whole-var dec is the payload's release — never decomposable.
    let niche_family = hazard
        .sum_enum_name
        .is_some_and(|name| name == interner.intern("Option") || name == interner.intern("Result"));
    match (hazard.sum_container, hazard.sum_variant) {
        (false, _) => Ok(None),
        (true, Some(variant))
            if hazard.skip_fields == [1]
                && !niche_family
                && container_is_user_variant_enum(func, partition, type_registry, hazard) =>
        {
            Ok(Some(vec![variant]))
        }
        (true, _) => {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?partition.node_key(hazard.view),
                sum_variant = ?hazard.sum_variant,
                skip_fields = ?hazard.skip_fields,
                "field-decomposition cure declined: sum skip not arm-safe"
            );
            Err(())
        }
    }
}

/// Sum arm-safety over the container's PLANNED releases: every release site
/// must be dominated by an extraction of the payload (the moved-out
/// reference is gone there) or sit on a tag-switch arm that EXCLUDES the
/// skip variant (no such payload exists there). A release reachable with
/// the payload unextracted would skip a live payload's drop — a leak.
/// Vacuously safe for a struct/tuple container (`sum_skip = None`).
fn sum_release_sites_safe(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    hazard: &FieldViewHazard,
    classes: &[ClassPlan],
    sum_skip: Option<&[u32]>,
) -> bool {
    let Some(&variant) = sum_skip.and_then(|skip| skip.first()) else {
        return true;
    };
    let Some(container_entry) = classes
        .iter()
        .find(|plan| partition.rep_of(plan.class) == hazard.container)
    else {
        return false;
    };
    let ClassOutcome::Planned(container_ops) = &container_entry.outcome else {
        return false;
    };
    let safe = sum_skip_sites_arm_safe(
        func,
        partition,
        hazard.view,
        hazard.container,
        variant,
        container_ops,
    );
    if !safe {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(hazard.view),
            variant,
            "field-decomposition cure declined: sum release site neither \
             extraction-dominated nor tag-excluded"
        );
    }
    safe
}

/// Whether every container release site is arm-safe for a variant-ordinal
/// skip: dominated by an extraction of the view's payload (the reference
/// moved out before the release), or dominated by a tag-switch arm entry
/// that excludes the skip variant (no such payload exists on that arm).
fn sum_skip_sites_arm_safe(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    view: NodeIdx,
    container: NodeIdx,
    variant: u32,
    container_ops: &[emit::PlannedOp],
) -> bool {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::{ArcInstr, ArcTerminator};

    let dom = crate::graph::DominatorTree::build(func);
    // Extraction sites: member-defining Projects of the view class.
    let mut extractions: Vec<(usize, usize)> = Vec::new();
    // The container class's member vars (tag-switch scrutinee detection).
    let nodes = partition.nodes_snapshot();
    let container_members: Vec<crate::ir::ArcVarId> = nodes
        .iter()
        .filter(|(_, path, _)| path.is_whole_var())
        .filter(|&&(_, _, node)| partition.rep_of(node) == container)
        .map(|&(var, _, _)| var)
        .collect();
    for (block_idx, arc_block) in func.blocks.iter().enumerate() {
        for (index, instr) in arc_block.body.iter().enumerate() {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = partition.register_node(*dst, FieldPath::whole_var());
            if partition.rep_of(node) == view {
                extractions.push((block_idx, index));
            }
        }
    }
    // Tag-excluded arm entries: switch arms (over a container tag read —
    // `Project <member>.0`) whose case value is NOT the skip variant, plus
    // the default arm when the skip variant is an explicit case.
    let mut tag_defs: Vec<crate::ir::ArcVarId> = Vec::new();
    for arc_block in &func.blocks {
        for instr in &arc_block.body {
            if let ArcInstr::Project {
                dst,
                value,
                field: 0,
                ..
            } = instr
            {
                if container_members.contains(value) {
                    tag_defs.push(*dst);
                }
            }
        }
    }
    let mut excluded_entries: Vec<usize> = Vec::new();
    for arc_block in &func.blocks {
        let ArcTerminator::Switch {
            scrutinee,
            cases,
            default,
        } = &arc_block.terminator
        else {
            continue;
        };
        if !tag_defs.contains(scrutinee) {
            continue;
        }
        for &(value, target) in cases {
            if value != u64::from(variant) {
                excluded_entries.push(target.index());
            }
        }
        if cases.iter().any(|&(value, _)| value == u64::from(variant)) {
            excluded_entries.push(default.index());
        }
    }
    container_ops.iter().all(|op| {
        if !matches!(
            op.kind,
            emit::PlannedOpKind::Dec | emit::PlannedOpKind::DecPartial { .. }
        ) {
            return true;
        }
        let block = op.slot.block();
        let block_id = func.blocks[block].id;
        let extraction_dominates = extractions.iter().any(|&(pb, pi)| {
            if pb == block {
                // Same block: the extraction must precede the release slot.
                match op.slot {
                    emit::PlanSlot::AfterBody { index, .. } => index >= pi,
                    emit::PlanSlot::BeforeBody { index, .. } => index > pi,
                    emit::PlanSlot::BeforeTerminator { .. } => true,
                    emit::PlanSlot::BlockFront { .. } => false,
                }
            } else {
                dom.dominates(func.blocks[pb].id, block_id)
            }
        });
        let tag_excluded = excluded_entries
            .iter()
            .any(|&entry| dom.dominates(func.blocks[entry].id, block_id));
        extraction_dominates || tag_excluded
    })
}

/// Commit a cured view's clean plan into the whole-function accumulators:
/// replace its outcome, flip its verdict to `Clean`, and drop it from the
/// declined list. Leaves the accumulators untouched and returns `false` when
/// `view`'s plan entry is not found.
fn commit_cured_view(
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

/// Cure one consume-marked endangered view by decomposing the container's
/// release per named owned field: the container's planned `Dec`s become
/// `DecPartial(skip = the view's field indices)` and the view's events are
/// RE-BOOKED with the move-in store non-consuming (ownership never enters
/// the container's release path), then re-planned + re-verified. The skip
/// set derives solely from the partition's consume marks — the UNIQUE
/// clause-preserving skip set per IA-T6 `FD_skipset_sound` (`aims-rules.md
/// §12` PV-6). A merely-read view is never consume-marked, never skipped (over-skip = leak).
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
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    hazard: &FieldViewHazard,
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
) -> bool {
    let Ok(sum_skip) = derive_sum_skip(func, partition, type_registry, interner, hazard) else {
        return false;
    };
    if !hazard.consume_marked
        || hazard.nested_path
        || hazard.construct_sites.is_empty()
        || hazard.skip_fields.is_empty()
    {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(hazard.view),
            consume_marked = hazard.consume_marked,
            nested_path = hazard.nested_path,
            sum_container = hazard.sum_container,
            constructless = hazard.construct_sites.is_empty(),
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
    let Some(outcome) = plan_and_verify_cure(
        func,
        preds,
        regions,
        partition,
        hazard.view,
        "field-decomposition",
        &rebooked,
        &[],
    ) else {
        return false;
    };
    let container = hazard.container;
    if !sum_release_sites_safe(func, partition, hazard, classes, sum_skip.as_deref()) {
        return false;
    }
    let Some(container_entry) = classes
        .iter_mut()
        .find(|plan| partition.rep_of(plan.class) == container)
    else {
        return false;
    };
    let ClassOutcome::Planned(container_ops) = &mut container_entry.outcome else {
        return false;
    };
    // Struct/tuple skips name top-level field indices; the admitted sum
    // shape's skip names the moved-out VARIANT ordinal instead.
    let dec_skip = sum_skip.as_ref().unwrap_or(&hazard.skip_fields);
    for op in container_ops.iter_mut() {
        match &mut op.kind {
            emit::PlannedOpKind::Dec => {
                op.kind = emit::PlannedOpKind::DecPartial {
                    skip_fields: dec_skip.clone(),
                };
            }
            emit::PlannedOpKind::DecPartial { skip_fields } => {
                for &field in dec_skip {
                    if !skip_fields.contains(&field) {
                        skip_fields.push(field);
                    }
                }
                skip_fields.sort_unstable();
            }
            emit::PlannedOpKind::Inc => {}
        }
    }
    if !commit_cured_view(classes, verdicts, declined, hazard.view, outcome) {
        return false;
    }
    tracing::debug!(
        target: "ori_arc::aims::class_ledger",
        view = ?partition.node_key(hazard.view),
        container = ?partition.node_key(container),
        skip_fields = ?hazard.skip_fields,
        "field-decomposition cure applied: container releases skip consume-marked fields"
    );
    true
}
