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

/// Whether any view-member Project or container planned-release block sits
/// in a CFG cycle — the acyclicity gate for POSITIONAL skip authority.
fn func_has_cycle_touching_view_or_container(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    regions: &emit::CycleRegions,
    hazard: &FieldViewHazard,
    classes: &[ClassPlan],
) -> bool {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::ArcInstr;

    for (block_idx, arc_block) in func.blocks.iter().enumerate() {
        for instr in &arc_block.body {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = partition.register_node(*dst, FieldPath::whole_var());
            if partition.rep_of(node) == hazard.view && regions.is_in_cycle(block_idx) {
                return true;
            }
        }
    }
    let Some(container_entry) = classes
        .iter()
        .find(|plan| partition.rep_of(plan.class) == hazard.container)
    else {
        return true;
    };
    let ClassOutcome::Planned(container_ops) = &container_entry.outcome else {
        return true;
    };
    container_ops
        .iter()
        .any(|op| regions.is_in_cycle(op.slot.block()))
}

/// Whether one view class's consumes mark it moved-OUT relative to a
/// container with `construct_sites`: a Consume that is neither this
/// container's own move-in store nor a FUNDED move-in at another released
/// container's Construct (one birth + one planned `Inc` per consume beyond
/// the first — the two-wrappers-share-one-inner shape; an unfunded
/// extract-then-restore stays marked).
fn facts_consume_marked(
    facts: &ClassHazardFacts,
    construct_sites: &[(usize, EventSite)],
    released_construct_union: &[(usize, EventSite)],
) -> bool {
    let extra: Vec<&(usize, EventSite)> = facts
        .consume_sites
        .iter()
        .filter(|site| !construct_sites.contains(site))
        .collect();
    if extra.is_empty() {
        return false;
    }
    let all_extra_at_released_constructs = extra
        .iter()
        .all(|site| released_construct_union.contains(site));
    let funded = facts.consume_sites.len() <= 1 + facts.planned_inc_count;
    !(all_extra_at_released_constructs && funded)
}

/// Whether a view class's DEMAND is endangered by a released container.
/// Demand endangers ONLY a view whose floors ride the container's reference:
/// a self-funded Clean view's demand is covered by its own acquired
/// reference, and a Clean BORROWED-rooted view's demand rides the CALLER's
/// reference with every store hand-off funded
/// (see `ClassHazardFacts::borrowed_rooted_clean`). A birth CONSUMED at a
/// SUM container's own construct site is NOT self-funding — the reference
/// moved INTO the container (the multi-payload match shape); nested STRUCT
/// chains interleave fund-before-release per level and stay balanced.
fn view_demand_endangered(
    class_facts: &[ClassHazardFacts],
    partition: &mut BirthSitePartition,
    view_rep: NodeIdx,
    sum_container: bool,
    construct_sites: &[(usize, EventSite)],
) -> bool {
    class_facts.iter().any(|facts| {
        if partition.rep_of(facts.class) != view_rep || !facts.has_demand {
            return false;
        }
        if facts.borrowed_rooted_clean {
            return false;
        }
        let funding_moved_in = sum_container
            && !facts.has_credit
            && facts
                .consume_sites
                .iter()
                .any(|site| construct_sites.contains(site));
        !facts.self_funded_clean || funding_moved_in
    })
}

