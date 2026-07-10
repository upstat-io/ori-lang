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

/// [`extract_class_events`] with a CREDIT injected at each extraction site
/// (PV-6 per-site refinement, `FD_per_site_skipset_sound`): the store
/// consume is KEPT — a whole-var release site pays it recursively on the
/// bypass path — and each extraction re-acquires the reference the store
/// gave the container (+1), so the extraction path balances through its
/// own downstream consume. Extraction sites are `(block, body_index)` of
/// the member-defining Projects.
pub(crate) fn extract_class_events_with_extraction_credits(
    func: &ArcFunction,
    classification: &LedgerClassification,
    partition: &mut BirthSitePartition,
    class: NodeIdx,
    extraction_sites: &[(usize, usize)],
    force_owned: bool,
) -> ClassEvents {
    let mut events =
        extract_class_events_inner(func, classification, partition, class, force_owned, &[]);
    for &(block, index) in extraction_sites {
        let Some(evs) = events.per_block.get_mut(block) else {
            continue;
        };
        let site = EventSite::Body(index);
        // Insert in site order: after every event at an earlier body index
        // or block entry, before later ones.
        let pos = evs
            .iter()
            .position(|ev| match ev.site {
                EventSite::BlockEntry => false,
                EventSite::Body(i) => i > index,
                EventSite::Terminator => true,
            })
            .unwrap_or(evs.len());
        evs.insert(
            pos,
            ClassEvent {
                site,
                kind: EventKind::Credit,
                var: None,
                delta: 1,
                floor: 0,
            },
        );
    }
    events
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

/// Demand blocks restricted to the given vars (a seeded member's own alias
/// closure): only same-reference demand — a different seeded extraction is a
/// different iteration's reference and never keeps THIS one alive.
pub(crate) fn demand_blocks_of_vars(
    events: &ClassEvents,
    vars: &rustc_hash::FxHashSet<crate::ir::ArcVarId>,
) -> Vec<bool> {
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

/// One arm-local FULL MOVE (the branch-exclusive rebuild shape): in
/// `block`, every owned field of one aggregate class is projected and
/// consumed exactly once as an arg of the ONE `Construct` at
/// `construct_index` (outside the class). The aggregate's reference
/// transfers WHOLE into the new construct — the RL-2 `ConstructArg`
/// transfer (`FD_moveout_is_committed_transfer`; the full-skip cell of
/// `FD_skipset_sound`).
pub(crate) struct FullMoveArm {
    pub(crate) block: usize,
    pub(crate) construct_index: usize,
    /// `(body index, projection dst)` of each member-moving `Project`.
    pub(crate) projections: Vec<(usize, ArcVarId)>,
    /// The moved aggregate's class rep.
    pub(crate) class_rep: NodeIdx,
    /// The projected-from member var (the rebooked consume's subject).
    pub(crate) src_var: ArcVarId,
}

/// Detect every arm-local full move in `func` (pure-IR pre-pass, before
/// per-class event extraction). Per block: ONE `Construct` consuming
/// projection dsts; all those `Project`s read ONE aggregate class; each dst
/// used exactly once (at the construct); the projected field set equals the
/// aggregate burden's owned top-level field set; no OTHER use of any
/// aggregate member var in the block (class-internal `Let` aliases and the
/// `Project`s themselves permitted). Fail-closed on any mismatch.
pub(crate) fn detect_full_move_arms(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
) -> Vec<FullMoveArm> {
    let mut arms = Vec::new();
    for block in 0..func.blocks.len() {
        if let Some(arm) = full_move_arm_in_block(func, partition, type_registry, block) {
            arms.push(arm);
        }
    }
    arms
}

/// The [`FullMoveArm`] in `block`, when the shape holds (see
/// [`detect_full_move_arms`]).
fn full_move_arm_in_block(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    block: usize,
) -> Option<FullMoveArm> {
    use crate::ir::ArcInstr;

    let blk = func.blocks.get(block)?;
    // Projections in this block, keyed by dst.
    let mut projections: Vec<(usize, ArcVarId, ArcVarId, u32)> = Vec::new();
    for (i, instr) in blk.body.iter().enumerate() {
        if let ArcInstr::Project {
            dst, value, field, ..
        } = instr
        {
            projections.push((i, *dst, *value, *field));
        }
    }
    // ONE Construct consuming projection dsts.
    let mut construct_index: Option<usize> = None;
    for (i, instr) in blk.body.iter().enumerate() {
        let ArcInstr::Construct { args, .. } = instr else {
            continue;
        };
        if !projections
            .iter()
            .any(|&(_, pdst, _, _)| args.contains(&pdst))
        {
            continue;
        }
        if construct_index.is_some() {
            return None;
        }
        construct_index = Some(i);
    }
    let cidx = construct_index?;
    let ArcInstr::Construct {
        dst: construct_dst,
        args: construct_args,
        ..
    } = blk.body.get(cidx)?
    else {
        return None;
    };
    // The moved projections: those consumed by the construct. All must read
    // ONE class; each dst used exactly once (at the construct).
    let moved: Vec<(usize, ArcVarId, ArcVarId, u32)> = projections
        .iter()
        .copied()
        .filter(|&(_, pdst, _, _)| construct_args.contains(&pdst))
        .collect();
    let (_, _, first_src, _) = *moved.first()?;
    let first_node = partition.register_node(first_src, FieldPath::whole_var());
    let class_rep = partition.rep_of(first_node);
    let construct_node = partition.register_node(*construct_dst, FieldPath::whole_var());
    if partition.rep_of(construct_node) == class_rep {
        return None;
    }
    for &(_, pdst, src, _) in &moved {
        let src_node = partition.register_node(src, FieldPath::whole_var());
        if partition.rep_of(src_node) != class_rep {
            return None;
        }
        if construct_args.iter().filter(|&&arg| arg == pdst).count() != 1 {
            return None;
        }
        let other_use = blk
            .body
            .iter()
            .enumerate()
            .any(|(i, instr)| i != cidx && instr.uses_var(pdst))
            || blk.terminator.uses_var(pdst);
        if other_use {
            return None;
        }
    }
    if !class_uses_confined_to_moves(func, partition, class_rep, block, &moved) {
        return None;
    }
    if moved_class_shares_edge_source(func, partition, class_rep) {
        return None;
    }
    if !moved_fields_cover_owned(func, type_registry, first_src, &moved) {
        return None;
    }
    tracing::trace!(
        target: "ori_arc::aims::class_ledger",
        block,
        construct_index = cidx,
        "full-move arm detected: every owned field moved into one Construct"
    );
    Some(FullMoveArm {
        block,
        construct_index: cidx,
        projections: moved.iter().map(|&(i, pdst, _, _)| (i, pdst)).collect(),
        class_rep,
        src_var: first_src,
    })
}

/// No OTHER use of the moved aggregate in `block`: permitted uses are the
/// moved `Project`s themselves and `Let` aliases inside the tracked set.
/// The tracked set is the union of the class's partition members AND the
/// block-local `Let`-alias closure of the projected-from vars — the
/// per-source partition can split runtime-same-allocation lineages into
/// sibling classes (a loop-header init param vs the iteration param), and a
/// terminator hand-off of ANY alias of the moved aggregate means the value
/// survives the arm (the loop-header-merge-read over-fire: rebooking there
/// releases a field the next iteration still reads).
fn class_uses_confined_to_moves(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class_rep: NodeIdx,
    block: usize,
    moved: &[(usize, ArcVarId, ArcVarId, u32)],
) -> bool {
    use crate::ir::{ArcInstr, ArcValue};

    let Some(blk) = func.blocks.get(block) else {
        return false;
    };
    let mut tracked: FxHashSet<ArcVarId> = {
        let nodes = partition.nodes_snapshot();
        nodes
            .iter()
            .filter(|(_, path, _)| path.is_whole_var())
            .filter(|&&(_, _, node)| partition.rep_of(node) == class_rep)
            .map(|&(var, _, _)| var)
            .collect()
    };
    for &(_, _, src, _) in moved {
        tracked.insert(src);
    }
    // Close over block-local `Let { Var }` edges in BOTH directions until
    // fixpoint: an alias of a tracked var and the var an alias reads are
    // the SAME runtime value.
    loop {
        let mut grew = false;
        for instr in &blk.body {
            let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            else {
                continue;
            };
            if tracked.contains(dst) && tracked.insert(*src) {
                grew = true;
            }
            if tracked.contains(src) && tracked.insert(*dst) {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    for (i, instr) in blk.body.iter().enumerate() {
        // The aggregate must not be BORN in the arm block: a same-block
        // birth keeps the aggregate's own release here (its events are not
        // all Reads, so the rebook cannot apply) while the field credits
        // would still inject — the half-applied booking double-funds the
        // store (the no-loop single-reassign shape). `Let` aliases define
        // without birthing and stay permitted.
        if !matches!(instr, ArcInstr::Let { .. })
            && instr
                .defined_var()
                .is_some_and(|dst| tracked.contains(&dst))
        {
            return false;
        }
        for &member in &tracked {
            if !instr.uses_var(member) {
                continue;
            }
            let permitted = matches!(instr, ArcInstr::Project { .. })
                && moved.iter().any(|&(idx, _, _, _)| idx == i)
                || matches!(
                    instr,
                    ArcInstr::Let { value: ArcValue::Var(v), .. } if *v == member
                ) && matches!(instr, ArcInstr::Let { dst, .. } if tracked.contains(dst));
            if !permitted {
                return false;
            }
        }
    }
    tracked
        .iter()
        .all(|&member| !blk.terminator.uses_var(member))
}

/// Whether ANY `Jump` edge feeds a param of the moved class AND a param of
/// a DIFFERENT class from same-class args — the two lineages may alias ONE
/// runtime allocation on that edge (the loop-header init param vs iteration
/// param shape), so a full move through one lineage strands the other's
/// reads. Decline the arm.
fn moved_class_shares_edge_source(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class_rep: NodeIdx,
) -> bool {
    for blk in &func.blocks {
        let ArcTerminator::Jump { target, args } = &blk.terminator else {
            continue;
        };
        let Some(target_blk) = func.blocks.get(target.index()) else {
            continue;
        };
        let mut moved_arg_reps: Vec<NodeIdx> = Vec::new();
        let mut other_arg_reps: Vec<NodeIdx> = Vec::new();
        for (i, &(param, _)) in target_blk.params.iter().enumerate() {
            let Some(&arg) = args.get(i) else {
                continue;
            };
            let param_node = partition.register_node(param, FieldPath::whole_var());
            let arg_node = partition.register_node(arg, FieldPath::whole_var());
            let arg_rep = partition.rep_of(arg_node);
            if partition.rep_of(param_node) == class_rep {
                moved_arg_reps.push(arg_rep);
            } else {
                other_arg_reps.push(arg_rep);
            }
        }
        if moved_arg_reps
            .iter()
            .any(|rep| other_arg_reps.contains(rep))
        {
            return true;
        }
    }
    false
}

/// The moved field set equals the aggregate burden's owned top-level field
/// set (non-empty).
fn moved_fields_cover_owned(
    func: &ArcFunction,
    type_registry: &ori_types::TypeRegistry,
    src_var: ArcVarId,
    moved: &[(usize, ArcVarId, ArcVarId, u32)],
) -> bool {
    use crate::lower::burden::BurdenRef;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    let Some(&src_ty) = func.var_types.get(src_var.index()) else {
        return false;
    };
    let Some(BurdenRef::User(user)) =
        lookup_burden(idx_to_type_ref(src_ty, type_registry), type_registry)
    else {
        return false;
    };
    let owned: FxHashSet<u32> = user
        .owned_fields
        .iter()
        .filter_map(|f| f.field_path.first().copied())
        .collect();
    if owned.is_empty() {
        return false;
    }
    let moved_fields: FxHashSet<u32> = moved.iter().map(|&(_, _, _, field)| field).collect();
    moved_fields == owned
}

/// Extraction-credit sites `(block, body_index)` this class picks up from
/// the detected full-move arms: the member-moving `Project`s whose dst is a
/// member of THIS class (the field-view side of the transfer — the field's
/// reference rides the aggregate's move, so the extraction re-acquires it
/// and no duplication inc is owed).
pub(crate) fn full_move_credit_sites(
    partition: &mut BirthSitePartition,
    arms: &[FullMoveArm],
    class: NodeIdx,
) -> Vec<(usize, usize)> {
    let class_rep = partition.rep_of(class);
    let mut sites = Vec::new();
    for arm in arms {
        for &(index, dst) in &arm.projections {
            let node = partition.register_node(dst, FieldPath::whole_var());
            if partition.rep_of(node) == class_rep {
                sites.push((arm.block, index));
            }
        }
    }
    sites
}

/// The aggregate side of the full-move rebook: when THIS class is a
/// detected arm's moved aggregate and its events in that block are all
/// Reads, they rebook to ONE move-out Consume at the Construct site — the
/// per-path owed counts then agree at the downstream merge. Fail-closed:
/// any other event shape leaves the block untouched (the per-class verify
/// walk re-checks the rebooked stream independently).
pub(crate) fn apply_full_move_rebook(
    partition: &mut BirthSitePartition,
    arms: &[FullMoveArm],
    class: NodeIdx,
    events: &mut ClassEvents,
) {
    let class_rep = partition.rep_of(class);
    for arm in arms {
        if arm.class_rep != class_rep {
            continue;
        }
        let Some(evs) = events.per_block.get_mut(arm.block) else {
            continue;
        };
        if evs.is_empty() || !evs.iter().all(|ev| ev.kind == EventKind::Read) {
            continue;
        }
        evs.clear();
        evs.push(ClassEvent {
            site: EventSite::Body(arm.construct_index),
            kind: EventKind::Consume,
            var: Some(arm.src_var),
            delta: -1,
            floor: 1,
        });
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            block = arm.block,
            construct_index = arm.construct_index,
            "full-move arm rebooked: the class's Reads become its move-out Consume"
        );
    }
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
