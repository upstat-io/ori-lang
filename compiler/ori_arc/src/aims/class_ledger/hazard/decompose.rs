//! Field-decomposition cure (PV-6): decompose a released container's
//! whole-var releases per named owned field — uniformly, or per release
//! site when a bypass edge keeps the recursive whole-var drop.
//! Trace events share the `ori_arc::aims::class_ledger` target across the cure ladder.

use crate::aims::intraprocedural::birth_site_partition::NodeIdx;

use super::super::emit::{self, ClassOutcome};
use super::super::events;
use super::skip_derive::{derive_sum_skip, SkipAuthority};
use super::sum_arm::{sum_release_sites_safe, SiteVerdict, SumArmContext};
use super::{
    commit_cured_view, plan_and_verify_cure, FieldViewHazard, HazardCureInputs, HazardCureState,
};

/// Whether any view-member Project or container planned-release block sits
/// in a CFG cycle — the acyclicity gate for POSITIONAL skip authority.
fn func_has_cycle_touching_view_or_container(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    hazard: &FieldViewHazard,
) -> bool {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::ArcInstr;

    for (block_idx, arc_block) in inputs.func.blocks.iter().enumerate() {
        for instr in &arc_block.body {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = state.partition.register_node(*dst, FieldPath::whole_var());
            if state.partition.rep_of(node) == hazard.view && inputs.regions.is_in_cycle(block_idx)
            {
                return true;
            }
        }
    }
    let Some(container_entry) = state
        .classes
        .iter()
        .find(|plan| state.partition.rep_of(plan.class) == hazard.container)
    else {
        return true;
    };
    let ClassOutcome::Planned(container_ops) = &container_entry.outcome else {
        return true;
    };
    container_ops
        .iter()
        .any(|op| inputs.regions.is_in_cycle(op.slot.block()))
}

/// Extraction sites that first expose a constructless container field on
/// each path. The call-result boundary contributes one field credit along a
/// path; a later dominated extraction is a duplicate and must retain its
/// ordinary increment.
fn constructless_boundary_credit_sites(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    hazard: &FieldViewHazard,
    authority: &SkipAuthority,
) -> Vec<(usize, usize)> {
    let ctx = SumArmContext::for_container_release(
        inputs.func,
        state.partition,
        hazard.view,
        hazard.container,
        authority.variant_ordinal(),
    );
    let dom = crate::graph::DominatorTree::build(inputs.func);
    let mut first_extraction: Vec<Option<usize>> = vec![None; inputs.func.blocks.len()];
    for &(block, index) in &ctx.extractions {
        let first = &mut first_extraction[block];
        *first = Some(first.map_or(index, |prior| prior.min(index)));
    }
    let mut dominated = vec![None; inputs.func.blocks.len()];
    ctx.extractions
        .iter()
        .copied()
        .filter(|&(block, index)| {
            first_extraction[block] == Some(index)
                && !has_extraction_ancestor(block, &dom.idom, &first_extraction, &mut dominated)
        })
        .collect()
}

fn has_extraction_ancestor(
    block: usize,
    immediate_dominators: &[Option<usize>],
    first_extraction: &[Option<usize>],
    memo: &mut [Option<bool>],
) -> bool {
    if let Some(result) = memo[block] {
        return result;
    }
    let result = match immediate_dominators[block] {
        Some(parent) if parent != block => {
            first_extraction[parent].is_some()
                || has_extraction_ancestor(parent, immediate_dominators, first_extraction, memo)
        }
        _ => false,
    };
    memo[block] = Some(result);
    result
}

