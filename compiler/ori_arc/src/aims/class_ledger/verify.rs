//! Per-class readiness verification — the credit-net verdict shape run once
//! per partition class over the class's event stream plus its planned ops.
//!
//! Gate order: non-convergence first, then merge disagreement, then the
//! per-path running-count walk (every event floor satisfied; every terminal
//! kind — Return, Resume, Unreachable — nets 0, matching the terminal-net
//! check `balance_verdict_from_nets` applies uniformly across all three).
//! Unwind edges run under the same rules — no special-casing.
//! Spec: Annex E §AIMS RL-2 + RL-4 + RL-comp.

use crate::aims::intraprocedural::birth_site_partition::NodeIdx;
use crate::aims::intraprocedural::ledger_events::EventSite;
use crate::aims::verify::burden_delta::compute_burden_entry_nets;
use crate::ir::{ArcFunction, ArcTerminator};

use super::emit::{DeclineReason, PlanSlot, PlannedOp, PlannedOpKind};
use super::events::ClassEvents;

/// Per-class verification verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClassVerdict {
    /// Every terminal — Return, Resume, Unreachable — nets 0; the running
    /// owed count satisfies every event floor.
    Clean,
    /// Some terminal (Return, Resume, or Unreachable) nets > 0 (an
    /// unreleased reference); everything else holds.
    LeakOnly,
    /// Non-convergence, merge disagreement, a floor violation, a negative
    /// terminal net, or no reachable terminal walked at all (a diverging
    /// function verifies by its Resume/Unreachable terminals).
    Unprovable,
}

/// Whole-function readiness: the plan is trusted only when no class
/// declined and every class verifies `Clean`.
#[derive(Debug)]
pub(crate) struct ReadinessSummary {
    pub(crate) all_classes_clean: bool,
    /// Per-class verdict, every collected class (declined ones verify
    /// against their bare event stream).
    pub(crate) verdicts: Vec<(NodeIdx, ClassVerdict)>,
    /// Classes the emitter declined, with the fail-closed reason.
    pub(crate) declined: Vec<(NodeIdx, DeclineReason)>,
}

/// One ordered walk item: a class event or a planned op.
#[derive(Clone, Copy, Debug)]
struct WalkItem {
    delta: i64,
    floor: i64,
}

/// Verify one class's events plus planned ops against the owed invariant.
pub(crate) fn verify_class(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    events: &ClassEvents,
    ops: &[PlannedOp],
) -> ClassVerdict {
    let walks = build_walks(func, events, ops);
    let delta: Vec<i64> = walks
        .iter()
        .map(|walk| walk.iter().map(|item| item.delta).sum())
        .collect();
    let nets = compute_burden_entry_nets(func, preds, &delta);
    if !nets.converged {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            gate = "non-converged",
            "class verdict Unprovable: entry-net fixpoint did not converge"
        );
        return ClassVerdict::Unprovable;
    }
    if !nets.disagree_blocks.is_empty() {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            gate = "merge-disagree",
            blocks = ?nets.disagree_blocks,
            "class verdict Unprovable: predecessor edges disagree on the owed count into a merge"
        );
        return ClassVerdict::Unprovable;
    }
    walk_paths(func, &nets.entry_net, &walks)
}

/// Run the per-path running-count walk over every reachable block.
fn walk_paths(
    func: &ArcFunction,
    entry_net: &[Option<i64>],
    walks: &[Vec<WalkItem>],
) -> ClassVerdict {
    let mut saw_terminal = false;
    let mut leak = false;
    for (block, arc_block) in func.blocks.iter().enumerate() {
        let Some(Some(entry)) = entry_net.get(block) else {
            continue;
        };
        let mut running = *entry;
        for (index, item) in walks[block].iter().enumerate() {
            if running < item.floor {
                tracing::trace!(
                    target: "ori_arc::aims::class_ledger",
                    gate = "floor",
                    block,
                    walk_index = index,
                    running,
                    floor = item.floor,
                    "class verdict Unprovable: running owed count below an event floor"
                );
                return ClassVerdict::Unprovable;
            }
            running += item.delta;
        }
        match arc_block.terminator {
            ArcTerminator::Return { .. } => {
                saw_terminal = true;
                if running > 0 {
                    tracing::trace!(
                        target: "ori_arc::aims::class_ledger",
                        gate = "leak",
                        block,
                        running,
                        "class terminal nets > 0 (unreleased reference at Return)"
                    );
                    leak = true;
                } else if running < 0 {
                    tracing::trace!(
                        target: "ori_arc::aims::class_ledger",
                        gate = "negative-terminal-net",
                        block,
                        running,
                        "class verdict Unprovable: Return terminal nets < 0"
                    );
                    return ClassVerdict::Unprovable;
                }
            }
            ArcTerminator::Resume | ArcTerminator::Unreachable => {
                saw_terminal = true;
                if running > 0 {
                    tracing::trace!(
                        target: "ori_arc::aims::class_ledger",
                        gate = "leak",
                        block,
                        running,
                        "class terminal nets > 0 (unreleased reference at Resume/Unreachable)"
                    );
                    leak = true;
                } else if running < 0 {
                    tracing::trace!(
                        target: "ori_arc::aims::class_ledger",
                        gate = "negative-terminal-net",
                        block,
                        running,
                        "class verdict Unprovable: Resume/Unreachable terminal nets < 0"
                    );
                    return ClassVerdict::Unprovable;
                }
            }
            _ => {}
        }
    }
    if !saw_terminal {
        return no_terminal_verdict(func, walks);
    }
    if leak {
        ClassVerdict::LeakOnly
    } else {
        ClassVerdict::Clean
    }
}

