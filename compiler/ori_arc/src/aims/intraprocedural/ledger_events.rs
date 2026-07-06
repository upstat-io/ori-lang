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

use rustc_hash::FxHashMap;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership};
use crate::ownership::Ownership;

use super::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use super::state_map::{AimsStateMap, ApplyAliasSource};
use crate::aims::contract::MemoryContract;

/// RL-2 terminal-use kinds — the committed 12-row coverage grid.
///
/// MUST match `AimsProof.Realization::TerminalUse` member-for-member; the
/// transfer split is [`TerminalUse::transfers_ownership`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by the class-ledger emitter wiring; test-pinned until the emitter toggle lands"
    )
)]
pub(crate) enum TerminalUse {
    Return,
    ConstructArg,
    ReuseArg,
    CollectionReuseArg,
    SetValue,
    PartialApplyCapture,
    ApplyToOwnedParam,
    JumpArg,
    ApplyToIterConsumingParam,
    LastReadBeforeScopeExit,
    ScopeExit,
    ApplyToBorrowedParam,
}

impl TerminalUse {
    /// The 12-kind transfer partition: 9 transfer kinds hand the reference
    /// off (CONSUME); 3 non-transfer kinds are the terminal READ the placed
    /// dec must follow. Mirrors `rl2_use_transfers_ownership` exactly.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "consumed by the class-ledger emitter wiring; test-pinned until the emitter toggle lands"
        )
    )]
    pub(crate) fn transfers_ownership(self) -> bool {
        !matches!(
            self,
            Self::LastReadBeforeScopeExit | Self::ScopeExit | Self::ApplyToBorrowedParam
        )
    }

    /// All 12 kinds, for exhaustiveness tests over the grid.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 12] = [
        Self::Return,
        Self::ConstructArg,
        Self::ReuseArg,
        Self::CollectionReuseArg,
        Self::SetValue,
        Self::PartialApplyCapture,
        Self::ApplyToOwnedParam,
        Self::JumpArg,
        Self::ApplyToIterConsumingParam,
        Self::LastReadBeforeScopeExit,
        Self::ScopeExit,
        Self::ApplyToBorrowedParam,
    ];
}

/// Class-origin attribution — WHERE a partition class's tracked reference
/// enters the function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ClassOrigin {
    /// A local allocation site (`Construct` / `Reuse` / `CollectionReuse` /
    /// `PartialApply`).
    Fresh,
    /// A callee-produced owned allocation (owned function param, or a call
    /// result whose contract proves an owned fresh return).
    Foreign,
    /// A borrowed function param — the caller retains ownership; the emitter
    /// treats borrowed-rooted classes as always-inc-at-consume.
    Borrowed,
    /// A call result with no contract knowledge — conservative unknown.
    Opaque,
    /// A refused phi/Select merge — the class starts at the merge point and
    /// is funded per-edge by cross-class jump-arg credits, never by a birth
    /// event (one predecessor edge executes per walk).
    Merge,
}

/// A class-resolved instruction event — the Rust mirror of the calculus's
/// `LedgerInstr` with variables resolved to partition-class representatives.
///
/// `Read` / `Mutate` carry the member variable so walk-level derivation can
/// compute the dynamic-COW live-sibling floor from the path suffix
/// (`sibReadCount`), exactly as the calculus computes it at derivation time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClassInstr {
    /// The class's tracked reference enters: +1 (except `Merge`, which is
    /// funded per-edge and never emits a birth).
    Birth { class: NodeIdx, origin: ClassOrigin },
    /// RL-1 duplication inc / RL-34 return-leg / sharing-view producer /
    /// cross-class jump-arg funding: +1.
    Credit { class: NodeIdx },
    /// Ownership hand-off out (a transfer terminal use) or a placed
    /// release: -1.
    Consume { class: NodeIdx },
    /// A borrow-view or terminal read: running count >= 1.
    Read { class: NodeIdx, value: ArcVarId },
    /// A COW-mutating use: running count >= 1 + live same-class siblings.
    Mutate { class: NodeIdx, value: ArcVarId },
}

/// Source position of one classified event inside its block — the
/// placement anchor the class-ledger emitter plans insertions against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventSite {
    /// Entry-block function-param births (before any body instruction).
    Params,
    /// The body instruction at this index.
    Body(usize),
    /// The block terminator.
    Terminator,
}

/// A derived per-class ledger event — mirror of the calculus `LedgerEvent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LedgerEvent {
    Birth,
    Credit,
    Consume,
    Read,
    Mutate { live_siblings: usize },
}

