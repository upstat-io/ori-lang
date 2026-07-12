//! Field-view hazard DETECTION: the endangered (view, container) pairs the
//! cure ladder consumes, from per-class facts and the container's own
//! `Construct` surface.

use ori_ir::Name;

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::ir::ArcFunction;

use super::{ClassHazardFacts, FieldViewHazard};

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
/// valid (per `AimsProof.Realization::RLDROP_user_drop_balance_neutral`).
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
