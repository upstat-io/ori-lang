//! Per-class event extraction and liveness over the class-ledger
//! classifier's per-block streams — the placement-ready view the emitter
//! and the per-class verifier consume.

mod liveness;
mod resolve;

use rustc_hash::FxHashSet;

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use crate::aims::intraprocedural::ledger_events::{
    sib_read_count, ClassInstr, ClassOrigin, EventSite, LedgerClassification,
};
use crate::graph::successor_block_ids;
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

pub(crate) use liveness::{
    live_from, live_from_forward, live_from_forward_killing, live_from_killing, live_out,
    live_out_forward, live_out_forward_killing, live_out_killing,
};
use resolve::resolve_event_var;

/// Event vocabulary of one class-resolved instruction, placement-ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    Birth,
    Credit,
    Consume,
    Read,
    Mutate,
    /// A `Select` acquisition marker (delta 0): realized by a planner-placed
    /// RL-1 duplication inc at the select site.
    SelectCredit,
}

/// One class event: its source site, resolved subject variable, signed
/// owed-count delta, and required running floor.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClassEvent {
    pub(crate) site: EventSite,
    pub(crate) kind: EventKind,
    /// The member variable at the event site, when resolvable.
    pub(crate) var: Option<ArcVarId>,
    /// Signed change to the class's owed count.
    pub(crate) delta: i64,
    /// Required owed count immediately BEFORE the event applies.
    pub(crate) floor: i64,
}

/// All events of one class, per block, plus the class origin.
#[derive(Debug)]
pub(crate) struct ClassEvents {
    pub(crate) origin: Option<ClassOrigin>,
    /// A member reference crosses a BACK-edge silently (a same-class jump
    /// arg into a same-class loop param): the SAME reference persists into
    /// the next iteration, so liveness must include the back-edge. A class
    /// without silent back-edge threading treats back-edges as the next
    /// iteration's ledger (forward-only liveness).
    pub(crate) threads_back_edge: bool,
    /// The class is birth-less but holds a field-path member: its reference
    /// lives in a container's field slot (a param/foreign aggregate's
    /// field), released by the CONTAINER's class, never this one.
    pub(crate) container_held: bool,
    /// Whether the class's tracked reference is owned ELSEWHERE — a
    /// borrowed function param (the caller retains ownership) or a
    /// container-held field slot (the aggregate retains ownership). Either
    /// way the class owes nothing at entry and every hand-off needs its own
    /// funded reference (the borrowed-rooted discipline of the boundary
    /// calculus, with the container in the caller's role). FALSE for a
    /// `force_owned` (extraction-funded) re-extraction: the seed inc IS the
    /// class's own reference, so its hand-offs are RL-2 transfers, never
    /// duplications needing borrowed-rooted funding.
    pub(crate) externally_funded: bool,
    /// Indexed by block position; stream order preserved within a block.
    pub(crate) per_block: Vec<Vec<ClassEvent>>,
}

impl ClassEvents {
    /// The stored external-funding classification (see the field). A
    /// birth-less WHOLE-VAR class is NOT externally funded — that shape is
    /// a classification gap and stays fail-closed at the read floor.
    pub(crate) fn is_externally_funded(&self) -> bool {
        self.externally_funded
    }
}

/// Every class named by the classification, in first-seen stream order,
/// with origin-only leftovers appended in intern order (deterministic).
pub(crate) fn collect_classes(classification: &LedgerClassification) -> Vec<NodeIdx> {
    let mut seen = FxHashSet::default();
    let mut classes = Vec::new();
    for stream in &classification.blocks {
        for instr in stream {
            let class = class_of(instr);
            if seen.insert(class) {
                classes.push(class);
            }
        }
    }
    let mut leftovers: Vec<NodeIdx> = classification
        .class_origins
        .keys()
        .copied()
        .filter(|class| seen.insert(*class))
        .collect();
    leftovers.sort_unstable();
    classes.extend(leftovers);
    classes
}

