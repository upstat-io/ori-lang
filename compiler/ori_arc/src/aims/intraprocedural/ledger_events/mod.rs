//! Per-class ledger-event classifier over the birth-site partition.
//!
//! Classifies every partition-class member use into the per-class event
//! vocabulary BIRTH / CREDIT / CONSUME / READ / MUTATE. RL-2 terminal-use
//! rules and callee contracts determine events without re-deriving callees.
//!
//! This pure analysis emits per-block streams and class origins; placement is
//! left to the emitter. Diagnostics intentionally share the
//! `ori_arc::aims::class_ledger` target with the owning planner.

mod dispatch;
mod types;

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};
use crate::ownership::Ownership;

use super::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use super::state_map::AimsStateMap;
#[cfg(test)]
pub(crate) use types::TerminalUse;
pub(crate) use types::{
    BoundaryFacts, ClassInstr, ClassOrigin, EventSite, LedgerClassification, LedgerEvent,
};

/// The per-function classification walk state.
struct Classifier<'a> {
    /// The function under classification (repr lookups for the concat
    /// operand ownership split).
    func: &'a ArcFunction,
    state_map: &'a AimsStateMap,
    partition: &'a mut BirthSitePartition,
    boundary_facts: &'a FxHashMap<Name, BoundaryFacts>,
    /// The interned `@iter` protocol-builtin name (the iterator-creation
    /// call the borrowed-rooted READ classification keys on).
    iter_name: Name,
    /// Retain-less borrow-view accessor callees (`trace`/`trace_entries`):
    /// their result is a pure interior view — no credit, no birth.
    borrow_view_accessors: rustc_hash::FxHashSet<Name>,
    /// Vars whose `Let`-alias chain roots at a Borrowed param this
    /// function's own contract does NOT mark iter-consuming — the emitter's
    /// `is_var_borrowed_rooted` twin: an `@iter [own]` hand-off of such a
    /// var creates a NON-owning iterator (the owner/caller releases the
    /// source per `RL2_borrowed_param_emits_caller_dec`), so it classifies
    /// READ, never CONSUME.
    borrowed_rooted_vars: rustc_hash::FxHashSet<ArcVarId>,
    out: LedgerClassification,
    /// `Invoke`/`InvokeIndirect` result events routed to their NORMAL
    /// successor's block entry (pre-recorded by
    /// `record_invoke_result_entries`; drained by the block walk).
    pending_entry: FxHashMap<usize, Vec<ClassInstr>>,
    /// Vars defined by NON-string literals under a heap repr — placeholders
    /// (the iterator-protocol `%n: [T] = 0` shape), never allocations; no
    /// event attaches to them.
    placeholders: rustc_hash::FxHashSet<ArcVarId>,
    /// ABSENT params (contract cardinality `Absent`): caller-retained per
    /// the borrowed call-site convention, no events booked — excluded
    /// under the classifier's own semantics (VF-2 guards no-live-uses).
    absent_params: rustc_hash::FxHashSet<ArcVarId>,
    /// Scalar-excluded vars ADMITTED for their user-`@drop` obligation
    /// (RL-DROP: no refcount, one balance-neutral drop obligation booked
    /// like a reference).
    user_drop_admitted: rustc_hash::FxHashSet<ArcVarId>,
}

