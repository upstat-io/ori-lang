//! Demand-block projections over extracted class events.

use rustc_hash::FxHashSet;

use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::ir::ArcVarId;

use super::{ClassEvents, EventKind};

/// Demand blocks EXCLUDING seed-funded member reads: an extraction-funded
/// (seeded) member var's demand is paid by its own RL-1 inc at the `Project`
/// site, so it never counts as surviving demand on the pre-consume reference.
pub(crate) fn demand_blocks_excluding_seeded(
    events: &ClassEvents,
    seed_vars: &FxHashSet<ArcVarId>,
) -> Vec<bool> {
    events
        .per_block
        .iter()
        .map(|evs| {
            evs.iter().any(|ev| {
                matches!(
                    ev.kind,
                    EventKind::Read | EventKind::Mutate | EventKind::Consume
                ) && !ev.var.is_some_and(|v| seed_vars.contains(&v))
            })
        })
        .collect()
}

/// Demand blocks restricted to the given vars (a seeded member's own alias
/// closure): only same-reference demand — a different seeded extraction is a
/// different iteration's reference and never keeps THIS one alive.
pub(crate) fn demand_blocks_of_vars(events: &ClassEvents, vars: &FxHashSet<ArcVarId>) -> Vec<bool> {
    events
        .per_block
        .iter()
        .map(|evs| {
            evs.iter().any(|ev| {
                matches!(
                    ev.kind,
                    EventKind::Read | EventKind::Mutate | EventKind::Consume
                ) && ev.var.is_some_and(|v| vars.contains(&v))
            })
        })
        .collect()
}

/// Blocks whose ENTRY carries a Credit re-acquisition: demand at/after such
/// a block is credit-funded and never propagates back past it.
pub(crate) fn entry_credit_blocks(events: &ClassEvents) -> Vec<bool> {
    events
        .per_block
        .iter()
        .map(|evs| {
            evs.iter()
                .any(|ev| ev.kind == EventKind::Credit && ev.site == EventSite::BlockEntry)
        })
        .collect()
}

/// Per-block seed flags: with `demand_only`, blocks holding a value use
/// (Read / Mutate / Consume); otherwise blocks holding ANY event.
pub(crate) fn event_blocks(events: &ClassEvents, demand_only: bool) -> Vec<bool> {
    events
        .per_block
        .iter()
        .map(|evs| {
            evs.iter().any(|ev| {
                !demand_only
                    || matches!(
                        ev.kind,
                        EventKind::Read | EventKind::Mutate | EventKind::Consume
                    )
            })
        })
        .collect()
}
