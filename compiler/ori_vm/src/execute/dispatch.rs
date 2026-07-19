//! Verified-opcode dispatch over selectable frame storage.

mod adapters;

use crate::bytecode::{Continuation, Op, Pc, RcSemantics, Register};
use crate::ExecutionError;

use super::calls::CallDispatch;
use super::operands::{DispatchLayout, OperandAccess};
use super::profile::{DispatchProbe, DispatchSite};
use super::value::VmValue;
use super::Interpreter;

#[derive(Clone, Copy)]
struct ClosureCallOperands {
    destination: Register,
    closure: Register,
    arguments: usize,
    normal: Continuation,
    unwind: Option<Pc>,
}

impl Interpreter<'_> {
    #[inline]
    pub(super) fn step<'plan, P, L>(
        &mut self,
        probe: &mut P,
        layout: L,
    ) -> Result<Option<VmValue>, ExecutionError>
    where
        P: DispatchProbe,
        L: DispatchLayout<'plan>,
    {
        let frame = self.depth - 1;
        let function = self.frames[frame].function;
        let pc = self.frames[frame].pc;
        let operation = self.current_operation(frame);
        let mut operands = layout.operands_at(function, pc);
        if P::ENABLED {
            probe.dispatched(DispatchSite {
                frame,
                function,
                pc,
                operation,
            });
        }
        let result = self.dispatch_operation(frame, operation, &mut operands);
        if result.is_ok() {
            operands.finish();
        }
        result
    }

    fn dispatch_operation(
        &mut self,
        frame: usize,
        operation: Op,
        operands: &mut impl OperandAccess,
    ) -> Result<Option<VmValue>, ExecutionError> {
        match operation {
            Op::Const { dst, value } => self
                .dispatch_const(frame, dst, value, operands)
                .map(|()| None),
            Op::Copy { dst, src } => {
                self.dispatch_copy(frame, dst, src, operands);
                Ok(None)
            }
            Op::Binary { dst, op, lhs, rhs } => self
                .dispatch_binary(frame, dst, op, [lhs, rhs], operands)
                .map(|()| None),
            Op::IntBinary { dst, op, lhs, rhs } => self
                .dispatch_int_binary(frame, dst, op, [lhs, rhs], operands)
                .map(|()| None),
            Op::StringBinary { dst, op, lhs, rhs } => self
                .dispatch_string_binary(frame, dst, op, [lhs, rhs], operands)
                .map(|()| None),
            Op::RuntimeBinary {
                dst,
                operator,
                lhs,
                rhs,
            } => self
                .dispatch_runtime_binary(frame, dst, operator, [lhs, rhs], operands)
                .map(|()| None),
            Op::Unary { dst, op, arg } => self
                .dispatch_unary(frame, dst, op, arg, operands)
                .map(|()| None),
            Op::BoolNot { dst, arg } => self
                .dispatch_bool_not(frame, dst, arg, operands)
                .map(|()| None),
            call @ (Op::Call { .. } | Op::MakeClosure { .. } | Op::CallClosure { .. }) => {
                self.dispatch_call_operation(frame, call, operands)
            }
            Op::Construct { dst, ctor, args } => self
                .dispatch_construct(frame, dst, ctor, args.index(), operands)
                .map(|()| None),
            Op::Project { dst, value, field } => self
                .dispatch_project(frame, dst, value, field, operands)
                .map(|()| None),
            Op::RcInc {
                var,
                count,
                semantics,
            } => self
                .dispatch_rc_inc(frame, var, count, semantics, operands)
                .map(|()| None),
            Op::RcDec { var, semantics } => self
                .dispatch_rc_dec(frame, var, semantics, operands)
                .map(|()| None),
            Op::IsShared { dst, var } => self
                .dispatch_is_shared(frame, dst, var, operands)
                .map(|()| None),
            Op::Set { base, field, value } => self
                .dispatch_set(frame, base, field, value, operands)
                .map(|()| None),
            Op::SetTag { base, tag } => self
                .dispatch_set_tag(frame, base, tag, operands)
                .map(|()| None),
            Op::Select {
                dst,
                cond,
                true_value,
                false_value,
            } => self
                .dispatch_select(frame, dst, cond, [true_value, false_value], operands)
                .map(|()| None),
            control @ (Op::Jump { .. }
            | Op::Branch { .. }
            | Op::Switch { .. }
            | Op::Return { .. }
            | Op::Resume
            | Op::Unreachable) => self.dispatch_control_operation(frame, control, operands),
        }
    }

    fn dispatch_call_operation(
        &mut self,
        frame: usize,
        operation: Op,
        operands: &mut impl OperandAccess,
    ) -> Result<Option<VmValue>, ExecutionError> {
        match operation {
            Op::Call {
                dst,
                callee,
                args,
                normal,
                unwind,
            } => self
                .dispatch_call(
                    frame,
                    CallDispatch {
                        destination: operands.write(dst),
                        callee,
                        operands: args.index(),
                        normal,
                        unwind,
                    },
                    operands,
                )
                .map(|()| None),
            Op::MakeClosure {
                dst,
                callee,
                captures,
            } => self
                .dispatch_make_closure(frame, dst, callee, captures.index(), operands)
                .map(|()| None),
            Op::CallClosure {
                dst,
                closure,
                args,
                normal,
                unwind,
            } => self
                .dispatch_call_closure(
                    frame,
                    ClosureCallOperands {
                        destination: dst,
                        closure,
                        arguments: args.index(),
                        normal,
                        unwind,
                    },
                    operands,
                )
                .map(|()| None),
            unexpected @ (Op::Const { .. }
            | Op::Copy { .. }
            | Op::Binary { .. }
            | Op::IntBinary { .. }
            | Op::StringBinary { .. }
            | Op::RuntimeBinary { .. }
            | Op::Unary { .. }
            | Op::BoolNot { .. }
            | Op::Construct { .. }
            | Op::Project { .. }
            | Op::RcInc { .. }
            | Op::RcDec { .. }
            | Op::IsShared { .. }
            | Op::Set { .. }
            | Op::SetTag { .. }
            | Op::Select { .. }
            | Op::Jump { .. }
            | Op::Branch { .. }
            | Op::Switch { .. }
            | Op::Return { .. }
            | Op::Resume
            | Op::Unreachable) => {
                unreachable!("call dispatcher received a non-call operation: {unexpected:?}")
            }
        }
    }

    fn dispatch_control_operation(
        &mut self,
        frame: usize,
        operation: Op,
        operands: &mut impl OperandAccess,
    ) -> Result<Option<VmValue>, ExecutionError> {
        match operation {
            Op::Jump { target, moves } => {
                self.dispatch_jump(frame, target, moves.index(), operands);
                Ok(None)
            }
            Op::Branch {
                cond,
                then_pc,
                else_pc,
            } => self
                .dispatch_branch(frame, cond, then_pc, else_pc, operands)
                .map(|()| None),
            Op::Switch {
                scrutinee,
                table,
                default_pc,
            } => self
                .dispatch_switch(frame, scrutinee, table.index(), default_pc, operands)
                .map(|()| None),
            Op::Return { value } => self.dispatch_return(frame, value, operands),
            Op::Resume => self.resume_panic(frame).map(|()| None),
            Op::Unreachable => Err(ExecutionError::ReachedUnreachable),
            unexpected @ (Op::Const { .. }
            | Op::Copy { .. }
            | Op::Binary { .. }
            | Op::IntBinary { .. }
            | Op::StringBinary { .. }
            | Op::RuntimeBinary { .. }
            | Op::Unary { .. }
            | Op::BoolNot { .. }
            | Op::Call { .. }
            | Op::MakeClosure { .. }
            | Op::CallClosure { .. }
            | Op::Construct { .. }
            | Op::Project { .. }
            | Op::RcInc { .. }
            | Op::RcDec { .. }
            | Op::IsShared { .. }
            | Op::Set { .. }
            | Op::SetTag { .. }
            | Op::Select { .. }) => {
                unreachable!("control dispatcher received a non-control operation: {unexpected:?}")
            }
        }
    }

    #[inline]
    fn current_operation(&self, frame: usize) -> Op {
        let frame = &self.frames[frame];
        self.program.functions[frame.function.index()].ops[frame.pc.index()]
    }

    fn rc_strategy(semantics: RcSemantics) -> ori_arc::RcStrategy {
        semantics.strategy
    }
}