impl Classifier<'_> {
    /// The class representative of `var`'s whole-variable node.
    fn rep(&mut self, var: ArcVarId) -> NodeIdx {
        let node = self.partition.register_node(var, FieldPath::whole_var());
        self.partition.rep_of(node)
    }

    fn excluded(&self, var: ArcVarId) -> bool {
        (self.state_map.is_excluded(var) && !self.user_drop_admitted.contains(&var))
            || self.placeholders.contains(&var)
            || self.absent_params.contains(&var)
    }

    fn birth(&mut self, stream: &mut Vec<ClassInstr>, var: ArcVarId, origin: ClassOrigin) {
        let instr = self.birth_instr(var, origin);
        stream.push(instr);
    }

    /// Build a birth event and record the class origin.
    fn birth_instr(&mut self, var: ArcVarId, origin: ClassOrigin) -> ClassInstr {
        let class = self.rep(var);
        self.out.class_origins.insert(class, origin);
        ClassInstr::Birth { class, origin }
    }

    /// Pre-record every `Invoke`/`InvokeIndirect` RESULT event at its NORMAL
    /// successor's block entry: the result materializes only when the call
    /// returns, so the unwind path never inherits an owed count for it
    /// (PV-4: the boundary credit lands where the return lands).
    fn record_invoke_result_entries(&mut self, func: &ArcFunction) {
        for arc_block in &func.blocks {
            let (event, normal) = match &arc_block.terminator {
                ArcTerminator::Invoke {
                    dst,
                    func: callee,
                    args,
                    normal,
                    ..
                } => (
                    self.call_result_event(*dst, Some(*callee), args.first().copied()),
                    normal.index(),
                ),
                ArcTerminator::InvokeIndirect { dst, normal, .. } => {
                    (self.call_result_event(*dst, None, None), normal.index())
                }
                _ => continue,
            };
            if let Some(event) = event {
                self.pending_entry.entry(normal).or_default().push(event);
            }
        }
    }

    fn consume(&mut self, stream: &mut Vec<ClassInstr>, var: ArcVarId) {
        let class = self.rep(var);
        stream.push(ClassInstr::Consume { class });
    }

    fn read(&mut self, stream: &mut Vec<ClassInstr>, var: ArcVarId) {
        let class = self.rep(var);
        stream.push(ClassInstr::Read { class, value: var });
    }

    /// A fresh allocation site: the destination births FRESH and every
    /// non-excluded argument hands its reference into the allocation (a
    /// transfer terminal use — `ConstructArg` / `ReuseArg` /
    /// `CollectionReuseArg` / `PartialApplyCapture` all transfer per the
    /// committed table).
    fn classify_fresh_alloc(
        &mut self,
        stream: &mut Vec<ClassInstr>,
        dst: ArcVarId,
        args: &[ArcVarId],
    ) {
        if !self.excluded(dst) {
            self.birth(stream, dst, ClassOrigin::Fresh);
        }
        for &arg in args {
            if !self.excluded(arg) {
                self.consume(stream, arg);
            }
        }
    }

    /// A COW-mutating use of `base`'s class.
    fn mutate(&mut self, stream: &mut Vec<ClassInstr>, base: ArcVarId) {
        if self.excluded(base) {
            return;
        }
        let class = self.rep(base);
        stream.push(ClassInstr::Mutate { class, value: base });
    }

    /// Attribute MERGE origin to every multi-predecessor block param whose
    /// class the population pass REFUSED to unify (cross-class per edge; the
    /// class is funded by per-edge credits, never a birth event).
    fn record_merge_origins(&mut self, func: &ArcFunction) {
        let mut incoming: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
        for arc_block in &func.blocks {
            let ArcTerminator::Jump { target, args } = &arc_block.terminator else {
                continue;
            };
            let Some(target_block) = func.blocks.get(target.index()) else {
                continue;
            };
            for (&arg, &(param, _)) in args.iter().zip(target_block.params.iter()) {
                incoming.entry(param).or_default().push(arg);
            }
        }
        for (param, args) in incoming {
            if self.excluded(param) || args.len() < 2 {
                continue;
            }
            let param_class = self.rep(param);
            let unified = args.iter().any(|&arg| {
                let arg_class = self.rep(arg);
                arg_class == param_class
            });
            if !unified {
                self.out
                    .class_origins
                    .insert(param_class, ClassOrigin::Merge);
            }
        }
    }

    /// Function params birth their classes: Owned = FOREIGN, Borrowed =
    /// BORROWED — EXCEPT a Borrowed param whose own contract requires a
    /// whole-value credit. Iter-consuming params receive the caller's
    /// transferred reference; borrowed-COW-consuming params receive a newly
    /// funded reference while the caller retains its original owner. Both
    /// arrive owning one reference and therefore birth FOREIGN.
    fn classify_params(&mut self, func: &ArcFunction, stream: &mut Vec<ClassInstr>) {
        let incoming_credit: Vec<bool> = self.boundary_facts.get(&func.name).map_or_else(
            || vec![false; func.params.len()],
            |facts| {
                (0..func.params.len())
                    .map(|position| facts.incoming_whole_value_credit(position))
                    .collect()
            },
        );
        let own_absent: Vec<bool> = self.boundary_facts.get(&func.name).map_or_else(
            || vec![false; func.params.len()],
            |facts| {
                (0..func.params.len())
                    .map(|position| {
                        facts
                            .param_cardinality_absent
                            .get(position)
                            .copied()
                            .unwrap_or(false)
                    })
                    .collect()
            },
        );
        for (position, param) in func.params.iter().enumerate() {
            if self.excluded(param.var) {
                continue;
            }
            // Absent parameters are caller-retained borrowed arguments and book
            // no callee events; VF-2 guards the no-live-use premise.
            if own_absent.get(position).copied().unwrap_or(false) {
                tracing::trace!(
                    target: "ori_arc::aims::class_ledger",
                    param = ?param.var,
                    "absent param books no events: caller-retained per the \
                     borrowed call-site convention"
                );
                self.absent_params.insert(param.var);
                continue;
            }
            let owns_credit = incoming_credit.get(position).copied().unwrap_or(false);
            let origin = match param.ownership {
                Ownership::Owned => ClassOrigin::Foreign,
                Ownership::Borrowed if owns_credit => ClassOrigin::Foreign,
                Ownership::Borrowed => ClassOrigin::Borrowed,
            };
            self.birth(stream, param.var, origin);
        }
    }
}