/// PV-4 boundary-contract projection for one callee — the contract facts
/// call-site classification consumes (`BoundaryContract.ofParamContract`).
#[derive(Clone, Debug, Default)]
pub(crate) struct BoundaryFacts {
    /// Per-param: the callee iter-consumes this argument (RL-2 inward
    /// transfer despite a Borrowed annotation).
    pub(crate) param_iter_consumes: Vec<bool>,
    /// Per-param: the callee transfers this argument through its return
    /// (RL-34 passthrough — consume at call, credit at return, net 0).
    pub(crate) param_transfers_through_return: Vec<bool>,
    /// The callee's return is a sharing view of an argument's allocation
    /// (seamless slice family): the result carries a CREDIT.
    pub(crate) returns_sharing_view: bool,
    /// The callee's return is a fresh owned allocation the caller now owns.
    pub(crate) returns_owned_fresh: bool,
}

impl BoundaryFacts {
    /// Project the classification-relevant facts out of a full
    /// `MemoryContract` (the production adapter; PV-4's
    /// `BoundaryContract.ofParamContract` composed per param).
    pub(crate) fn from_contract(contract: &MemoryContract) -> Self {
        Self {
            param_iter_consumes: contract.params.iter().map(|p| p.iter_consumes).collect(),
            param_transfers_through_return: contract
                .params
                .iter()
                .map(|p| p.transfers_through_return)
                .collect(),
            returns_sharing_view: contract.return_info.returns_sharing_view,
            returns_owned_fresh: contract.return_info.preserves_freshness,
        }
    }

    /// Whether argument `position` is an RL-2 iter-consume inward transfer:
    /// the callee iter-consumes it AND does not pass it back through the
    /// return.
    fn iter_consume_transfer(&self, position: usize) -> bool {
        self.param_iter_consumes
            .get(position)
            .copied()
            .unwrap_or(false)
            && !self
                .param_transfers_through_return
                .get(position)
                .copied()
                .unwrap_or(false)
    }
}

