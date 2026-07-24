//! Register and side-table adapters for shared opcode semantics.

use crate::bytecode::{Constant, Pc, RcSemantics, Register};
use crate::ExecutionError;

use super::super::calls::{CallDispatch, ClosureCallDispatch};
use super::super::operands::{FrameSlot, OperandAccess};
use super::super::value::VmValue;
use super::super::{primitives, Interpreter};
use super::ClosureCallOperands;

impl Interpreter<'_> {
    pub(super) fn dispatch_const(
        &mut self,
        frame: usize,
        destination: Register,
        constant: Constant,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let value = Self::constant(constant)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_copy(
        &mut self,
        frame: usize,
        destination: Register,
        source: Register,
        operands: &mut impl OperandAccess,
    ) {
        let destination = operands.write(destination);
        let value = operands.read(&self.frames[frame].registers, source);
        if !operands.copy_is_noop() {
            self.set_slot(frame, destination, value);
        }
        self.advance(frame);
    }

    pub(super) fn dispatch_binary(
        &mut self,
        frame: usize,
        destination: Register,
        operation: ori_ir::BinaryOp,
        inputs: [Register; 2],
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let [left, right] = self.read_pair(frame, inputs, operands);
        let value = self.execute_binary(operation, left, right)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_int_binary(
        &mut self,
        frame: usize,
        destination: Register,
        operation: crate::bytecode::IntBinaryOp,
        inputs: [Register; 2],
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let [left, right] = self.read_pair(frame, inputs, operands);
        let value = primitives::int_binary(operation, left, right)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_string_binary(
        &mut self,
        frame: usize,
        destination: Register,
        operation: crate::bytecode::StringBinaryOp,
        inputs: [Register; 2],
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let [left, right] = self.read_pair(frame, inputs, operands);
        let value = self.string_binary(operation, left, right)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_runtime_binary(
        &mut self,
        frame: usize,
        destination: Register,
        operator: ori_registry::RuntimeOperator,
        inputs: [Register; 2],
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let [left, right] = self.read_pair(frame, inputs, operands);
        let value = self.runtime_binary(operator, left, right)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_unary(
        &mut self,
        frame: usize,
        destination: Register,
        operation: ori_ir::UnaryOp,
        argument: Register,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let argument = operands.read(&self.frames[frame].registers, argument);
        let value = primitives::unary(operation, argument)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_bool_not(
        &mut self,
        frame: usize,
        destination: Register,
        argument: Register,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let argument = operands.read(&self.frames[frame].registers, argument);
        let value = VmValue::bool(!argument.as_bool()?);
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_call(
        &mut self,
        frame: usize,
        call: CallDispatch,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        self.call(frame, call, operands)
    }

    pub(super) fn dispatch_make_closure(
        &mut self,
        frame: usize,
        destination: Register,
        callee: ori_repr::executable::FunctionId,
        captures: usize,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let closure = self.make_closure(frame, callee, captures, operands)?;
        self.store_and_advance(frame, destination, closure);
        Ok(())
    }

    pub(super) fn dispatch_call_closure(
        &mut self,
        frame: usize,
        call: ClosureCallOperands,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let ClosureCallOperands {
            destination,
            closure,
            arguments,
            normal,
            unwind,
        } = call;
        let destination = operands.write(destination);
        let closure = operands.read(&self.frames[frame].registers, closure);
        self.call_closure(
            frame,
            ClosureCallDispatch {
                destination,
                closure,
                operands: arguments,
                normal,
                unwind,
            },
            operands,
        )
    }

    pub(super) fn dispatch_construct(
        &mut self,
        frame: usize,
        destination: Register,
        constructor: ori_arc::CtorKind,
        operand_list: usize,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let value = self.construct(frame, destination, constructor, operand_list, operands)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_project(
        &mut self,
        frame: usize,
        destination: Register,
        source: Register,
        field: u32,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let source = operands.read(&self.frames[frame].registers, source);
        let value = self.project(source, field)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    pub(super) fn dispatch_rc_inc(
        &mut self,
        frame: usize,
        source: Register,
        count: u32,
        semantics: RcSemantics,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let value = operands.read(&self.frames[frame].registers, source);
        self.retain_with_strategy(value, count, Self::rc_strategy(semantics))?;
        self.advance(frame);
        Ok(())
    }

    pub(super) fn dispatch_rc_dec(
        &mut self,
        frame: usize,
        source: Register,
        semantics: RcSemantics,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let value = operands.read(&self.frames[frame].registers, source);
        self.release_with_strategy(value, Self::rc_strategy(semantics))?;
        self.advance(frame);
        Ok(())
    }

    pub(super) fn dispatch_is_shared(
        &mut self,
        frame: usize,
        destination: Register,
        source: Register,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let value = operands.read(&self.frames[frame].registers, source);
        let shared = self.heap.is_shared(value)?;
        self.store_and_advance(frame, destination, VmValue::bool(shared));
        Ok(())
    }

    pub(super) fn dispatch_set(
        &mut self,
        frame: usize,
        base: Register,
        field: u32,
        value: Register,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let base = operands.read(&self.frames[frame].registers, base);
        let value = operands.read(&self.frames[frame].registers, value);
        self.set_field(base, field, value)?;
        self.advance(frame);
        Ok(())
    }

    pub(super) fn dispatch_set_tag(
        &mut self,
        frame: usize,
        base: Register,
        tag: u64,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let base = operands.read(&self.frames[frame].registers, base);
        self.set_tag(base, tag)?;
        self.advance(frame);
        Ok(())
    }

    pub(super) fn dispatch_select(
        &mut self,
        frame: usize,
        destination: Register,
        condition: Register,
        choices: [Register; 2],
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let destination = operands.write(destination);
        let condition = operands.read(&self.frames[frame].registers, condition);
        let [true_value, false_value] = self.read_pair(frame, choices, operands);
        let selected = if condition.as_bool()? {
            true_value
        } else {
            false_value
        };
        self.store_and_advance(frame, destination, selected);
        Ok(())
    }

    pub(super) fn dispatch_jump(
        &mut self,
        frame: usize,
        target: Pc,
        moves: usize,
        operands: &mut impl OperandAccess,
    ) {
        self.execute_moves(frame, moves, operands);
        self.frames[frame].pc = target;
    }

    pub(super) fn dispatch_branch(
        &mut self,
        frame: usize,
        condition: Register,
        then_pc: Pc,
        else_pc: Pc,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let condition = operands.read(&self.frames[frame].registers, condition);
        self.frames[frame].pc = if condition.as_bool()? {
            then_pc
        } else {
            else_pc
        };
        Ok(())
    }

    pub(super) fn dispatch_switch(
        &mut self,
        frame: usize,
        scrutinee: Register,
        table: usize,
        default: Pc,
        operands: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let scrutinee = operands.read(&self.frames[frame].registers, scrutinee);
        let key = self.discriminant(scrutinee)?;
        self.frames[frame].pc = self
            .switch_table(table)
            .iter()
            .find_map(|(value, target)| (*value == key).then_some(*target))
            .unwrap_or(default);
        Ok(())
    }

    pub(super) fn dispatch_return(
        &mut self,
        frame: usize,
        source: Register,
        operands: &mut impl OperandAccess,
    ) -> Result<Option<VmValue>, ExecutionError> {
        let value = operands.read(&self.frames[frame].registers, source);
        self.return_from(frame, value)
    }

    fn read_pair(
        &self,
        frame: usize,
        registers: [Register; 2],
        operands: &mut impl OperandAccess,
    ) -> [VmValue; 2] {
        registers.map(|register| operands.read(&self.frames[frame].registers, register))
    }

    fn store_and_advance(&mut self, frame: usize, destination: FrameSlot, value: VmValue) {
        self.set_slot(frame, destination, value);
        self.advance(frame);
    }

    fn constant(constant: Constant) -> Result<VmValue, ExecutionError> {
        match constant {
            Constant::Int(value) => Ok(VmValue::int(value)),
            Constant::Float(bits) => Ok(VmValue::float(bits)),
            Constant::Bool(value) => Ok(VmValue::bool(value)),
            Constant::String(value) => VmValue::constant_string(value.index()),
            Constant::Char(value) => Ok(VmValue::char(value)),
            Constant::Unit => Ok(VmValue::UNIT),
            Constant::Null => Ok(VmValue::null()),
        }
    }
}
