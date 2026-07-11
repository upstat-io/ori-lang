//! Per-`ArcInstr` / `ArcTerminator` classification dispatch for
//! [`super::Classifier`].
//!
//! One match arm per instruction/terminator variant, routing each use to a
//! BIRTH / CREDIT / CONSUME / READ / MUTATE event via the shared primitives
//! on `Classifier` (`super::mod`).

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership};

use super::super::birth_site_partition::FieldPath;
use super::super::state_map::ApplyAliasSource;
use super::{ClassInstr, ClassOrigin, Classifier};

impl Classifier<'_> {
    pub(super) fn classify_instr(&mut self, instr: &ArcInstr, stream: &mut Vec<ClassInstr>) {
        match instr {
            ArcInstr::Construct { dst, args, .. } | ArcInstr::Reuse { dst, args, .. } => {
                self.classify_fresh_alloc(stream, *dst, args);
            }
            ArcInstr::PartialApply { dst, args, .. } => {
                // A capture of a BORROWED-ROOTED var hands a reference the
                // function never owned into the closure env (the curried
                // `a -> b -> a + b` shape) — an unmodeled ownership hand-off;
                // poison so the readiness gate falls back (fail-closed).
                if args
                    .iter()
                    .any(|arg| !self.excluded(*arg) && self.borrowed_rooted_vars.contains(arg))
                {
                    self.out.indirect_arg_handoff = true;
                }
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
            ArcInstr::Let { dst, value, .. } => {
                self.classify_let_value(stream, *dst, value);
            }
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
                dst,
                cond,
                true_val,
                false_val,
                ..
            } => {
                // The Select edge is EXCLUDED from partition admission; the
                // operands are conditional-alias READS and the dst carries
                // no birth (the selected allocation's obligation stays with
                // its source class). A non-excluded dst ACQUIRES the
                // selected reference: the planner realizes it with an RL-1
                // duplication inc after the select.
                for &operand in &[*cond, *true_val, *false_val] {
                    if !self.excluded(operand) {
                        self.read(stream, operand);
                    }
                }
                if !self.excluded(*dst) {
                    let class = self.rep(*dst);
                    stream.push(ClassInstr::SelectCredit { class, var: *dst });
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
                dst,
                closure,
                args,
                arg_ownership,
                ..
            } => {
                // Indirect callees have no contract: the closure receiver is
                // borrowed (ABI pos 0); the result is OPAQUE. A POPULATED
                // `arg_ownership` (the Step-4b prelude runs
                // `emit_arg_ownership` before classification) resolves each
                // hand-off — the same annotation source direct no-contract
                // calls classify by. An UNANNOTATED non-excluded HEAP arg
                // stays an unmodeled hand-off: read it for floor honesty AND
                // poison the classification so readiness falls back.
                if !self.excluded(*closure) {
                    self.read(stream, *closure);
                }
                self.classify_indirect_args(stream, args, arg_ownership);
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

    /// Classify a `Let` by its value kind. A whole-var alias is a partition
    /// edge, not an event. A non-excluded STRING literal (a non-empty
    /// string; the empty string is immortal-excluded) is a fresh allocation
    /// — the TF-3 analog with zero funded args; every other literal kind
    /// allocates nothing at runtime even under a heap-repr variable (the
    /// iterator-protocol placeholder `%n: str = 0` shape), so it stays
    /// event-less. A HEAP-producing `PrimOp` (str/list concat) is a fresh
    /// allocation whose operand ownership splits by repr, mirroring the
    /// runtime contract's consumed-operand discriminator: an `RcPointer`
    /// operand transfers into the
    /// dual-consuming `ori_list_concat_cow` (CONSUME); a `FatValue` str
    /// operand is BORROWED by `ori_str_concat` — the operand's class READs
    /// and keeps its own owed release with the planner (the caller-side
    /// dec). A SCALAR-dst `PrimOp` (comparison) borrow-READS its heap
    /// operands.
    fn classify_let_value(
        &mut self,
        stream: &mut Vec<ClassInstr>,
        dst: ArcVarId,
        value: &ArcValue,
    ) {
        match value {
            ArcValue::Var(_) => {}
            ArcValue::Literal(lit) => {
                if matches!(lit, crate::ir::LitValue::String(_)) && !self.excluded(dst) {
                    self.birth(stream, dst, ClassOrigin::Fresh);
                }
            }
            ArcValue::PrimOp { args, .. } => {
                if self.excluded(dst) {
                    for &arg in args {
                        if !self.excluded(arg) {
                            self.read(stream, arg);
                        }
                    }
                } else {
                    if !self.excluded(dst) {
                        self.birth(stream, dst, ClassOrigin::Fresh);
                    }
                    for &arg in args {
                        if self.excluded(arg) {
                            continue;
                        }
                        if matches!(
                            self.func.var_repr(arg),
                            Some(crate::ir::ValueRepr::RcPointer)
                        ) {
                            // `ori_list_concat_cow` consumes BOTH operands
                            // on every strategy row (reuse / copy / takeover
                            // all release or absorb the operand reference);
                            // COW uniqueness selects the internal strategy,
                            // never the external refcount contract. A
                            // borrowed-param-rooted operand is externally
                            // net-0: the operator lowering emits its own
                            // `borrow_protect.inc` whose undo is the concat's
                            // dec, so the caller-retained reference survives
                            // — the class READs (a ledger funding inc would
                            // double-fund it).
                            if self.borrowed_rooted_vars.contains(&arg) {
                                self.read(stream, arg);
                            } else {
                                self.consume(stream, arg);
                            }
                        } else {
                            self.read(stream, arg);
                        }
                    }
                }
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

    pub(super) fn classify_terminator(
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
                    if self.excluded(param) {
                        continue;
                    }
                    if self.excluded(arg) {
                        // An excluded hand-off (an immortal seed) still
                        // CREDITS the param class: the slot holds a
                        // reference whose eventual dec is a runtime no-op
                        // on the immortal, so every entry edge funds the
                        // param uniformly.
                        let param_class = self.rep(param);
                        stream.push(ClassInstr::Credit { class: param_class });
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
            // Invoke/InvokeIndirect RESULT events are pre-recorded at the
            // NORMAL successor's entry (`record_invoke_result_entries`);
            // only the argument uses classify at the terminator (the
            // hand-off happens on BOTH paths — the callee owns consumed
            // args even when it unwinds).
            ArcTerminator::Invoke {
                func: callee,
                args,
                arg_ownership,
                ..
            } => {
                self.classify_call_args(stream, Some(*callee), args, arg_ownership, true);
            }
            ArcTerminator::InvokeIndirect {
                closure,
                args,
                arg_ownership,
                ..
            } => {
                if !self.excluded(*closure) {
                    self.read(stream, *closure);
                }
                // Same annotated-vs-unmodeled split as the body
                // ApplyIndirect arm.
                self.classify_indirect_args(stream, args, arg_ownership);
            }
            ArcTerminator::Resume | ArcTerminator::Unreachable => {}
        }
    }

    /// Classify a direct call boundary through the callee contract (PV-4):
    /// per-arg CONSUME/READ by the iter-consume fact and the ownership
    /// annotation; the result event pushed inline (the `Apply` body-site
    /// path — an `Invoke` result routes to its normal successor's entry via
    /// `record_invoke_result_entries` instead).
    fn classify_call(
        &mut self,
        stream: &mut Vec<ClassInstr>,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        arg_ownership: &[ArgOwnership],
        default_owned: bool,
    ) {
        self.classify_call_args(stream, Some(callee), args, arg_ownership, default_owned);
        if let Some(event) = self.call_result_event(dst, Some(callee), args.first().copied()) {
            stream.push(event);
        }
    }

    /// Per-arg classification at an INDIRECT call boundary. Annotated args
    /// (`arg_ownership` parallel to `args`, populated by the Step-4b
    /// `emit_arg_ownership` prelude) classify CONSUME/READ per annotation
    /// with an all-Borrowed default; unannotated non-excluded heap args
    /// read for floor honesty and poison the classification.
    fn classify_indirect_args(
        &mut self,
        stream: &mut Vec<ClassInstr>,
        args: &[ArcVarId],
        arg_ownership: &[ArgOwnership],
    ) {
        if arg_ownership.len() == args.len() {
            self.classify_call_args(stream, None, args, arg_ownership, false);
            return;
        }
        for &arg in args {
            if !self.excluded(arg) {
                self.read(stream, arg);
                self.out.indirect_arg_handoff = true;
            }
        }
    }

    /// Per-arg CONSUME/READ classification at a direct call boundary.
    pub(super) fn classify_call_args(
        &mut self,
        stream: &mut Vec<ClassInstr>,
        callee: Option<Name>,
        args: &[ArcVarId],
        arg_ownership: &[ArgOwnership],
        default_owned: bool,
    ) {
        let facts = callee.and_then(|name| self.boundary_facts.get(&name));
        let iterator_creation = callee == Some(self.iter_name);
        for (position, &arg) in args.iter().enumerate() {
            if self.excluded(arg) {
                continue;
            }
            let iter_consume = facts.is_some_and(|f| f.iter_consume_transfer(position));
            let owned = arg_ownership
                .get(position)
                .map_or(default_owned, |o| *o == ArgOwnership::Owned);
            // An `@iter [own]` hand-off of a borrowed-rooted var creates a
            // NON-owning iterator (the emitter skips the source dec for a
            // borrowed-rooted receiver; the owner/caller releases it per
            // `RL2_borrowed_param_emits_caller_dec`) — a READ, never the
            // transfer the owned annotation would otherwise book.
            if iterator_creation && self.borrowed_rooted_vars.contains(&arg) {
                self.read(stream, arg);
                continue;
            }
            if iter_consume || owned {
                self.consume(stream, arg);
            } else {
                self.read(stream, arg);
            }
        }
    }

    /// The call RESULT event (PV-4): CREDIT on a passthrough/sharing-view
    /// contract, BIRTH (FOREIGN/OPAQUE) otherwise; `None` for an excluded
    /// destination. `callee = None` (indirect call) has no contract and
    /// births OPAQUE.
    pub(super) fn call_result_event(
        &mut self,
        dst: ArcVarId,
        callee: Option<Name>,
        receiver: Option<ArcVarId>,
    ) -> Option<ClassInstr> {
        if self.excluded(dst) {
            return None;
        }
        let facts = callee.and_then(|name| self.boundary_facts.get(&name));
        let returns_sharing_view = facts.is_some_and(|f| f.returns_sharing_view);
        let returns_owned_fresh = facts.is_some_and(|f| f.returns_owned_fresh);
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
        // Retain-less borrow-view accessor: the runtime returns an INTERIOR
        // allocation's pointer with NO reference minted — the receiver's
        // aggregate keeps it alive and its own recursive release frees it.
        // The result books a BORROWED-origin arrival (the borrowed-param
        // discipline): owed 0 at entry, reads floor 0 (the external owner
        // covers them), and every consume hand-off needs its own funded
        // duplication inc (`plan_incs` borrowed-rooted rule) — an Opaque
        // birth here would plan a release that frees the interior
        // allocation out from under the still-live receiver.
        if callee.is_some_and(|name| self.borrow_view_accessors.contains(&name)) {
            let _ = receiver;
            return Some(self.birth_instr(dst, ClassOrigin::Borrowed));
        }
        if unified_alias || returns_sharing_view {
            // RL-34 passthrough return leg / sharing-view producer: the
            // caller re-acquires the SAME allocation — credit, not birth.
            let class = self.rep(dst);
            Some(ClassInstr::Credit { class })
        } else if returns_owned_fresh {
            Some(self.birth_instr(dst, ClassOrigin::Foreign))
        } else {
            Some(self.birth_instr(dst, ClassOrigin::Opaque))
        }
    }
}
