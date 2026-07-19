//! Per-release-site arm safety for a derived skip authority
//! (`FD_site_uniform_projection`): extraction domination, tag exclusion,
//! and forward reachability from the view's extraction sites.

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::ir::ArcFunction;

use super::super::emit::{self, ClassOutcome};
use super::super::ClassPlan;
use super::skip_derive::SkipAuthority;
use super::FieldViewHazard;

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
pub(super) fn sum_release_sites_safe(
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
pub(super) enum SiteVerdict {
    Skip,
    Whole,
}

/// Shared per-site machinery for a variant-ordinal skip: the view's
/// extraction sites, the tag-excluded arm entries, and forward
/// reachability from the extractions (`FD_site_uniform_projection`).
pub(super) struct SumArmContext {
    dom: crate::graph::DominatorTree,
    pub(super) extractions: Vec<(usize, usize)>,
    excluded_entries: Vec<usize>,
    reachable_from_extraction: Vec<bool>,
}

impl SumArmContext {
    pub(super) fn build(
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
    pub(super) fn classify(&self, func: &ArcFunction, op: &emit::PlannedOp) -> Option<SiteVerdict> {
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
pub(super) fn sum_skip_sites_arm_safe(
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