/// Classify every partition-class member use in `func` into per-block
/// class-instruction streams.
///
/// Inputs: the converged `state_map` (scalar/immortal exclusion + the
/// contract-derived apply-result alias table) and the POPULATED `partition`
/// (post `compute_birth_site_partition`; representatives are stable).
/// `boundary_facts` carries the PV-4 callee-contract projections keyed by
/// callee name; an absent entry classifies conservatively (owned args
/// consume, borrowed args read, results are `Opaque`).
#[cfg(test)] // test-only convenience: production routes through the admitted variant
pub(crate) fn classify_function(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    partition: &mut BirthSitePartition,
    boundary_facts: &FxHashMap<Name, BoundaryFacts>,
    interner: &ori_ir::StringInterner,
) -> LedgerClassification {
    classify_function_with_admitted(
        func,
        state_map,
        partition,
        boundary_facts,
        interner,
        rustc_hash::FxHashSet::default(),
    )
}

/// [`classify_function`] with an ADMISSION override: scalar-excluded vars in
/// `user_drop_admitted` are classified — the user-`@drop` obligation
/// carriers whose books hold one balance-neutral drop obligation.
pub(crate) fn classify_function_with_admitted(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    partition: &mut BirthSitePartition,
    boundary_facts: &FxHashMap<Name, BoundaryFacts>,
    interner: &ori_ir::StringInterner,
    user_drop_admitted: rustc_hash::FxHashSet<ArcVarId>,
) -> LedgerClassification {
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let mut classifier = Classifier {
        func,
        state_map,
        partition,
        boundary_facts,
        iter_name,
        borrow_view_accessors: crate::borrow::borrow_view_accessor_builtin_names(interner),
        borrowed_rooted_vars: collect_borrowed_rooted_vars(func, boundary_facts),
        out: LedgerClassification::default(),
        pending_entry: FxHashMap::default(),
        placeholders: collect_placeholder_vars(func, state_map),
        absent_params: rustc_hash::FxHashSet::default(),
        user_drop_admitted,
    };
    classifier.record_merge_origins(func);
    classifier.record_invoke_result_entries(func);
    for (block_idx, arc_block) in func.blocks.iter().enumerate() {
        let mut stream = Vec::new();
        let mut sites = Vec::new();
        if block_idx == func.entry.index() {
            classifier.classify_params(func, &mut stream);
        }
        if let Some(pending) = classifier.pending_entry.remove(&block_idx) {
            stream.extend(pending);
        }
        sites.resize(stream.len(), EventSite::BlockEntry);
        for (position, instr) in arc_block.body.iter().enumerate() {
            classifier.classify_instr(instr, &mut stream);
            sites.resize(stream.len(), EventSite::Body(position));
        }
        classifier.classify_terminator(func, &arc_block.terminator, &mut stream);
        sites.resize(stream.len(), EventSite::Terminator);
        classifier.out.blocks.push(stream);
        classifier.out.sites.push(sites);
    }
    // An admitted user-drop scalar without any ledger event has no booking
    // surface and is empty-plan equivalent. Other live eventless variables
    // remain classifier gaps, and any booking ends the exemption.
    let mut evented_reps = rustc_hash::FxHashSet::default();
    let mut consumed_reps = rustc_hash::FxHashSet::default();
    for block_idx in 0..classifier.out.blocks.len() {
        for event_idx in 0..classifier.out.blocks[block_idx].len() {
            let class = match classifier.out.blocks[block_idx][event_idx] {
                ClassInstr::Birth { class, .. }
                | ClassInstr::Credit { class }
                | ClassInstr::SelectCredit { class, .. }
                | ClassInstr::Read { class, .. }
                | ClassInstr::Mutate { class, .. } => class,
                ClassInstr::Consume { class } => {
                    consumed_reps.insert(classifier.partition.rep_of(class));
                    class
                }
            };
            evented_reps.insert(classifier.partition.rep_of(class));
        }
    }
    let node_reps: Vec<(ArcVarId, NodeIdx)> = classifier
        .partition
        .nodes_snapshot()
        .into_iter()
        .map(|(var, _path, node)| (var, node))
        .collect();
    classifier.out.all_vars_excluded = (0..func.var_types.len()).all(|raw| {
        u32::try_from(raw).is_ok_and(|raw| {
            let var = ArcVarId::new(raw);
            if classifier.excluded(var) {
                return true;
            }
            classifier.user_drop_admitted.contains(&var)
                && node_reps.iter().all(|&(node_var, node)| {
                    node_var != var || !evented_reps.contains(&classifier.partition.rep_of(node))
                })
        })
    });
    classifier
        .out
        .user_drop_admitted
        .clone_from(&classifier.user_drop_admitted);
    classifier.out.consume_covered = classifier
        .partition
        .nodes_snapshot()
        .into_iter()
        .filter(|(_, path, node)| {
            path.is_whole_var() && consumed_reps.contains(&classifier.partition.rep_of(*node))
        })
        .map(|(var, _, _)| var)
        .collect();
    classifier.out
}

