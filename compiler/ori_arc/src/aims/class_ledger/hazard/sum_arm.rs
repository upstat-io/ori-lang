//! Per-release-site safety for local and boundary-derived field-transfer
//! authorities (`FD_site_uniform_projection` and
//! `FD_authority_union_skipset_sound`): transfer state, tag exclusion, and
//! the exact position of each planned release.

use std::collections::VecDeque;

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::ir::ArcFunction;

use super::super::emit::{self, ClassOutcome};
use super::super::ClassPlan;
use super::skip_derive::SkipAuthority;
use super::FieldViewHazard;

/// Validates each planned container release against its field-transfer
/// authority. A release must be extraction-dominated or lie on a tag arm
/// excluding the skipped variant; otherwise the skipped payload leaks.
/// Construct-bearing structs and tuples are vacuously safe. Constructless
/// positional authority has no tag exclusion, so every uniform skip must be
/// extraction-dominated; a bypass routes to the per-site path.
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
pub(in crate::aims::class_ledger) enum SiteVerdict {
    Skip,
    Whole,
}

/// CFG-only transfer-state classifier shared by local extraction and
/// boundary-contract adapters. Each block entry tracks whether it is
/// reachable before and/or after the transfer; the two-bit fixed point makes
/// joins and backedges explicit instead of inferring them from dominance.
pub(in crate::aims::class_ledger) struct TransferFlowContext {
    transfer_sites: Vec<Vec<EventSite>>,
    entry_without_transfer: Vec<bool>,
    entry_with_transfer: Vec<bool>,
}

impl TransferFlowContext {
    #[must_use]
    pub(in crate::aims::class_ledger) fn from_transfer_sites(
        func: &ArcFunction,
        transfer_sites: &[(usize, EventSite)],
    ) -> Self {
        let mut sites_by_block = vec![Vec::new(); func.blocks.len()];
        for &(block, site) in transfer_sites {
            if let Some(sites) = sites_by_block.get_mut(block) {
                sites.push(site);
            }
        }
        let mut entry_without_transfer = vec![false; func.blocks.len()];
        let mut entry_with_transfer = vec![false; func.blocks.len()];
        let entry = func.entry.index();
        if entry < func.blocks.len() {
            entry_without_transfer[entry] = true;
        }
        let mut worklist = VecDeque::from([entry]);
        while let Some(block) = worklist.pop_front() {
            if block >= func.blocks.len() {
                continue;
            }
            let block_transfers = !sites_by_block[block].is_empty();
            let exit_without = entry_without_transfer[block] && !block_transfers;
            let exit_with =
                entry_with_transfer[block] || (entry_without_transfer[block] && block_transfers);
            for successor in crate::aims::class_ledger::events::successors_of(func, block) {
                let changed_without = exit_without && !entry_without_transfer[successor];
                let changed_with = exit_with && !entry_with_transfer[successor];
                if changed_without {
                    entry_without_transfer[successor] = true;
                }
                if changed_with {
                    entry_with_transfer[successor] = true;
                }
                if changed_without || changed_with {
                    worklist.push_back(successor);
                }
            }
        }
        Self {
            transfer_sites: sites_by_block,
            entry_without_transfer,
            entry_with_transfer,
        }
    }

    /// Classify one planned release. `None` means concrete paths reach the
    /// site both before and after transfer, so neither release shape is safe.
    pub(in crate::aims::class_ledger) fn classify(
        &self,
        op: &emit::PlannedOp,
    ) -> Option<SiteVerdict> {
        if !matches!(
            op.kind,
            emit::PlannedOpKind::Dec | emit::PlannedOpKind::DecPartial { .. }
        ) {
            return Some(SiteVerdict::Whole);
        }
        let block = op.slot.block();
        let mut without = *self.entry_without_transfer.get(block)?;
        let mut with = *self.entry_with_transfer.get(block)?;
        if self.transfer_sites[block]
            .iter()
            .copied()
            .any(|site| transfer_precedes_release(site, op.slot))
        {
            with |= without;
            without = false;
        }
        match (without, with) {
            (true, false) => Some(SiteVerdict::Whole),
            (false, true) => Some(SiteVerdict::Skip),
            (true, true) | (false, false) => None,
        }
    }
}

fn transfer_precedes_release(site: EventSite, slot: emit::PlanSlot) -> bool {
    match (site, slot) {
        (EventSite::BlockEntry, _)
        | (EventSite::Body(_), emit::PlanSlot::BeforeTerminator { .. }) => true,
        (EventSite::Body(transfer), emit::PlanSlot::BeforeBody { index, .. }) => transfer < index,
        (EventSite::Body(transfer), emit::PlanSlot::AfterBody { index, .. }) => transfer <= index,
        (EventSite::Body(_), emit::PlanSlot::BlockFront { .. }) | (EventSite::Terminator, _) => {
            false
        }
    }
}

/// Shared per-site machinery for a variant-ordinal skip: the view's
/// extraction sites, the tag-excluded arm entries, and forward
/// reachability from the extractions (`FD_site_uniform_projection`).
pub(super) struct SumArmContext {
    flow: TransferFlowContext,
    dom: crate::graph::DominatorTree,
    pub(super) extractions: Vec<(usize, usize)>,
    excluded_entries: Vec<usize>,
}

impl SumArmContext {
    pub(super) fn for_container_release(
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
        let transfer_sites: Vec<_> = extractions
            .iter()
            .map(|&(block, index)| (block, EventSite::Body(index)))
            .collect();
        Self {
            flow: TransferFlowContext::from_transfer_sites(func, &transfer_sites),
            dom,
            extractions,
            excluded_entries,
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
        let tag_excluded = self
            .excluded_entries
            .iter()
            .any(|&entry| self.dom.dominates(func.blocks[entry].id, block_id));
        if tag_excluded {
            return Some(SiteVerdict::Skip);
        }
        self.flow.classify(op)
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
    let ctx = SumArmContext::for_container_release(func, partition, view, container, variant);
    container_ops.iter().all(|op| {
        ctx.classify(func, op) == Some(SiteVerdict::Skip)
            || !matches!(
                op.kind,
                emit::PlannedOpKind::Dec | emit::PlannedOpKind::DecPartial { .. }
            )
    })
}