/// The field-path VIEW classes endangered by a locally-released container:
/// the container's recursive release would free the view's allocation while
/// the view still uses it (a demand event), or after the view moved OUT to a
/// new owner — a Consume that is neither this container's own move-in store
/// nor (for a plan-funded view) a funded move-in at ANOTHER released
/// container's Construct site (`facts_consume_marked`). Deduplicated,
/// deterministic order.
pub(crate) fn field_view_hazard_classes(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class_facts: &[ClassHazardFacts],
    full_move_construct_sites: &[(usize, EventSite)],
    user_drop_admitted: &rustc_hash::FxHashSet<crate::ir::ArcVarId>,
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
    // Every released container's construct surface, up front: a view's
    // Consume at ANY released container's Construct is a move-in store that
    // container's own planned release pays for (the two-wrappers-share-one-
    // inner shape: each store funded per RL-1, each wrapper's drop the
    // matched release) — never a move-out of THIS container.
    let scans: Vec<(NodeIdx, Vec<crate::ir::ArcVarId>, ContainerConstructScan)> = released
        .iter()
        .map(|&container| {
            let container_rep = partition.rep_of(container);
            let member_vars: Vec<_> = nodes
                .iter()
                .filter(|(_, path, _)| path.is_whole_var())
                .filter(|&&(_, _, node)| partition.rep_of(node) == container_rep)
                .map(|&(var, _, _)| var)
                .collect();
            let scan = container_construct_sites(func, &member_vars);
            (container, member_vars, scan)
        })
        .collect();
    // Full-move arm Construct sites join the funded union: the arm's
    // transfer is self-accounting — the extraction credit funds the store
    // and the receiving container's lineage carries the reference to its
    // own release (`apply_full_move_rebook` + the injected extraction
    // credits; the per-class verify re-checks the books independently).
    let released_construct_union: Vec<(usize, EventSite)> = scans
        .iter()
        .flat_map(|(_, _, scan)| scan.sites.iter().copied())
        .chain(full_move_construct_sites.iter().copied())
        .collect();
    let mut hazards: Vec<FieldViewHazard> = Vec::new();
    for (container, member_vars, scan) in &scans {
        if is_admitted_scalar_container(member_vars, user_drop_admitted) {
            continue;
        }
        let container_rep = partition.rep_of(*container);
        let all_payloadless = scan.all_payloadless;
        let (construct_sites, sum_container, uniform_variant) =
            (scan.sites.clone(), scan.sum_container, scan.uniform_variant);
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
                    && facts_consume_marked(facts, &construct_sites, &released_construct_union)
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
                || view_demand_endangered(
                    class_facts,
                    partition,
                    view_rep,
                    sum_container,
                    &construct_sites,
                );
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
                all_payloadless,
            });
        }
    }
    hazards.sort_unstable_by_key(|hazard| (hazard.view, hazard.container));
    hazards
}

/// Whether the container class roots in an ADMITTED user-drop SCALAR var:
/// such a container releases via the balance-neutral `@drop` call only —
/// nothing is freed, so a field-path view read after the release stays
/// valid (per `AimsProof.Realization::RLDROP_user_drop_balance_neutral`;
/// the legacy walk emits the same read-after-release ordering).
fn is_admitted_scalar_container(
    member_vars: &[crate::ir::ArcVarId],
    user_drop_admitted: &rustc_hash::FxHashSet<crate::ir::ArcVarId>,
) -> bool {
    member_vars
        .iter()
        .any(|var| user_drop_admitted.contains(var))
}

/// The container class's own `Construct` surface: the sites (a view's
/// Consume at one is the move-in store the container's release pays for),
/// whether any site is a sum-variant ctor, and — when EVERY site builds the
/// SAME single-payload variant — that uniform variant's identity.
struct ContainerConstructScan {
    sites: Vec<(usize, EventSite)>,
    sum_container: bool,
    uniform_variant: Option<(Name, u32)>,
    /// Every construct site is a PAYLOAD-LESS variant (arity 0): no payload
    /// of ANY variant exists, so a field-path view of the container is
    /// vacuous — the whole-var release everywhere is already correct.
    all_payloadless: bool,
}

