//! Per-class ledger-event classifier over the birth-site partition.
//!
//! Classifies every partition-class member use into the per-class event
//! vocabulary BIRTH / CREDIT / CONSUME / READ / MUTATE. The classification
//! mirrors the proven ledger calculus: events derive from the committed
//! RL-2 twelve-kind terminal-use table
//! (`AimsProof.Realization::rl2_use_transfers_ownership`) and the per-class
//! derivation (`AimsProof.Ledger::deriveLedger`); call-boundary events
//! classify through the callee contract (`AimsProof.ContractBoundary`),
//! never by re-deriving the callee body.
//!
//! Pure analysis: reads the IR, the converged state map, and the populated
//! partition; emits per-block class-instruction streams plus per-class
//! origin attribution. Placement is the emitter's job — this module mutates
//! nothing and emits no burden ops.

mod types;

#[cfg(test)]
pub(crate) use types::TerminalUse;
pub(crate) use types::{
    BoundaryFacts, ClassInstr, ClassOrigin, EventSite, LedgerClassification, LedgerEvent,
};

use rustc_hash::FxHashMap;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};
use crate::ownership::Ownership;

use super::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use super::state_map::AimsStateMap;

/// Classify every partition-class member use in `func` into per-block
/// class-instruction streams.
///
/// Inputs: the converged `state_map` (scalar/immortal exclusion + the
/// contract-derived apply-result alias table) and the POPULATED `partition`
/// (post `compute_birth_site_partition`; representatives are stable).
/// `boundary_facts` carries the PV-4 callee-contract projections keyed by
/// callee name; an absent entry classifies conservatively (owned args
/// consume, borrowed args read, results are `Opaque`).
pub(crate) fn classify_function(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    partition: &mut BirthSitePartition,
    boundary_facts: &FxHashMap<Name, BoundaryFacts>,
) -> LedgerClassification {
    let mut classifier = Classifier {
        state_map,
        partition,
        boundary_facts,
        out: LedgerClassification::default(),
        pending_entry: FxHashMap::default(),
        placeholders: collect_placeholder_vars(func, state_map),
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
    let mut siblings: Vec<ArcVarId> = Vec::new();
    for instr in rest {
        let ClassInstr::Read {
            class: c,
            value: read_value,
        } = *instr
        else {
            continue;
        };
        if c == class && read_value != value && !siblings.contains(&read_value) {
            siblings.push(read_value);
        }
    }
    siblings.len()
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

/// The per-function classification walk state.
struct Classifier<'a> {
    state_map: &'a AimsStateMap,
    partition: &'a mut BirthSitePartition,
    boundary_facts: &'a FxHashMap<Name, BoundaryFacts>,
    out: LedgerClassification,
    /// `Invoke`/`InvokeIndirect` result events routed to their NORMAL
    /// successor's block entry (pre-recorded by
    /// `record_invoke_result_entries`; drained by the block walk).
    pending_entry: FxHashMap<usize, Vec<ClassInstr>>,
    /// Vars defined by NON-string literals under a heap repr — placeholders
    /// (the iterator-protocol `%n: [T] = 0` shape), never allocations; no
    /// event attaches to them.
    placeholders: rustc_hash::FxHashSet<ArcVarId>,
}

impl Classifier<'_> {
    /// The class representative of `var`'s whole-variable node.
    fn rep(&mut self, var: ArcVarId) -> NodeIdx {
        let node = self.partition.register_node(var, FieldPath::whole_var());
        self.partition.rep_of(node)
    }

    fn excluded(&self, var: ArcVarId) -> bool {
        self.state_map.is_excluded(var) || self.placeholders.contains(&var)
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
                    normal,
                    ..
                } => (self.call_result_event(*dst, Some(*callee)), normal.index()),
                ArcTerminator::InvokeIndirect { dst, normal, .. } => {
                    (self.call_result_event(*dst, None), normal.index())
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
    /// BORROWED.
    fn classify_params(&mut self, func: &ArcFunction, stream: &mut Vec<ClassInstr>) {
        for param in &func.params {
            if self.excluded(param.var) {
                continue;
            }
            let origin = match param.ownership {
                Ownership::Owned => ClassOrigin::Foreign,
                Ownership::Borrowed => ClassOrigin::Borrowed,
            };
            self.birth(stream, param.var, origin);
        }
    }
}

mod dispatch;

#[cfg(test)]
mod tests;
