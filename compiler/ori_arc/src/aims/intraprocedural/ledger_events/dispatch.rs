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
