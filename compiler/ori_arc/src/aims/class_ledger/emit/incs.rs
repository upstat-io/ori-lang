//! Funding (`BurdenInc`) planning: RL-1 duplication incs for consumes the
//! class survives, and RL-1 realization incs for `Select` acquisitions.
//! Spec: Annex E §AIMS RL-1 + RL-34.

use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::ir::ArcFunction;

use super::super::events::{live_out, live_out_forward, ClassEvent, ClassEvents, EventKind};
use super::{DeclineReason, PlanSlot, PlannedOp, PlannedOpKind};

/// `BurdenInc` before every CONSUME that duplicates: the class stays live
/// past the hand-off (a later Read / Mutate / Consume in the stream or a
/// successor), or the class is borrowed-rooted. A consume refunded by a
/// same-site CREDIT (the passthrough return leg) transfers the existing
/// reference and needs no inc on an owned-rooted class.
pub(super) fn plan_incs(
    func: &ArcFunction,
    events: &ClassEvents,
    demand_live: &[bool],
    dom: &crate::graph::DominatorTree,
) -> Result<Vec<PlannedOp>, DeclineReason> {
    let borrowed = events.is_externally_funded();
    let mut ops = Vec::new();
    for (block, evs) in events.per_block.iter().enumerate() {
        for (position, ev) in evs.iter().enumerate() {
            if ev.kind != EventKind::Consume {
                continue;
            }
            let demand_out = if events.threads_back_edge {
                live_out(func, block, demand_live)
            } else {
                live_out_forward(func, block, demand_live, dom)
            };
            if !borrowed
                && (same_site_credit_follows(evs, position)
                    || invoke_refund_credit_follows(func, events, block, ev)
                    || !(suffix_has_demand(evs, position) || demand_out))
            {
                continue;
            }
            let Some(var) = ev.var else {
                return Err(DeclineReason::UnresolvedOpVar);
            };
            let slot = match ev.site {
                EventSite::Body(index) => PlanSlot::BeforeBody { block, index },
                EventSite::Terminator => PlanSlot::BeforeTerminator { block },
                EventSite::BlockEntry => return Err(DeclineReason::UnplaceableInc),
            };
            ops.push(PlannedOp {
                slot,
                kind: PlannedOpKind::Inc,
                var,
            });
        }
    }
    Ok(ops)
}

/// Realize every `Select` acquisition: the dst conditionally holds ONE
/// operand's allocation, and the acquired reference is manufactured by an
/// RL-1 duplication inc on the dst immediately after the select (the
/// runtime inc lands on whichever allocation was selected; each operand
/// class stays balanced by its own birth + release).
pub(super) fn plan_select_credit_incs(
    events: &ClassEvents,
) -> Result<Vec<PlannedOp>, DeclineReason> {
    let mut ops = Vec::new();
    for (block, evs) in events.per_block.iter().enumerate() {
        for ev in evs {
            if ev.kind != EventKind::SelectCredit {
                continue;
            }
            let EventSite::Body(index) = ev.site else {
                return Err(DeclineReason::UnplaceableInc);
            };
            let Some(var) = ev.var else {
                return Err(DeclineReason::UnresolvedOpVar);
            };
            ops.push(PlannedOp {
                slot: PlanSlot::AfterBody { block, index },
                kind: PlannedOpKind::Inc,
                var,
            });
        }
    }
    Ok(ops)
}

/// Whether a Terminator-site consume is refunded at the SAME call boundary
/// across an `Invoke`'s normal edge: the result credit routes to the NORMAL
/// successor's block entry, so the RL-34 transfer-with-refund pair spans
/// the edge instead of sharing a site.
fn invoke_refund_credit_follows(
    func: &ArcFunction,
    events: &ClassEvents,
    block: usize,
    consume: &ClassEvent,
) -> bool {
    if consume.site != EventSite::Terminator {
        return false;
    }
    let Some(arc_block) = func.blocks.get(block) else {
        return false;
    };
    let normal = match &arc_block.terminator {
        crate::ir::ArcTerminator::Invoke { normal, .. }
        | crate::ir::ArcTerminator::InvokeIndirect { normal, .. } => normal.index(),
        _ => return false,
    };
    events.per_block.get(normal).is_some_and(|evs| {
        evs.iter()
            .any(|ev| ev.kind == EventKind::Credit && ev.site == EventSite::BlockEntry)
    })
}

/// Whether a same-site CREDIT follows `position` (the transfer-with-refund
/// pair a passthrough call classifies to).
fn same_site_credit_follows(evs: &[ClassEvent], position: usize) -> bool {
    let Some(current) = evs.get(position) else {
        return false;
    };
    evs[position + 1..]
        .iter()
        .any(|ev| ev.kind == EventKind::Credit && ev.site == current.site)
}

/// Whether a later value use (Read / Mutate / Consume) exists in the block.
fn suffix_has_demand(evs: &[ClassEvent], position: usize) -> bool {
    evs[position + 1..].iter().any(|ev| {
        matches!(
            ev.kind,
            EventKind::Read | EventKind::Mutate | EventKind::Consume
        )
    })
}