fn container_construct_sites(
    func: &ArcFunction,
    member_vars: &[crate::ir::ArcVarId],
) -> ContainerConstructScan {
    use crate::ir::ArcInstr;

    let mut construct_sites: Vec<(usize, EventSite)> = Vec::new();
    let mut sum_container = false;
    let mut all_payloadless = true;
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
                all_payloadless &= args.is_empty();
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
    let all_payloadless = all_payloadless && !construct_sites.is_empty();
    ContainerConstructScan {
        sites: construct_sites,
        sum_container,
        uniform_variant: uniform_variant.flatten(),
        all_payloadless,
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

/// Type-derived variant skip for a CONSTRUCTLESS sum container (call- or
/// merge-produced — no `Construct` in the container class to inspect):
/// the container type's burden table names EXACTLY ONE payload-bearing
/// variant carrying ONE payload slot, so any extracted payload belongs to
/// that variant — the moved mark is variant-unique by type structure
/// (`FD_skipset_sound`; the construct-uniform premise met by the type
/// instead of the construct sites). `self_heap_alloc` required: a
/// niche-family wrapper (no own allocation) is never decomposable — its
/// whole-var dec IS the payload's release.
fn derive_constructless_enum_variant(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    hazard: &FieldViewHazard,
) -> Option<Vec<u32>> {
    use crate::lower::burden::BurdenRef;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    if !hazard.construct_sites.is_empty() || hazard.skip_fields != [1] {
        return None;
    }
    let (var, _) = partition.node_key(hazard.container)?;
    let &ty = func.var_types.get(var.index())?;
    let Some(BurdenRef::User(user)) =
        lookup_burden(idx_to_type_ref(ty, type_registry), type_registry)
    else {
        return None;
    };
    if !user.self_heap_alloc {
        return None;
    }
    let mut payload_bearing = user
        .variant_burdens
        .iter()
        .filter(|v| !v.retained_owned.is_empty() || !v.transfers_on_match.is_empty());
    let unique = payload_bearing.next()?;
    if payload_bearing.next().is_some() || unique.retained_owned.len() != 1 {
        return None;
    }
    // `VariantId` is 1-indexed; the skip names the 0-based ordinal the
    // tag-switched drop glue tests.
    Some(vec![unique.variant_id.get().get() - 1])
}

/// The container's skip authority: HOW its `DecPartial` skip set is keyed.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SkipAuthority {
    /// A SUM container: the skip names the moved-out VARIANT ordinal
    /// (tag-switched enum drop glue).
    Variant(Vec<u32>),
    /// A STRUCT/TUPLE container: the skip names top-level FIELD ordinals
    /// (positional field-walk drop glue).
    Positional(Vec<u32>),
}

impl SkipAuthority {
    fn skip_fields(&self) -> &[u32] {
        match self {
            Self::Variant(skip) | Self::Positional(skip) => skip,
        }
    }

    /// The variant ordinal for tag-exclusion; `None` for positional skips.
    fn variant_ordinal(&self) -> Option<u32> {
        match self {
            Self::Variant(skip) => skip.first().copied(),
            Self::Positional(_) => None,
        }
    }
}

/// Type-derived POSITIONAL skip for a CONSTRUCTLESS struct/tuple container
/// (call-produced — no `Construct` to inspect): the container type's burden
/// is struct-shaped (no variants; positional field-walk drop glue), and
/// every consume-marked view path names one of its owned-field ordinals, so
/// the skip IS the hazard's own field set (`FD_skipset_sound` — the moved
/// mark is field-unique by position; `FD_per_site_skipset_sound` gates each
/// release site by extraction domination downstream).
fn derive_constructless_positional_skip(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    hazard: &FieldViewHazard,
) -> Option<Vec<u32>> {
    use crate::lower::burden::BurdenRef;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    if !hazard.construct_sites.is_empty() || hazard.nested_path || hazard.skip_fields.is_empty() {
        return None;
    }
    let (var, _) = partition.node_key(hazard.container)?;
    let &ty = func.var_types.get(var.index())?;
    let Some(BurdenRef::User(user)) =
        lookup_burden(idx_to_type_ref(ty, type_registry), type_registry)
    else {
        return None;
    };
    if !user.variant_burdens.is_empty() {
        return None;
    }
    let owned: Vec<u32> = user
        .owned_fields
        .iter()
        .filter_map(|field| field.field_path.first().copied())
        .collect();
    if !hazard.skip_fields.iter().all(|field| owned.contains(field)) {
        return None;
    }
    if !view_projections_all_move_out(func, partition, hazard.view) {
        return None;
    }
    let mut skip = hazard.skip_fields.clone();
    skip.sort_unstable();
    Some(skip)
}

/// Every member-defining Project of the view class MOVES its projection out:
/// the dst — through its function-wide `Let`-alias closure — reaches a
/// transfer position (an owned call arg, a Construct/Reuse arg, a `Set`
/// value, a `PartialApply` capture, a `Return`, or a `Jump` arg, per the
/// committed RL-2 transfer table). A borrow-read projection (a `len()`
/// receiver, a condition read) DECLINES the positional authority —
/// crediting it as an extraction would mint a reference no runtime
/// acquisition matches, and the view's re-booked plan would over-release
/// (the mixed read-and-move shape).
fn view_projections_all_move_out(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    view: NodeIdx,
) -> bool {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::{ArcInstr, ArcTerminator, ArgOwnership};

    let mut projection_dsts: Vec<crate::ir::ArcVarId> = Vec::new();
    for arc_block in &func.blocks {
        for instr in &arc_block.body {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = partition.register_node(*dst, FieldPath::whole_var());
            if partition.rep_of(node) == view {
                projection_dsts.push(*dst);
            }
        }
    }
    projection_dsts.iter().all(|&dst| {
        let closure = super::emit::close_over_let_aliases(func, std::iter::once(dst).collect());
        let transferred_in_body = func.blocks.iter().any(|arc_block| {
            arc_block.body.iter().any(|instr| match instr {
                ArcInstr::Apply {
                    args,
                    arg_ownership,
                    ..
                } => args.iter().enumerate().any(|(position, arg)| {
                    closure.contains(arg)
                        && arg_ownership.get(position) == Some(&ArgOwnership::Owned)
                }),
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. }
                | ArcInstr::PartialApply { args, .. } => {
                    args.iter().any(|arg| closure.contains(arg))
                }
                ArcInstr::Set { value, .. } => closure.contains(value),
                _ => false,
            })
        });
        let transferred_at_terminator =
            func.blocks
                .iter()
                .any(|arc_block| match &arc_block.terminator {
                    ArcTerminator::Return { value } => closure.contains(value),
                    ArcTerminator::Jump { args, .. } => {
                        args.iter().any(|arg| closure.contains(arg))
                    }
                    ArcTerminator::Invoke {
                        args,
                        arg_ownership,
                        ..
                    }
                    | ArcTerminator::InvokeIndirect {
                        args,
                        arg_ownership,
                        ..
                    } => args.iter().enumerate().any(|(position, arg)| {
                        closure.contains(arg)
                            && arg_ownership.get(position) == Some(&ArgOwnership::Owned)
                    }),
                    _ => false,
                });
        transferred_in_body || transferred_at_terminator
    })
}

