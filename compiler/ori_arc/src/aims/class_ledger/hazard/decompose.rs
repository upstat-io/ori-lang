//! Field-decomposition cure (PV-6): decompose a released container's
//! whole-var releases per named owned field — uniformly, or per release
//! site when a bypass edge keeps the recursive whole-var drop.

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::ledger_events::LedgerClassification;
use crate::ir::ArcFunction;

use super::super::emit::{self, ClassOutcome};
use super::super::events;
use super::super::verify::ClassVerdict;
use super::super::ClassPlan;
use super::skip_derive::{derive_sum_skip, SkipAuthority};
use super::sum_arm::{sum_release_sites_safe, SiteVerdict, SumArmContext};
use super::{commit_cured_view, plan_and_verify_cure, FieldViewHazard};

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
pub(super) fn cure_view_with_field_decomposition(
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