/// A walk item's source position inside its block: entry first, then per
/// body index (a planned op BEFORE the instruction, the instruction's own
/// event, a planned op AFTER it), then the pre-terminator slot, then the
/// terminator's own events. Derived `Ord` on (body index, phase) IS the
/// interleaving contract — a planned `BlockFront` op follows entry events,
/// a `BeforeTerminator` op precedes terminator events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WalkKey {
    index: usize,
    phase: WalkPhase,
}

/// Within-index phase ordering (declaration order is the `Ord` contract).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum WalkPhase {
    Before,
    At,
    After,
}

impl WalkKey {
    fn for_event(func: &ArcFunction, block: usize, site: EventSite) -> Self {
        match site {
            // Entry events precede a BlockFront op: entry is index 0
            // Before, the op index 0 At.
            EventSite::BlockEntry => Self {
                index: 0,
                phase: WalkPhase::Before,
            },
            // Body events sit at their own instruction, between the
            // before/after op slots: index+1 keeps them past the entry pair.
            EventSite::Body(index) => Self {
                index: index + 1,
                phase: WalkPhase::At,
            },
            EventSite::Terminator => Self {
                index: body_len(func, block) + 1,
                phase: WalkPhase::At,
            },
        }
    }

    fn for_op(func: &ArcFunction, slot: PlanSlot) -> Self {
        match slot {
            PlanSlot::BlockFront { .. } => Self {
                index: 0,
                phase: WalkPhase::At,
            },
            PlanSlot::BeforeBody { index, .. } => Self {
                index: index + 1,
                phase: WalkPhase::Before,
            },
            PlanSlot::AfterBody { index, .. } => Self {
                index: index + 1,
                phase: WalkPhase::After,
            },
            PlanSlot::BeforeTerminator { block } => Self {
                index: body_len(func, block) + 1,
                phase: WalkPhase::Before,
            },
        }
    }
}

/// Merge events and planned ops into per-block walks ordered by source
/// position per [`WalkKey`]'s derived ordering; ties keep arrival order.
/// Verdict for a DIVERGING function (no entry-reachable terminal): a class
/// whose EVERY walk item sits in entry-UNREACHABLE code never executes —
/// the placement clauses hold vacuously. Any reachable item keeps the
/// fail-closed verdict (a diverging loop touching the class still owes
/// per-iteration balance).
fn no_terminal_verdict(func: &ArcFunction, walks: &[Vec<WalkItem>]) -> ClassVerdict {
    let mut reachable = vec![false; func.blocks.len()];
    let mut stack = vec![func.entry.index()];
    while let Some(block) = stack.pop() {
        if reachable.get(block).copied().unwrap_or(true) {
            continue;
        }
        reachable[block] = true;
        for successor in super::events::successors_of(func, block) {
            stack.push(successor);
        }
    }
    let touches_reachable = walks
        .iter()
        .enumerate()
        .any(|(block, items)| !items.is_empty() && reachable.get(block).copied().unwrap_or(false));
    if !touches_reachable {
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            gate = "unreachable-class-vacuous",
            "class verdict Clean: every event + op sits in entry-unreachable code"
        );
        return ClassVerdict::Clean;
    }
    tracing::trace!(
        target: "ori_arc::aims::class_ledger",
        gate = "no-reachable-terminal",
        "class verdict Unprovable: no reachable terminal was walked"
    );
    ClassVerdict::Unprovable
}

fn build_walks(func: &ArcFunction, events: &ClassEvents, ops: &[PlannedOp]) -> Vec<Vec<WalkItem>> {
    let mut keyed: Vec<Vec<(WalkKey, usize, WalkItem)>> = vec![Vec::new(); func.blocks.len()];
    let mut sequence = 0usize;
    for (block, evs) in events.per_block.iter().enumerate() {
        for ev in evs {
            let key = WalkKey::for_event(func, block, ev.site);
            if let Some(items) = keyed.get_mut(block) {
                items.push((
                    key,
                    sequence,
                    WalkItem {
                        delta: ev.delta,
                        floor: ev.floor,
                    },
                ));
            }
            sequence += 1;
        }
    }
    for op in ops {
        let block = op.slot.block();
        let key = WalkKey::for_op(func, op.slot);
        let item = match op.kind {
            PlannedOpKind::Inc => WalkItem { delta: 1, floor: 0 },
            PlannedOpKind::Dec | PlannedOpKind::DecPartial { .. } => WalkItem {
                delta: -1,
                floor: 1,
            },
        };
        if let Some(items) = keyed.get_mut(block) {
            items.push((key, sequence, item));
        }
        sequence += 1;
    }
    keyed
        .into_iter()
        .map(|mut items| {
            items.sort_by_key(|&(key, seq, _)| (key, seq));
            items.into_iter().map(|(_, _, item)| item).collect()
        })
        .collect()
}

/// Body length of `block`, zero when out of range.
fn body_len(func: &ArcFunction, block: usize) -> usize {
    func.blocks
        .get(block)
        .map_or(0, |arc_block| arc_block.body.len())
}