/// The `DecPartial` skip set for a SUM container — the moved-out variant's
/// ordinal — or `None` for a struct/tuple container. `Err(())` declines:
/// a sum container's skip is discriminant- and arm-conditional, so sums
/// decline (fail-closed) EXCEPT the uniform single-payload-variant shape
/// (every construct site builds the SAME one-payload variant and the view
/// is its sole payload slot; slot 0 is the tag), whose skip names the
/// variant ordinal per the tag-switched enum drop glue. A CONSTRUCTLESS
/// container derives the variant from the type's burden table instead
/// (`derive_constructless_enum_variant`).
fn derive_sum_skip(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    hazard: &FieldViewHazard,
) -> Result<Option<SkipAuthority>, ()> {
    // Niche-family wrappers: the wrapper IS the payload allocation, so its
    // whole-var dec is the payload's release — never decomposable.
    let niche_family = hazard
        .sum_enum_name
        .is_some_and(|name| name == interner.intern("Option") || name == interner.intern("Result"));
    match (hazard.sum_container, hazard.sum_variant) {
        (false, _) => Ok(
            derive_constructless_enum_variant(func, partition, type_registry, hazard)
                .map(SkipAuthority::Variant)
                .or_else(|| {
                    derive_constructless_positional_skip(func, partition, type_registry, hazard)
                        .map(SkipAuthority::Positional)
                }),
        ),
        (true, Some(variant))
            if hazard.skip_fields == [1]
                && !niche_family
                && container_is_user_variant_enum(func, partition, type_registry, hazard) =>
        {
            Ok(Some(SkipAuthority::Variant(vec![variant])))
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
/// Vacuously safe for a construct-bearing struct/tuple container (no skip
/// authority). A POSITIONAL authority (constructless struct/tuple) runs the
/// same per-site classification with NO tag exclusion: every release site
/// must be extraction-dominated for the uniform whole-container skip; a
/// bypass site routes to the per-site path instead.
fn sum_release_sites_safe(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    hazard: &FieldViewHazard,
    classes: &[ClassPlan],
    authority: Option<&SkipAuthority>,
) -> bool {
    let Some(authority) = authority else {
        return true;
    };
    let variant = authority.variant_ordinal();
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
            ?variant,
            "release sites not uniformly skip-safe; trying per-site decomposition"
        );
    }
    safe
}

/// Per-site arm verdict for a variant-ordinal skip (PV-6 per-site
/// refinement, `FD_site_uniform_projection`): SKIP when every path through
/// the release site moved the payload out (extraction-dominated) or no such
/// payload exists there (tag-excluded); WHOLE when the site is untouched by
/// any extraction (not forward-reachable from one — the bypass edge keeps
/// the recursive release); MIXED (None) when paths disagree — decline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SiteVerdict {
    Skip,
    Whole,
}