/// Derive class `c`'s ledger from a walk's flattened class-instruction
/// stream — the Rust mirror of `AimsProof.Ledger::deriveLedger`. The
/// dynamic-COW live-sibling floor is computed from the path suffix at
/// derivation time (`sibReadCount`), never free input.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "test-pinned mirror of AimsProof.Ledger::deriveLedger; the class-ledger emitter computes per-class deltas directly in class_ledger::events::event_shape, never through this derivation"
    )
)]
pub(crate) fn derive_ledger(class: NodeIdx, instrs: &[ClassInstr]) -> Vec<LedgerEvent> {
    let mut events = Vec::new();
    for (position, instr) in instrs.iter().enumerate() {
        match *instr {
            ClassInstr::Birth { class: c, .. } if c == class => events.push(LedgerEvent::Birth),
            ClassInstr::Credit { class: c } if c == class => events.push(LedgerEvent::Credit),
            ClassInstr::SelectCredit { class: c, .. } if c == class => {
                events.push(LedgerEvent::Credit);
            }
            ClassInstr::Consume { class: c } if c == class => events.push(LedgerEvent::Consume),
            ClassInstr::Read { class: c, .. } if c == class => events.push(LedgerEvent::Read),
            ClassInstr::Mutate { class: c, value } if c == class => {
                events.push(LedgerEvent::Mutate {
                    live_siblings: sib_read_count(class, value, &instrs[position + 1..]),
                });
            }
            _ => {}
        }
    }
    events
}