/// The per-SITE decomposition attempt (`FD_site_uniform_projection`):
/// classify every container release site (Skip = extraction-dominated or
/// tag-excluded; Whole = untouched by every extraction; None = MIXED —
/// decline), book the view with the kept store consume plus a CREDIT at
/// each extraction, and re-plan + verify. Returns the per-op verdicts on
/// success (the caller applies the skip conversion per verdict).
fn try_per_site_decomposition(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    hazard: &FieldViewHazard,
    authority: Option<&SkipAuthority>,
) -> Option<Vec<SiteVerdict>> {
    let container = hazard.container;
    let variant = authority?.variant_ordinal();
    let ctx = SumArmContext::for_container_release(
        inputs.func,
        state.partition,
        hazard.view,
        container,
        variant,
    );
    let container_entry = state
        .classes
        .iter()
        .find(|plan| state.partition.rep_of(plan.class) == container)?;
    let ClassOutcome::Planned(container_ops) = &container_entry.outcome else {
        return None;
    };
    let mut verdicts_per_op = Vec::with_capacity(container_ops.len());
    for op in container_ops {
        let Some(verdict) = ctx.classify(inputs.func, op) else {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?state.partition.node_key(hazard.view),
                op = ?op,
                "field-decomposition cure declined: mixed release site (reachable both with and without extraction)"
            );
            return None;
        };
        verdicts_per_op.push(verdict);
    }
    let extractions = ctx.extractions.clone();
    drop(ctx);
    let credited = events::extract_class_events_with_extraction_credits(
        inputs.func,
        inputs.classification,
        state.partition,
        hazard.view,
        &extractions,
        events::EventFunding::ExtractionOwned,
    );
    let outcome_opt = plan_and_verify_cure(
        inputs,
        state,
        hazard.view,
        "field-decomposition-per-site",
        &credited,
        &[],
    );
    let outcome = outcome_opt?;
    if !commit_cured_view(state, hazard.view, outcome) {
        return None;
    }
    Some(verdicts_per_op)
}

/// Convert the container's planned releases to the skip form: every op
/// (or, per-site, every `Skip`-verdict op) becomes / widens a `DecPartial`.
/// Struct/tuple skips name top-level field indices; the admitted sum
/// shape's skip names the moved-out VARIANT ordinal instead.
fn apply_container_skip_conversion(
    state: &mut HazardCureState<'_>,
    container: NodeIdx,
    dec_skip: &[u32],
    per_site_verdicts: Option<&[SiteVerdict]>,
) -> bool {
    let Some(container_entry) = state
        .classes
        .iter_mut()
        .find(|plan| state.partition.rep_of(plan.class) == container)
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
                skip_fields.extend_from_slice(dec_skip);
                skip_fields.sort_unstable();
                skip_fields.dedup();
            }
            emit::PlannedOpKind::Inc => {}
        }
    }
    true
}

/// Gate: is `hazard` skip-derivable at all? Declines (with a diagnostic
/// trace naming which precondition failed) any view that is not
/// consume-marked, sits on a nested path, has its container transferred out,
/// is constructless with no type-derived variant authority, or already
/// carries empty `skip_fields`.
fn is_skip_derivable(
    state: &HazardCureState<'_>,
    hazard: &FieldViewHazard,
    authority: Option<&SkipAuthority>,
) -> bool {
    let derivable = hazard.is_consume_marked()
        && !hazard.is_nested_path()
        // A transferred container still owns its field, so extraction needs a
        // separate credit rather than re-booking the original move-in.
        && !hazard.is_container_transferred_out()
        // Only type-derived variant authority admits constructless containers.
        && (!hazard.construct_sites.is_empty() || authority.is_some())
        && !hazard.skip_fields.is_empty();
    if !derivable {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?state.partition.node_key(hazard.view),
            consume_marked = hazard.is_consume_marked(),
            nested_path = hazard.is_nested_path(),
            container_transferred_out = hazard.is_container_transferred_out(),
            sum_container = hazard.is_sum_container(),
            constructless = hazard.construct_sites.is_empty(),
            skip_fields = ?hazard.skip_fields,
            "field-decomposition cure declined: view not skip-derivable"
        );
    }
    derivable
}