/// Shared per-site machinery for a variant-ordinal skip: the view's
/// extraction sites, the tag-excluded arm entries, and forward
/// reachability from the extractions (`FD_site_uniform_projection`).
struct SumArmContext {
    dom: crate::graph::DominatorTree,
    extractions: Vec<(usize, usize)>,
    excluded_entries: Vec<usize>,
    reachable_from_extraction: Vec<bool>,
}

impl SumArmContext {
    fn build(
        func: &ArcFunction,
        partition: &mut BirthSitePartition,
        view: NodeIdx,
        container: NodeIdx,
        variant: Option<u32>,
    ) -> Self {
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
        // `Project <member>.0`) whose case value is NOT the skip variant,
        // plus the default arm when the skip variant is an explicit case.
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
        // Tag exclusion applies to VARIANT-ordinal skips only; a positional
        // (tuple/struct field) skip has no discriminant to exclude by.
        let mut excluded_entries: Vec<usize> = Vec::new();
        if let Some(variant) = variant {
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
        }
        // Forward reachability FROM the extractions: a block downstream of
        // any extraction may hold a moved-out payload; a block no extraction
        // reaches never does (the bypass edge — whole-var release safe).
        let mut reachable_from_extraction = vec![false; func.blocks.len()];
        let mut worklist: Vec<usize> = Vec::new();
        for &(block, _) in &extractions {
            for succ in crate::aims::class_ledger::events::successors_of(func, block) {
                if !reachable_from_extraction[succ] {
                    reachable_from_extraction[succ] = true;
                    worklist.push(succ);
                }
            }
        }
        while let Some(block) = worklist.pop() {
            for succ in crate::aims::class_ledger::events::successors_of(func, block) {
                if !reachable_from_extraction[succ] {
                    reachable_from_extraction[succ] = true;
                    worklist.push(succ);
                }
            }
        }
        Self {
            dom,
            extractions,
            excluded_entries,
            reachable_from_extraction,
        }
    }