/// The class an event names.
fn class_of(instr: &ClassInstr) -> NodeIdx {
    match *instr {
        ClassInstr::Birth { class, .. }
        | ClassInstr::Credit { class }
        | ClassInstr::SelectCredit { class, .. }
        | ClassInstr::Consume { class }
        | ClassInstr::Read { class, .. }
        | ClassInstr::Mutate { class, .. } => class,
    }
}

/// Extract `class`'s events per block, with resolved vars, deltas, floors.
pub(crate) fn extract_class_events(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    class: NodeIdx,
) -> ClassEvents {
    extract_class_events_with(func, classification, partition, class, false)
}

/// [`extract_class_events`] with `force_owned`: a funded-at-extraction view
/// class re-extracts under OWNED semantics (reads floor 1 against its
/// extraction inc) instead of the container-held floor-0 discipline.
pub(crate) fn extract_class_events_with(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    class: NodeIdx,
    force_owned: bool,
) -> ClassEvents {
    extract_class_events_inner(func, classification, partition, class, force_owned, &[])
}

/// [`extract_class_events`] with the store-consumes at the given sites
/// RE-BOOKED as non-consuming: a consume-marked field skipped by the
/// container's `DecPartial` never enters the container's release books, so
/// its move-in store is not an ownership handoff (IA-T6 `payloadEvents`
/// skipped cell; per `aims-rules.md §12` PV-6).
pub(crate) fn extract_class_events_rebooked(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    class: NodeIdx,
    skip_consume_sites: &[(usize, EventSite)],
) -> ClassEvents {
    extract_class_events_inner(
        func,
        classification,
        partition,
        class,
        false,
        skip_consume_sites,
    )
}

fn extract_class_events_inner(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    class: NodeIdx,
    force_owned: bool,
    skip_consume_sites: &[(usize, EventSite)],
) -> ClassEvents {
    let origin = classification.class_origins.get(&class).copied();
    let container_held = origin.is_none() && partition.class_has_field_path_member(class);
    let externally_funded =
        !force_owned && (origin == Some(ClassOrigin::Borrowed) || container_held);
    let threads_back_edge = class_threads_back_edge(func, partition, class);
    let mut per_block: Vec<Vec<ClassEvent>> = vec![Vec::new(); func.blocks.len()];
    for (block, stream) in classification.blocks.iter().enumerate() {
        for (position, instr) in stream.iter().enumerate() {
            if class_of(instr) != class {
                continue;
            }
            let site = event_site(classification, block, position);
            if matches!(instr, ClassInstr::Consume { .. })
                && skip_consume_sites.contains(&(block, site))
            {
                continue;
            }
            let var = resolve_event_var(func, partition, class, block, site, instr);
            let (kind, delta, floor) = event_shape(
                func,
                classification,
                class,
                block,
                position,
                instr,
                externally_funded,
            );
            if let Some(events) = per_block.get_mut(block) {
                events.push(ClassEvent {
                    site,
                    kind,
                    var,
                    delta,
                    floor,
                });
            }
        }
    }
    ClassEvents {
        origin,
        threads_back_edge,
        container_held,
        externally_funded,
        per_block,
    }
}

/// Whether any same-class jump arg lands in a same-class param across a
/// BACK-edge (target dominates the jumping block) — the silent threading
/// the RL-4 exemption keeps event-less.
fn class_threads_back_edge(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class: NodeIdx,
) -> bool {
    let dom = crate::graph::DominatorTree::build(func);
    for arc_block in &func.blocks {
        let ArcTerminator::Jump { target, args } = &arc_block.terminator else {
            continue;
        };
        let Some(target_block) = func.blocks.get(target.index()) else {
            continue;
        };
        if !dom.dominates(target_block.id, arc_block.id) {
            continue;
        }
        for (&arg, &(param, _)) in args.iter().zip(target_block.params.iter()) {
            let arg_node = partition.register_node(arg, FieldPath::whole_var());
            let param_node = partition.register_node(param, FieldPath::whole_var());
            if partition.rep_of(arg_node) == class && partition.rep_of(param_node) == class {
                return true;
            }
        }
    }
    false
}

