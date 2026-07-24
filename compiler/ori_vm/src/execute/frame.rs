//! Reusable interpreter call frames.

use ori_repr::executable::FunctionId;

use crate::bytecode::Pc;

use super::operands::FrameSlot;
use super::value::VmValue;

#[derive(Clone, Copy, Debug)]
pub(super) struct ReturnTo {
    pub(super) destination: FrameSlot,
    pub(super) normal: Pc,
    pub(super) unwind: Option<Pc>,
}

pub(super) struct Frame {
    pub(super) function: FunctionId,
    pub(super) pc: Pc,
    pub(super) registers: Vec<VmValue>,
    pub(super) move_scratch: Vec<VmValue>,
    pub(super) return_to: Option<ReturnTo>,
}

impl Frame {
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
        self.move_scratch.clear();
    }
}
