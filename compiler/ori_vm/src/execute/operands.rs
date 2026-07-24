//! Storage selection beneath shared opcode semantics.

use ori_repr::executable::FunctionId;

use crate::bytecode::{Pc, Register};
use crate::physical::{PhysicalExecutionView, PhysicalImmediate, PhysicalPcView, PhysicalRead};

use super::value::VmValue;

/// One frame slot selected by the active canonical or physical layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FrameSlot(usize);

impl FrameSlot {
    pub(super) const fn index(self) -> usize {
        self.0
    }
}

impl From<Register> for FrameSlot {
    fn from(register: Register) -> Self {
        Self(register.index())
    }
}

/// Storage policy for one interpreter session.
#[derive(Clone, Copy)]
pub(super) enum ExecutionLayout<'plan> {
    Canonical,
    Physical(PhysicalExecutionView<'plan>),
}

impl ExecutionLayout<'_> {
    pub(super) fn frame_slot(self, function: FunctionId, register: Register) -> FrameSlot {
        match self {
            Self::Canonical => FrameSlot(register.index()),
            Self::Physical(plan) => FrameSlot(
                plan.function_at(function.index())
                    .lane_at(register.index())
                    .index(),
            ),
        }
    }

    pub(super) fn frame_slot_count(
        self,
        function: FunctionId,
        canonical_registers: usize,
    ) -> usize {
        match self {
            Self::Canonical => canonical_registers,
            Self::Physical(plan) => plan.function_at(function.index()).lane_count(),
        }
    }
}

/// Storage-specialized operand access beneath shared opcode semantics.
pub(super) trait OperandAccess {
    fn read(&mut self, registers: &[VmValue], canonical: Register) -> VmValue;

    fn write(&mut self, canonical: Register) -> FrameSlot;

    fn copy_is_noop(&self) -> bool;

    fn finish(&self);
}

/// Layout selected once for a complete interpreter session.
pub(super) trait DispatchLayout<'plan>: Copy {
    type Operands: OperandAccess;

    fn operands_at(self, function: FunctionId, pc: Pc) -> Self::Operands;
}

/// Canonical register bindings for one dispatch.
pub(super) struct CanonicalOperands {
    next_read: usize,
    next_write: usize,
}

impl CanonicalOperands {
    const fn new() -> Self {
        Self {
            next_read: 0,
            next_write: 0,
        }
    }
}

impl OperandAccess for CanonicalOperands {
    #[inline]
    fn read(&mut self, registers: &[VmValue], canonical: Register) -> VmValue {
        self.next_read += 1;
        registers[canonical.index()]
    }

    #[inline]
    fn write(&mut self, canonical: Register) -> FrameSlot {
        self.next_write += 1;
        FrameSlot(canonical.index())
    }

    #[inline]
    fn copy_is_noop(&self) -> bool {
        false
    }

    #[inline]
    fn finish(&self) {}
}

/// Physical lane and immediate bindings for one dispatch.
pub(super) struct PhysicalOperands<'plan> {
    physical: PhysicalPcView<'plan>,
    next_read: usize,
    next_write: usize,
}

impl<'plan> PhysicalOperands<'plan> {
    const fn new(physical: PhysicalPcView<'plan>) -> Self {
        Self {
            physical,
            next_read: 0,
            next_write: 0,
        }
    }
}

impl OperandAccess for PhysicalOperands<'_> {
    #[inline]
    fn read(&mut self, registers: &[VmValue], _canonical: Register) -> VmValue {
        let value = match self.physical.reads()[self.next_read] {
            PhysicalRead::Lane(lane) => registers[lane.index()],
            PhysicalRead::Immediate(PhysicalImmediate::Int(value)) => VmValue::int(value),
            PhysicalRead::Immediate(PhysicalImmediate::Bool(value)) => VmValue::bool(value),
        };
        self.next_read += 1;
        value
    }

    #[inline]
    fn write(&mut self, _canonical: Register) -> FrameSlot {
        let slot = FrameSlot(self.physical.writes()[self.next_write].index());
        self.next_write += 1;
        slot
    }

    #[inline]
    fn copy_is_noop(&self) -> bool {
        self.physical.copy_is_noop()
    }

    #[inline]
    fn finish(&self) {
        debug_assert_eq!(self.next_read, self.physical.reads().len());
        debug_assert_eq!(self.next_write, self.physical.writes().len());
    }
}

/// Zero-sized canonical dispatch policy.
#[derive(Clone, Copy)]
pub(super) struct CanonicalDispatch;

impl DispatchLayout<'_> for CanonicalDispatch {
    type Operands = CanonicalOperands;

    #[inline]
    fn operands_at(self, _function: FunctionId, _pc: Pc) -> Self::Operands {
        CanonicalOperands::new()
    }
}

/// Validated physical dispatch policy retained for the complete session.
#[derive(Clone, Copy)]
pub(super) struct PhysicalDispatch<'plan>(PhysicalExecutionView<'plan>);

impl<'plan> PhysicalDispatch<'plan> {
    pub(super) const fn new(plan: PhysicalExecutionView<'plan>) -> Self {
        Self(plan)
    }
}

impl<'plan> DispatchLayout<'plan> for PhysicalDispatch<'plan> {
    type Operands = PhysicalOperands<'plan>;

    #[inline]
    fn operands_at(self, function: FunctionId, pc: Pc) -> Self::Operands {
        let function = self.0.function_at(function.index());
        let physical_pc = function.canonical_pc_at(pc.index());
        let owner = function.physical_op(physical_pc.owner());
        debug_assert_eq!(
            owner.canonical_span().start().index() + physical_pc.owner_offset(),
            pc.index()
        );
        PhysicalOperands::new(physical_pc)
    }
}