/// Distinct OTHER same-class values still read in the path suffix — the
/// live-sibling count a dynamic-COW mutate's floor demands
/// (`AimsProof.Ledger::sibReadCount`).
pub(crate) fn sib_read_count(class: NodeIdx, value: ArcVarId, rest: &[ClassInstr]) -> usize {
    let mut siblings = rustc_hash::FxHashSet::default();
    for instr in rest {
        let ClassInstr::Read {
            class: c,
            value: read_value,
        } = *instr
        else {
            continue;
        };
        if c == class && read_value != value {
            siblings.insert(read_value);
        }
    }
    siblings.len()
}

/// The `Let`-alias closure of every Borrowed param this function's own
/// contract does NOT give an incoming whole-value credit. This is the backend-neutral
/// classification every physical projection must consume: an iterator created
/// from such a var does not own its source; the owner/caller releases it, per
/// `RL2_borrowed_param_emits_caller_dec`. The LLVM emitter's current
/// `is_var_borrowed_rooted` recheck is a projection seam, not a second policy.
fn collect_borrowed_rooted_vars(
    func: &ArcFunction,
    boundary_facts: &FxHashMap<Name, BoundaryFacts>,
) -> rustc_hash::FxHashSet<ArcVarId> {
    use crate::ir::{ArcInstr, ArcValue};
    let own_facts = boundary_facts.get(&func.name);
    let mut roots = rustc_hash::FxHashSet::default();
    for (position, param) in func.params.iter().enumerate() {
        let owns_credit = own_facts.is_some_and(|f| f.incoming_whole_value_credit(position));
        if param.ownership == Ownership::Borrowed && !owns_credit {
            roots.insert(param.var);
        }
    }
    if roots.is_empty() {
        return roots;
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if roots.contains(src) && roots.insert(*dst) {
                        changed = true;
                    }
                }
            }
        }
    }
    roots
}

/// Vars defined by NON-string literals (a placeholder value, never an
/// allocation) — event-less like excluded vars. A `Select` whose value
/// operands are ALL excluded holds an excluded value itself; exclusion
/// propagates through Select chains to a fixpoint.
fn collect_placeholder_vars(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> rustc_hash::FxHashSet<ArcVarId> {
    use crate::ir::{ArcInstr, ArcValue, LitValue};
    let mut placeholders = rustc_hash::FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Literal(lit),
                ..
            } = instr
            {
                if !matches!(lit, LitValue::String(_)) {
                    placeholders.insert(*dst);
                }
            }
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                let excluded = |set: &rustc_hash::FxHashSet<ArcVarId>, var: ArcVarId| {
                    state_map.is_excluded(var) || set.contains(&var)
                };
                match instr {
                    ArcInstr::Select {
                        dst,
                        true_val,
                        false_val,
                        ..
                    } => {
                        if !excluded(&placeholders, *dst)
                            && excluded(&placeholders, *true_val)
                            && excluded(&placeholders, *false_val)
                            && placeholders.insert(*dst)
                        {
                            changed = true;
                        }
                    }
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } => {
                        if !excluded(&placeholders, *dst)
                            && excluded(&placeholders, *src)
                            && placeholders.insert(*dst)
                        {
                            changed = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    placeholders
}

#[cfg(test)]
mod tests;