/// Classifier output: per-block class-instruction streams (block order mirrors
/// `func.blocks`) plus the per-class origin attribution map.
#[derive(Debug, Default)]
pub(crate) struct LedgerClassification {
    /// One ordered stream per block, indexed by block position.
    pub(crate) blocks: Vec<Vec<ClassInstr>>,
    /// Per-event source site, parallel to `blocks` (`sites[b][k]` locates
    /// `blocks[b][k]` within block `b`).
    pub(crate) sites: Vec<Vec<EventSite>>,
    /// Class representative -> origin kind.
    pub(crate) class_origins: FxHashMap<NodeIdx, ClassOrigin>,
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
    };
    classifier.record_merge_origins(func);
    for (block_idx, arc_block) in func.blocks.iter().enumerate() {
        let mut stream = Vec::new();
        let mut sites = Vec::new();
        if block_idx == func.entry.index() {
            classifier.classify_params(func, &mut stream);
            sites.resize(stream.len(), EventSite::Params);
        }
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
        reason = "consumed by the class-ledger emitter cutover; test-pinned until the differential harness lands"
    )
)]
pub(crate) fn derive_ledger(class: NodeIdx, instrs: &[ClassInstr]) -> Vec<LedgerEvent> {
    let mut events = Vec::new();
    for (position, instr) in instrs.iter().enumerate() {
        match *instr {
            ClassInstr::Birth { class: c, .. } if c == class => events.push(LedgerEvent::Birth),
            ClassInstr::Credit { class: c } if c == class => events.push(LedgerEvent::Credit),
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

/// The per-function classification walk state.
struct Classifier<'a> {
    state_map: &'a AimsStateMap,
    partition: &'a mut BirthSitePartition,
    boundary_facts: &'a FxHashMap<Name, BoundaryFacts>,
    out: LedgerClassification,
}

impl Classifier<'_> {
    /// The class representative of `var`'s whole-variable node.
    fn rep(&mut self, var: ArcVarId) -> NodeIdx {
        let node = self.partition.register_node(var, FieldPath::whole_var());
        self.partition.rep_of(node)
    }

    fn excluded(&self, var: ArcVarId) -> bool {
        self.state_map.is_excluded(var)
    }

    fn birth(&mut self, stream: &mut Vec<ClassInstr>, var: ArcVarId, origin: ClassOrigin) {
        let class = self.rep(var);
        stream.push(ClassInstr::Birth { class, origin });
        self.out.class_origins.insert(class, origin);
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

    fn classify_instr(&mut self, instr: &ArcInstr, stream: &mut Vec<ClassInstr>) {
        match instr {
            ArcInstr::Construct { dst, args, .. }
            | ArcInstr::Reuse { dst, args, .. }
            | ArcInstr::PartialApply { dst, args, .. } => {
                self.classify_fresh_alloc(stream, *dst, args);
            }
            ArcInstr::CollectionReuse {
                dst, old_var, args, ..
            } => {
                if !self.excluded(*old_var) {
                    self.consume(stream, *old_var);
                }
                self.classify_fresh_alloc(stream, *dst, args);
            }
            ArcInstr::Let { value, .. } => match value {
                // A whole-var alias is a partition edge, not an event.
                ArcValue::Var(_) | ArcValue::Literal(_) => {}
                ArcValue::PrimOp { args, .. } => {
                    for &arg in args {
                        if !self.excluded(arg) {
                            self.read(stream, arg);
                        }
                    }
                }
            },
            ArcInstr::Project { value, .. } => {
                // The view variable joins the field class via the partition;
                // the Project itself borrow-READS the source aggregate.
                if !self.excluded(*value) {
                    self.read(stream, *value);
                }
            }
            ArcInstr::Set { base, value, .. } => {
                self.mutate(stream, *base);
                if !self.excluded(*value) {
                    self.consume(stream, *value);
                }
            }
            ArcInstr::SetTag { base, .. } => {
                self.mutate(stream, *base);
            }
            ArcInstr::IsShared { var, .. } | ArcInstr::Reset { var, .. } => {
                if !self.excluded(*var) {
                    self.read(stream, *var);
                }
            }
            ArcInstr::Select {
                cond,
                true_val,
                false_val,
                ..
            } => {
                // The Select edge is EXCLUDED from partition admission; the
                // operands are conditional-alias READS and the dst carries
                // no birth (the selected allocation's obligation stays with
                // its source class).
                for &operand in &[*cond, *true_val, *false_val] {
                    if !self.excluded(operand) {
                        self.read(stream, operand);
                    }
                }
            }
            ArcInstr::Apply {
                dst,
                func,
                args,
                arg_ownership,
                ..
            } => {
                self.classify_call(stream, *dst, *func, args, arg_ownership, true);
            }
            ArcInstr::ApplyIndirect {
                dst, closure, args, ..
            } => {
                // Indirect callees have no contract: the closure receiver is
                // borrowed; args default Borrowed (conservative — the caller
                // retains cleanup responsibility); the result is OPAQUE.
                if !self.excluded(*closure) {
                    self.read(stream, *closure);
                }
                for &arg in args {
                    if !self.excluded(arg) {
                        self.read(stream, arg);
                    }
                }
                if !self.excluded(*dst) {
                    self.birth(stream, *dst, ClassOrigin::Opaque);
                }
            }
            ArcInstr::BurdenInc { .. }
            | ArcInstr::BurdenDec { .. }
            | ArcInstr::BurdenDecVariant { .. }
            | ArcInstr::BurdenDecPartial { .. }
            | ArcInstr::BurdenDecField { .. }
            | ArcInstr::RcInc { .. }
            | ArcInstr::RcDec { .. }
            | ArcInstr::RcDecPartial { .. }
            | ArcInstr::RcDecField { .. }
            | ArcInstr::RcDecVariant { .. } => {
                self.classify_placed_op(instr, stream);
            }
        }
    }

    /// Placed ops classify per the calculus: `BurdenInc` is the placed dup
    /// (CREDIT); every `BurdenDec` grain is a placed release (CONSUME) —
    /// field grains consume the field-path class. Realized RC ops are not
    /// uses (TF-11); the classifier runs before placement, and any realized
    /// ops in view belong to the legacy path the toggle keeps disjoint.
    fn classify_placed_op(&mut self, instr: &ArcInstr, stream: &mut Vec<ClassInstr>) {
        match instr {
            ArcInstr::BurdenInc { var } => {
                if !self.excluded(*var) {
                    let class = self.rep(*var);
                    stream.push(ClassInstr::Credit { class });
                }
            }
            ArcInstr::BurdenDec { var }
            | ArcInstr::BurdenDecVariant { var }
            | ArcInstr::BurdenDecPartial { var, .. } => {
                if !self.excluded(*var) {
                    self.consume(stream, *var);
                }
            }
            ArcInstr::BurdenDecField { base, field } => {
                if !self.excluded(*base) {
                    let node = self
                        .partition
                        .register_node(*base, FieldPath::single(*field));
                    let class = self.partition.rep_of(node);
                    stream.push(ClassInstr::Consume { class });
                }
            }
            _ => {}
        }
    }

    fn classify_terminator(
        &mut self,
        func: &ArcFunction,
        terminator: &ArcTerminator,
        stream: &mut Vec<ClassInstr>,
    ) {
        match terminator {
            ArcTerminator::Return { value } => {
                if !self.excluded(*value) {
                    self.consume(stream, *value);
                }
            }
            ArcTerminator::Jump { target, args } => {
                let Some(target_block) = func.blocks.get(target.index()) else {
                    return;
                };
                for (&arg, &(param, _)) in args.iter().zip(target_block.params.iter()) {
                    if self.excluded(arg) || self.excluded(param) {
                        continue;
                    }
                    let arg_class = self.rep(arg);
                    let param_class = self.rep(param);
                    if arg_class == param_class {
                        // RL-4 jump-arg exemption: the reference persists in
                        // the class — no event.
                        continue;
                    }
                    stream.push(ClassInstr::Consume { class: arg_class });
                    stream.push(ClassInstr::Credit { class: param_class });
                }
            }
            ArcTerminator::Branch { cond, .. } => {
                if !self.excluded(*cond) {
                    self.read(stream, *cond);
                }
            }
            ArcTerminator::Switch { scrutinee, .. } => {
                if !self.excluded(*scrutinee) {
                    self.read(stream, *scrutinee);
                }
            }
            ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                arg_ownership,
                ..
            } => {
                self.classify_call(stream, *dst, *callee, args, arg_ownership, true);
            }
            ArcTerminator::InvokeIndirect {
                dst, closure, args, ..
            } => {
                if !self.excluded(*closure) {
                    self.read(stream, *closure);
                }
                for &arg in args {
                    if !self.excluded(arg) {
                        self.read(stream, arg);
                    }
                }
                if !self.excluded(*dst) {
                    self.birth(stream, *dst, ClassOrigin::Opaque);
                }
            }
            ArcTerminator::Resume | ArcTerminator::Unreachable => {}
        }
    }

    /// Classify a direct call boundary through the callee contract (PV-4):
    /// per-arg CONSUME/READ by the iter-consume fact and the ownership
    /// annotation; the result CREDITS on a passthrough/sharing-view contract
    /// and BIRTHS (FOREIGN/OPAQUE) otherwise.
    fn classify_call(
        &mut self,
        stream: &mut Vec<ClassInstr>,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        arg_ownership: &[ArgOwnership],
        default_owned: bool,
    ) {
        let facts = self.boundary_facts.get(&callee);
        for (position, &arg) in args.iter().enumerate() {
            if self.excluded(arg) {
                continue;
            }
            let iter_consume = facts.is_some_and(|f| f.iter_consume_transfer(position));
            let owned = arg_ownership
                .get(position)
                .map_or(default_owned, |o| *o == ArgOwnership::Owned);
            if iter_consume || owned {
                self.consume(stream, arg);
            } else {
                self.read(stream, arg);
            }
        }
        if self.excluded(dst) {
            return;
        }
        let alias = self.state_map.apply_result_alias(dst);
        let unified_alias = match alias {
            Some(ApplyAliasSource::Direct(_) | ApplyAliasSource::Project { .. }) => true,
            Some(ApplyAliasSource::Conditional { candidates }) => {
                let dst_class = self.rep(dst);
                candidates.iter().any(|&candidate| {
                    let candidate_class = self.rep(candidate);
                    candidate_class == dst_class
                })
            }
            Some(ApplyAliasSource::Wrapped(_)) | None => false,
        };
        if unified_alias {
            // RL-34 passthrough return leg: the caller re-acquires the SAME
            // allocation — credit, not birth.
            let class = self.rep(dst);
            stream.push(ClassInstr::Credit { class });
        } else if facts.is_some_and(|f| f.returns_sharing_view) {
            let class = self.rep(dst);
            stream.push(ClassInstr::Credit { class });
        } else if facts.is_some_and(|f| f.returns_owned_fresh) {
            self.birth(stream, dst, ClassOrigin::Foreign);
        } else {
            self.birth(stream, dst, ClassOrigin::Opaque);
        }
    }
}

#[cfg(test)]
mod tests;
