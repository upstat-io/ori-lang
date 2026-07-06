//! Per-class event extraction and liveness over the class-ledger
//! classifier's per-block streams — the placement-ready view the emitter
//! and the per-class verifier consume.

use rustc_hash::FxHashSet;

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use crate::aims::intraprocedural::ledger_events::{
    sib_read_count, ClassInstr, ClassOrigin, EventSite, LedgerClassification,
};
use crate::graph::successor_block_ids;
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

/// Event vocabulary of one class-resolved instruction, placement-ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    Birth,
    Credit,
    Consume,
    Read,
    Mutate,
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
    /// Indexed by block position; stream order preserved within a block.
    pub(crate) per_block: Vec<Vec<ClassEvent>>,
}

impl ClassEvents {
    /// Whether the class roots at a borrowed function param — the caller
    /// retains ownership, so the class owes nothing at birth and every
    /// hand-off needs its own funded reference.
    pub(crate) fn is_borrowed_rooted(&self) -> bool {
        self.origin == Some(ClassOrigin::Borrowed)
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
    let origin = classification.class_origins.get(&class).copied();
    let borrowed = origin == Some(ClassOrigin::Borrowed);
    let mut per_block: Vec<Vec<ClassEvent>> = vec![Vec::new(); func.blocks.len()];
    for (block, stream) in classification.blocks.iter().enumerate() {
        for (position, instr) in stream.iter().enumerate() {
            if class_of(instr) != class {
                continue;
            }
            let site = event_site(classification, block, position);
            let var = resolve_event_var(func, partition, class, block, site, instr);
            let (kind, delta, floor) = event_shape(
                func,
                classification,
                class,
                block,
                position,
                instr,
                borrowed,
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
    ClassEvents { origin, per_block }
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

/// Backward closure: `true` at `b` iff `seed[b]` or any block reachable
/// from `b` is seeded. Monotone boolean fixpoint (at most `n` rounds).
pub(crate) fn live_from(func: &ArcFunction, seed: &[bool]) -> Vec<bool> {
    let mut live = seed.to_vec();
    let mut changed = true;
    while changed {
        changed = false;
        for block in 0..live.len() {
            if live[block] {
                continue;
            }
            if successors_of(func, block).iter().any(|&s| live[s]) {
                live[block] = true;
                changed = true;
            }
        }
    }
    live
}

/// Whether any successor of `block` is live.
pub(crate) fn live_out(func: &ArcFunction, block: usize, live: &[bool]) -> bool {
    successors_of(func, block)
        .iter()
        .any(|&s| live.get(s).copied().unwrap_or(false))
}

/// Resolve the member variable an event names. Reads and mutates carry it;
/// every other kind resolves through the source instruction's variables
/// (operands first for a consume — the handed-off reference — destination
/// first otherwise).
fn resolve_event_var(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class: NodeIdx,
    block: usize,
    site: EventSite,
    instr: &ClassInstr,
) -> Option<ArcVarId> {
    if let ClassInstr::Read { value, .. } | ClassInstr::Mutate { value, .. } = *instr {
        return Some(value);
    }
    let consume = matches!(instr, ClassInstr::Consume { .. });
    let candidates = match site {
        EventSite::BlockEntry => block_entry_candidates(func, block),
        EventSite::Body(index) => body_candidates(func, block, index, consume),
        EventSite::Terminator => terminator_candidates(func, block, instr),
    };
    candidates
        .into_iter()
        .find(|&var| is_member(partition, var, class))
}

/// Whether `var`'s whole-variable node belongs to `class`.
fn is_member(partition: &mut BirthSitePartition, var: ArcVarId, class: NodeIdx) -> bool {
    let node = partition.register_node(var, FieldPath::whole_var());
    partition.rep_of(node) == class
}

/// Candidate vars at a block-entry site: function params (entry block),
/// this block's params, and every `Invoke`/`InvokeIndirect` result
/// materialized at this block's entry (its normal successor).
fn block_entry_candidates(func: &ArcFunction, block: usize) -> Vec<ArcVarId> {
    let mut candidates: Vec<ArcVarId> = Vec::new();
    if block == func.entry.index() {
        candidates.extend(func.params.iter().map(|p| p.var));
    }
    if let Some(arc_block) = func.blocks.get(block) {
        candidates.extend(arc_block.params.iter().map(|&(param, _)| param));
    }
    for pred_block in &func.blocks {
        if let ArcTerminator::Invoke { dst, normal, .. }
        | ArcTerminator::InvokeIndirect { dst, normal, .. } = &pred_block.terminator
        {
            if normal.index() == block {
                candidates.push(*dst);
            }
        }
    }
    candidates
}

/// Candidate vars at a body site.
fn body_candidates(func: &ArcFunction, block: usize, index: usize, consume: bool) -> Vec<ArcVarId> {
    let Some(instr) = func
        .blocks
        .get(block)
        .and_then(|arc_block| arc_block.body.get(index))
    else {
        return Vec::new();
    };
    let defined: Vec<ArcVarId> = instr.defined_var().into_iter().collect();
    let used: Vec<ArcVarId> = instr.used_vars().into_iter().collect();
    if consume {
        used.into_iter().chain(defined).collect()
    } else {
        defined.into_iter().chain(used).collect()
    }
}

/// Candidate vars at a terminator site. A cross-class Jump CREDIT names the
/// target block's param; an Invoke result birth/credit names the
/// destination; everything else names the terminator's own operand vars.
fn terminator_candidates(func: &ArcFunction, block: usize, instr: &ClassInstr) -> Vec<ArcVarId> {
    let Some(arc_block) = func.blocks.get(block) else {
        return Vec::new();
    };
    let terminator = &arc_block.terminator;
    let mut candidates: Vec<ArcVarId> = Vec::new();
    if matches!(instr, ClassInstr::Credit { .. } | ClassInstr::Birth { .. }) {
        match terminator {
            ArcTerminator::Jump { target, .. } => {
                if let Some(target_block) = func.blocks.get(target.index()) {
                    candidates.extend(target_block.params.iter().map(|&(param, _)| param));
                }
            }
            ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } => {
                candidates.push(*dst);
            }
            _ => {}
        }
    }
    candidates.extend(terminator.used_vars());
    candidates
}