/// The recorded source site of `blocks[block][position]`.
fn event_site(classification: &LedgerClassification, block: usize, position: usize) -> EventSite {
    let Some(site) = classification
        .sites
        .get(block)
        .and_then(|sites| sites.get(position))
    else {
        unreachable!("classification sites are parallel to its event streams");
    };
    *site
}

/// Kind, owed delta, and running floor of one event. A borrowed-rooted
/// class's birth owes nothing and its value reads need no owned reference
/// (the caller keeps the allocation alive).
fn event_shape(
    func: &ArcFunction,
    classification: &LedgerClassification,
    class: NodeIdx,
    block: usize,
    position: usize,
    instr: &ClassInstr,
    borrowed: bool,
) -> (EventKind, i64, i64) {
    let owned_unit = i64::from(!borrowed);
    match *instr {
        ClassInstr::Birth { .. } => (EventKind::Birth, owned_unit, 0),
        ClassInstr::Credit { .. } => (EventKind::Credit, 1, 0),
        ClassInstr::SelectCredit { .. } => (EventKind::SelectCredit, 0, 0),
        ClassInstr::Consume { .. } => (EventKind::Consume, -1, 1),
        ClassInstr::Read { .. } => (EventKind::Read, 0, owned_unit),
        ClassInstr::Mutate { value, .. } => {
            let siblings =
                suffix_sibling_reads(func, classification, class, block, position, value);
            (EventKind::Mutate, 0, owned_unit + siblings)
        }
    }
}

/// Distinct OTHER same-class values read after the mutate — the block-stream
/// suffix plus every block reachable from the mutate's block (a sound
/// over-approximation of the walk suffix; reuses the classifier's
/// `sib_read_count`).
fn suffix_sibling_reads(
    func: &ArcFunction,
    classification: &LedgerClassification,
    class: NodeIdx,
    block: usize,
    position: usize,
    value: ArcVarId,
) -> i64 {
    let mut suffix: Vec<ClassInstr> = Vec::new();
    if let Some(stream) = classification.blocks.get(block) {
        suffix.extend_from_slice(&stream[position + 1..]);
    }
    for reachable in reachable_from(func, block) {
        if reachable == block {
            continue;
        }
        if let Some(stream) = classification.blocks.get(reachable) {
            suffix.extend_from_slice(stream);
        }
    }
    i64::try_from(sib_read_count(class, value, &suffix)).unwrap_or(i64::MAX)
}

/// Blocks reachable from `block`'s successors (transitively; includes
/// `block` itself on a cycle), ascending order.
fn reachable_from(func: &ArcFunction, block: usize) -> Vec<usize> {
    let mut reachable = FxHashSet::default();
    let mut stack = successors_of(func, block);
    while let Some(next) = stack.pop() {
        if reachable.insert(next) {
            stack.extend(successors_of(func, next));
        }
    }
    let mut ordered: Vec<usize> = reachable.into_iter().collect();
    ordered.sort_unstable();
    ordered
}

/// Distinct in-range successor indices of `block`.
pub(crate) fn successors_of(func: &ArcFunction, block: usize) -> Vec<usize> {
    let Some(arc_block) = func.blocks.get(block) else {
        return Vec::new();
    };
    let mut seen = FxHashSet::default();
    successor_block_ids(&arc_block.terminator)
        .iter()
        .map(|id| id.index())
        .filter(|&idx| idx < func.blocks.len() && seen.insert(idx))
        .collect()
}

/// Demand blocks EXCLUDING seed-funded member reads: an extraction-funded
/// (seeded) member var's demand is paid by its own RL-1 inc at the `Project`
/// site, so it never counts as surviving demand on the pre-consume reference.
pub(crate) fn demand_blocks_excluding_seeded(
    events: &ClassEvents,
    seed_vars: &rustc_hash::FxHashSet<crate::ir::ArcVarId>,
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