    /// One release site's verdict; `None` = MIXED (paths disagree).
    fn classify(&self, func: &ArcFunction, op: &emit::PlannedOp) -> Option<SiteVerdict> {
        if !matches!(
            op.kind,
            emit::PlannedOpKind::Dec | emit::PlannedOpKind::DecPartial { .. }
        ) {
            return Some(SiteVerdict::Whole);
        }
        let block = op.slot.block();
        let block_id = func.blocks[block].id;
        let same_block_extraction = |pb: usize, pi: usize| {
            pb == block
                && match op.slot {
                    emit::PlanSlot::AfterBody { index, .. } => index >= pi,
                    emit::PlanSlot::BeforeBody { index, .. } => index > pi,
                    emit::PlanSlot::BeforeTerminator { .. } => true,
                    emit::PlanSlot::BlockFront { .. } => false,
                }
        };
        let extraction_dominates = self.extractions.iter().any(|&(pb, pi)| {
            if pb == block {
                same_block_extraction(pb, pi)
            } else {
                self.dom.dominates(func.blocks[pb].id, block_id)
            }
        });
        let tag_excluded = self
            .excluded_entries
            .iter()
            .any(|&entry| self.dom.dominates(func.blocks[entry].id, block_id));
        if extraction_dominates || tag_excluded {
            return Some(SiteVerdict::Skip);
        }
        // Untouched by every extraction: neither reachable from one nor
        // holding one earlier in the same block — the payload is still in
        // the container on every path here; the whole-var release is its
        // recursive drop.
        let touched = self.reachable_from_extraction[block]
            || self
                .extractions
                .iter()
                .any(|&(pb, pi)| same_block_extraction(pb, pi));
        if !touched {
            return Some(SiteVerdict::Whole);
        }
        None
    }
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
    variant: Option<u32>,
    container_ops: &[emit::PlannedOp],
) -> bool {
    let ctx = SumArmContext::build(func, partition, view, container, variant);
    container_ops.iter().all(|op| {
        ctx.classify(func, op) == Some(SiteVerdict::Skip)
            || !matches!(
                op.kind,
                emit::PlannedOpKind::Dec | emit::PlannedOpKind::DecPartial { .. }
            )
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

/// The per-SITE decomposition attempt (`FD_site_uniform_projection`):
/// classify every container release site (Skip = extraction-dominated or
/// tag-excluded; Whole = untouched by every extraction; None = MIXED —
/// decline), book the view with the kept store consume plus a CREDIT at
/// each extraction, and re-plan + verify. Returns the per-op verdicts on
/// success (the caller applies the skip conversion per verdict).
#[expect(
    clippy::too_many_arguments,
    reason = "internal cure pass over analyze_class_ledger's own accumulators"
)]
fn try_per_site_decomposition(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    preds: &[Vec<usize>],
    regions: &emit::CycleRegions,
    hazard: &FieldViewHazard,
    classes: &mut [ClassPlan],
    verdicts: &mut [(NodeIdx, ClassVerdict)],
    declined: &mut Vec<(NodeIdx, emit::DeclineReason)>,
    authority: Option<&SkipAuthority>,
) -> Option<Vec<SiteVerdict>> {
    let container = hazard.container;
    let variant = authority?.variant_ordinal();
    let ctx = SumArmContext::build(func, partition, hazard.view, container, variant);
    let container_entry = classes
        .iter()
        .find(|plan| partition.rep_of(plan.class) == container)?;
    let ClassOutcome::Planned(container_ops) = &container_entry.outcome else {
        return None;
    };
    let mut verdicts_per_op = Vec::with_capacity(container_ops.len());
    for op in container_ops {
        let Some(verdict) = ctx.classify(func, op) else {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?partition.node_key(hazard.view),
                op = ?op,
                "field-decomposition cure declined: mixed release site                      (reachable both with and without extraction)"
            );
            return None;
        };
        verdicts_per_op.push(verdict);
    }
    let extractions = ctx.extractions.clone();
    drop(ctx);
    let credited = events::extract_class_events_with_extraction_credits(
        func,
        classification,
        partition,
        hazard.view,
        &extractions,
        false,
    );
    let outcome_opt = plan_and_verify_cure(
        func,
        preds,
        regions,
        partition,
        hazard.view,
        "field-decomposition-per-site",
        &credited,
        &[],
    );
    let outcome = outcome_opt?;
    if !commit_cured_view(classes, verdicts, declined, hazard.view, outcome) {
        return None;
    }
    Some(verdicts_per_op)
}

