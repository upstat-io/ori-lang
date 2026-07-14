//! Reusable interpreter frames and frame-local objects.

use ori_repr::executable::FunctionId;

use crate::bytecode::{BytecodeFunction, Pc, Register};

use super::value::VmValue;

#[derive(Clone, Copy, Debug)]
pub(super) struct Aggregate {
    pub(super) fields: [VmValue; 4],
    pub(super) length: u8,
    pub(super) variant: u64,
}

impl Default for Aggregate {
    fn default() -> Self {
        Self {
            fields: [VmValue::UNIT; 4],
            length: 0,
            variant: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct IteratorState {
    pub(super) current: i64,
    pub(super) end: i64,
    pub(super) step: i64,
    pub(super) inclusive: bool,
    pub(super) live: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ReturnTo {
    pub(super) destination: Register,
    pub(super) normal: Pc,
    pub(super) unwind: Option<Pc>,
}

pub(super) struct Frame {
    pub(super) function: FunctionId,
    pub(super) pc: Pc,
    pub(super) registers: Vec<VmValue>,
    pub(super) aggregates: Vec<Aggregate>,
    pub(super) iterators: Vec<IteratorState>,
    pub(super) move_scratch: Vec<VmValue>,
    pub(super) return_to: Option<ReturnTo>,
}

impl Frame {
    pub(super) fn new(
        function: FunctionId,
        bytecode: &BytecodeFunction,
        return_to: Option<ReturnTo>,
    ) -> Self {
        Self::new_layout(function, bytecode.entry, bytecode.register_count, return_to)
    }

    pub(super) fn new_layout(
        function: FunctionId,
        entry: Pc,
        register_count: usize,
        return_to: Option<ReturnTo>,
    ) -> Self {
        Self {
            function,
            pc: entry,
            registers: vec![VmValue::UNIT; register_count],
            aggregates: vec![Aggregate::default(); register_count],
            iterators: vec![IteratorState::default(); register_count],
            move_scratch: Vec::new(),
            return_to,
        }
    }

    pub(super) fn reset_layout(
        &mut self,
        function: FunctionId,
        entry: Pc,
        register_count: usize,
        return_to: Option<ReturnTo>,
    ) {
        self.function = function;
        self.pc = entry;
        self.return_to = return_to;
        self.registers.resize(register_count, VmValue::UNIT);
        self.registers.truncate(register_count);
        self.registers.fill(VmValue::UNIT);
        self.aggregates.resize(register_count, Aggregate::default());
        self.aggregates.truncate(register_count);
        self.aggregates.fill(Aggregate::default());
        self.iterators
            .resize(register_count, IteratorState::default());
        self.iterators.truncate(register_count);
        self.iterators.fill(IteratorState::default());
        self.move_scratch.clear();
    }
}
