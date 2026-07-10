//! Funding (`BurdenInc`) planning: RL-1 duplication incs for consumes the
//! class survives, and RL-1 realization incs for `Select` acquisitions.
//! Spec: Annex E §AIMS RL-1 + RL-34.

use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::ir::ArcFunction;

use super::super::events::{
    demand_blocks_of_vars, live_from_forward_killing, live_out_forward_killing, live_out_killing,
    ClassEvent, ClassEvents, EventKind,
};
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
    credit_kills: &[bool],
    seed_vars: &rustc_hash::FxHashSet<crate::ir::ArcVarId>,
    full_closure: bool,
    dom: &crate::graph::DominatorTree,
) -> Result<Vec<PlannedOp>, DeclineReason> {
    let borrowed = events.is_externally_funded();
    let mut ops = Vec::new();
    for (block, evs) in events.per_block.iter().enumerate() {
        for (position, ev) in evs.iter().enumerate() {
            if ev.kind != EventKind::Consume {
                continue;
            }
            // A consume OF a seeded member prices against SAME-REFERENCE
            // demand only (the seed var's own alias closure), FORWARD-only:
            // the seed funds exactly one reference per extraction, so a
            // hand-off keeps duplication funding only when THAT reference
            // is read past the consume — another seeded extraction (a later
            // iteration's) is a different reference, and a back-edge suffix
            // is the next iteration's ledger. Every other consume prices
            // against the seed-filtered class surface.
            let consume_of_seeded = ev.var.is_some_and(|v| seed_vars.contains(&v));
            // Entry-credit successors are KILLED for the funding decision:
            // their demand (at/after the credit re-acquisition) is funded
            // by the credit, never by a pre-consume duplication inc here.
            let (demand_out, suffix_demand) = if consume_of_seeded {
                let var = ev
                    .var
                    .unwrap_or_else(|| unreachable!("consume_of_seeded checked is_some"));
                let closure = super::close_over_let_aliases(func, std::iter::once(var).collect());
                let blocks = demand_blocks_of_vars(events, &closure);
                let live = live_from_forward_killing(func, &blocks, credit_kills, dom);
                (
                    live_out_forward_killing(func, block, &live, credit_kills, dom),
                    suffix_has_demand_of_vars(evs, position, &closure),
                )
            } else {
                let demand_out = if full_closure {
                    live_out_killing(func, block, demand_live, credit_kills)
                } else {
                    live_out_forward_killing(func, block, demand_live, credit_kills, dom)
                };
                (demand_out, suffix_has_demand(evs, position, seed_vars))
            };
            if !borrowed
                && (same_site_credit_follows(evs, position)
                    || invoke_refund_credit_follows(func, events, block, ev)
                    || !(suffix_demand || demand_out))
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

/// Whether a later same-reference value use exists in the block: demand on
/// the given vars only (a seeded member's own alias closure).
fn suffix_has_demand_of_vars(
    evs: &[ClassEvent],
    position: usize,
    vars: &rustc_hash::FxHashSet<crate::ir::ArcVarId>,
) -> bool {
    evs[position + 1..].iter().any(|ev| {
        matches!(
            ev.kind,
            EventKind::Read | EventKind::Mutate | EventKind::Consume
        ) && ev.var.is_some_and(|v| vars.contains(&v))
    })
}

/// Whether a later value use (Read / Mutate / Consume) exists in the block.
/// Seed-funded member vars are excluded: their demand is paid by the
/// extraction-site inc, never by a pre-consume duplication inc.
fn suffix_has_demand(
    evs: &[ClassEvent],
    position: usize,
    seed_vars: &rustc_hash::FxHashSet<crate::ir::ArcVarId>,
) -> bool {
    evs[position + 1..].iter().any(|ev| {
        matches!(
            ev.kind,
            EventKind::Read | EventKind::Mutate | EventKind::Consume
        ) && !ev.var.is_some_and(|v| seed_vars.contains(&v))
    })
}