/// Convert the container's planned releases to the skip form: every op
/// (or, per-site, every `Skip`-verdict op) becomes / widens a `DecPartial`.
/// Struct/tuple skips name top-level field indices; the admitted sum
/// shape's skip names the moved-out VARIANT ordinal instead.
fn apply_container_skip_conversion(
    partition: &mut BirthSitePartition,
    classes: &mut [ClassPlan],
    container: NodeIdx,
    dec_skip: &[u32],
    per_site_verdicts: Option<&[SiteVerdict]>,
) -> bool {
    let Some(container_entry) = classes
        .iter_mut()
        .find(|plan| partition.rep_of(plan.class) == container)
    else {
        return false;
    };
    let ClassOutcome::Planned(container_ops) = &mut container_entry.outcome else {
        return false;
    };
    for (op_index, op) in container_ops.iter_mut().enumerate() {
        if let Some(verdicts_per_op) = per_site_verdicts {
            if verdicts_per_op.get(op_index) != Some(&SiteVerdict::Skip) {
                continue;
            }
        }
        match &mut op.kind {
            emit::PlannedOpKind::Dec => {
                op.kind = emit::PlannedOpKind::DecPartial {
                    skip_fields: dec_skip.to_vec(),
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
    true
}

/// Cure one consume-marked endangered view by decomposing the container's
/// release per named owned field: the container's planned `Dec`s become
/// `DecPartial(skip = the view's field indices)` and the view's events are
/// RE-BOOKED with the move-in store non-consuming (ownership never enters
/// the container's release path), then re-planned + re-verified. The skip
/// set derives solely from the partition's consume marks — the UNIQUE
/// clause-preserving skip set per `FD_skipset_sound`
/// (`AimsProof.FieldDecomposition`; Spec: Annex E §AIMS §12). A merely-read
/// view is never consume-marked, never skipped (over-skip = leak).
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
    let Ok(authority) = derive_sum_skip(func, partition, type_registry, interner, hazard) else {
        return false;
    };
    if !hazard.consume_marked
        || hazard.nested_path
        // Constructless is admitted ONLY with a type-derived variant skip
        // (`derive_constructless_enum_variant`); a constructless struct
        // container has no skip authority and declines.
        || (hazard.construct_sites.is_empty() && authority.is_none())
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
    let container = hazard.container;
    // POSITIONAL authority is acyclic-only: the per-site verdicts rest on
    // dominator reasoning that a CFG cycle breaks (an in-loop extraction
    // "dominates" a later in-loop release, yet on the next iteration the
    // payload is a NEW reference the skip would strand — the loop-carried
    // struct-rebuild double-free). A VARIANT authority is unaffected (the
    // admitted sum shapes are post-switch acyclic arms).
    if matches!(authority, Some(SkipAuthority::Positional(_)))
        && func_has_cycle_touching_view_or_container(func, partition, regions, hazard, classes)
    {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?partition.node_key(hazard.view),
            "field-decomposition cure declined: positional skip inside a CFG cycle"
        );
        return false;
    }
    let all_sites_safe =
        sum_release_sites_safe(func, partition, hazard, classes, authority.as_ref());
    // Per-SITE fallback (sum shapes with a variant skip only): a bypass-edge
    // release keeps the whole-var Dec (the recursive drop of the unmoved
    // payload) while extraction-dominated sites take the variant skip; the
    // view books the kept store consume + a CREDIT at each extraction
    // (`FD_site_uniform_projection`; a MIXED site declines).
    let per_site_verdicts: Option<Vec<SiteVerdict>> = if all_sites_safe {
        None
    } else {
        match try_per_site_decomposition(
            func,
            classification,
            partition,
            preds,
            regions,
            hazard,
            classes,
            verdicts,
            declined,
            authority.as_ref(),
        ) {
            Some(verdicts_per_op) => Some(verdicts_per_op),
            None => return false,
        }
    };

    if per_site_verdicts.is_none() {
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
        if !commit_cured_view(classes, verdicts, declined, hazard.view, outcome) {
            return false;
        }
    }
    if !apply_container_skip_conversion(
        partition,
        classes,
        container,
        authority
            .as_ref()
            .map_or(&hazard.skip_fields[..], SkipAuthority::skip_fields),
        per_site_verdicts.as_deref(),
    ) {
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