/// When per-site decomposition did not run (`all_sites_safe`), rebook or
/// extraction-credit the view and commit the re-planned cure. Returns
/// `false` (decline the cure) on any step's failure.
fn rebook_or_credit_and_commit(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    hazard: &FieldViewHazard,
    authority: Option<&SkipAuthority>,
) -> bool {
    let rebooked = if hazard.construct_sites.is_empty() {
        let Some(authority) = authority else {
            return false;
        };
        let credit_sites = constructless_boundary_credit_sites(inputs, state, hazard, authority);
        if credit_sites.is_empty() {
            return false;
        }
        let funding = if credit_sites.iter().any(|&(block, index)| {
            let extraction = inputs
                .func
                .blocks
                .get(block)
                .and_then(|arc_block| arc_block.body.get(index))
                .and_then(|instr| match instr {
                    crate::ir::ArcInstr::Project { dst, .. } => Some(*dst),
                    _ => None,
                });
            extraction.is_none_or(|var| {
                !super::increment_is_materializable(inputs.func, var, inputs.type_registry)
            })
        }) {
            events::EventFunding::ExtractionOwned
        } else {
            events::EventFunding::Classified
        };
        events::extract_class_events_with_extraction_credits(
            inputs.func,
            inputs.classification,
            state.partition,
            hazard.view,
            &credit_sites,
            funding,
        )
    } else {
        events::extract_class_events_rebooked(
            inputs.func,
            inputs.classification,
            state.partition,
            hazard.view,
            &hazard.construct_sites,
        )
    };
    let Some(outcome) = plan_and_verify_cure(
        inputs,
        state,
        hazard.view,
        "field-decomposition",
        &rebooked,
        &[],
    ) else {
        return false;
    };
    commit_cured_view(state, hazard.view, outcome)
}

/// Decomposes a consume-marked view's container releases into `DecPartial`
/// operations over the exact moved fields. Construct-bearing views re-book
/// their move-in store as non-consuming; constructless call results receive
/// the container credit at their first extraction. Both transfer existing
/// ownership without an increment, then re-plan and re-verify. Skip fields
/// come only from consume marks per `FD_skipset_sound`; read-only views are
/// never skipped (`AimsProof.FieldDecomposition`; Annex E §AIMS §12).
pub(super) fn cure_view_with_field_decomposition(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    hazard: &FieldViewHazard,
) -> bool {
    let Ok(authority) = derive_sum_skip(
        inputs.func,
        state.partition,
        inputs.type_registry,
        inputs.interner,
        hazard,
    ) else {
        return false;
    };

    if !is_skip_derivable(state, hazard, authority.as_ref()) {
        return false;
    }
    let container = hazard.container;
    // Positional authority is acyclic: a later loop iteration may carry a new
    // payload despite an apparent extraction-to-release dominance relation.
    if matches!(authority, Some(SkipAuthority::Positional(_)))
        && func_has_cycle_touching_view_or_container(inputs, state, hazard)
    {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            view = ?state.partition.node_key(hazard.view),
            "field-decomposition cure declined: positional skip inside a CFG cycle"
        );
        return false;
    }
    let all_sites_safe = sum_release_sites_safe(
        inputs.func,
        state.partition,
        hazard,
        state.classes,
        authority.as_ref(),
    );
    // Variant skips may fall back per site: bypass releases keep the whole-var
    // decrement while extraction-dominated releases skip the moved payload.
    let per_site_verdicts: Option<Vec<SiteVerdict>> = if all_sites_safe {
        None
    } else {
        match try_per_site_decomposition(inputs, state, hazard, authority.as_ref()) {
            Some(verdicts_per_op) => Some(verdicts_per_op),
            None => return false,
        }
    };

    if per_site_verdicts.is_none()
        && !rebook_or_credit_and_commit(inputs, state, hazard, authority.as_ref())
    {
        return false;
    }
    if !apply_container_skip_conversion(
        state,
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
        view = ?state.partition.node_key(hazard.view),
        container = ?state.partition.node_key(container),
        skip_fields = ?hazard.skip_fields,
        "field-decomposition cure applied: container releases skip consume-marked fields"
    );
    true
}
