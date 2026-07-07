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

    /// Classify a `Let` by its value kind. A whole-var alias is a partition
    /// edge, not an event. A non-excluded STRING literal (a non-empty
    /// string; the empty string is immortal-excluded) is a fresh allocation
    /// — the TF-3 analog with zero funded args; every other literal kind
    /// allocates nothing at runtime even under a heap-repr variable (the
    /// iterator-protocol placeholder `%n: str = 0` shape), so it stays
    /// event-less. A HEAP-producing `PrimOp` (str/list concat) is a fresh
    /// allocation whose operands transfer in (the runtime concat frees or
    /// reuses both inputs — `ConstructArg` kind; the legacy path emits no
    /// operand dec); a SCALAR-dst `PrimOp` (comparison) borrow-READS its heap
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
                    self.classify_fresh_alloc(stream, dst, args);
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
            ArcTerminator::InvokeIndirect { closure, args, .. } => {
                if !self.excluded(*closure) {
                    self.read(stream, *closure);
                }
                for &arg in args {
                    if !self.excluded(arg) {
                        self.read(stream, arg);
                    }
                }
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
        if let Some(event) = self.call_result_event(dst, Some(callee)) {
            stream.push(event);
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
    }

    /// The call RESULT event (PV-4): CREDIT on a passthrough/sharing-view
    /// contract, BIRTH (FOREIGN/OPAQUE) otherwise; `None` for an excluded
    /// destination. `callee = None` (indirect call) has no contract and
    /// births OPAQUE.
    pub(super) fn call_result_event(
        &mut self,
        dst: ArcVarId,
        callee: Option<Name>,
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
