//! Compact physical operands and canonical-progress maps.

use std::num::NonZeroU32;

use crate::bytecode::Pc;

use super::PhysicalLayoutViolation;

#[derive(Debug)]
pub(super) struct PhysicalFunctionPlan {
    pub(super) lanes: Box<[PhysicalLane]>,
    pub(super) physical_ops: Box<[PhysicalOpPlan]>,
    pub(super) canonical_pcs: Box<[PhysicalPcPlan]>,
    pub(super) reads: Box<[PhysicalRead]>,
    pub(super) writes: Box<[PhysicalLane]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PhysicalLane(u32);

impl PhysicalLane {
    pub(super) const fn new(index: u32) -> Self {
        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalRead {
    /// A dynamically tagged VM value in a physical frame lane.
    Lane(PhysicalLane),
    /// A path-proven concrete value requiring no frame load.
    Immediate(PhysicalImmediate),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalImmediate {
    Int(i64),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PhysicalOpIndex(u32);

impl PhysicalOpIndex {
    pub(super) fn new(index: usize) -> Option<Self> {
        u32::try_from(index).ok().map(Self)
    }

    pub(super) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One executor entry and the ordered canonical instructions it owns.
///
/// Whole-span execution is only eligible when probes are disabled and fuel
/// covers the complete nonempty span. Otherwise execution enters canonical-
/// prefix micro-mode using the reverse per-PC map below. Even in whole-span
/// mode, an error-capable implementation must report progress at the owning
/// canonical PC rather than at the physical entry boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PhysicalOpPlan {
    canonical: CanonicalPcSpan,
}

impl PhysicalOpPlan {
    pub(super) const fn identity(pc: Pc) -> Self {
        Self {
            canonical: CanonicalPcSpan::single(pc),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CanonicalPcSpan {
    start: Pc,
    length: NonZeroU32,
}

impl CanonicalPcSpan {
    const fn single(start: Pc) -> Self {
        Self {
            start,
            length: NonZeroU32::MIN,
        }
    }

    pub(crate) const fn start(self) -> Pc {
        self.start
    }

    pub(crate) const fn len(self) -> usize {
        self.length.get() as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PhysicalPcPlan {
    owner: PhysicalOpIndex,
    owner_offset: u32,
    reads: OperandSpan,
    writes: OperandSpan,
    flags: PcFlags,
}

impl PhysicalPcPlan {
    pub(super) const fn new(
        owner: PhysicalOpIndex,
        owner_offset: u32,
        reads: OperandSpan,
        writes: OperandSpan,
        copy_is_noop: bool,
    ) -> Self {
        Self {
            owner,
            owner_offset,
            reads,
            writes,
            flags: PcFlags::new(copy_is_noop),
        }
    }

    pub(super) const fn copy_is_noop(&self) -> bool {
        self.flags.copy_is_noop()
    }

    #[cfg(test)]
    pub(super) fn set_copy_is_noop_for_test(&mut self, copy_is_noop: bool) {
        self.flags = PcFlags::new(copy_is_noop);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OperandSpan {
    start: u32,
    length: u32,
}

impl OperandSpan {
    pub(super) fn new(start: usize, length: usize) -> Option<Self> {
        Some(Self {
            start: u32::try_from(start).ok()?,
            length: u32::try_from(length).ok()?,
        })
    }

    pub(super) const fn start(self) -> usize {
        self.start as usize
    }

    pub(super) const fn len(self) -> usize {
        self.length as usize
    }

    pub(super) fn end(self) -> Option<usize> {
        self.start().checked_add(self.len())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PcFlags(u8);

impl PcFlags {
    const COPY_IS_NOOP: u8 = 1;

    const fn new(copy_is_noop: bool) -> Self {
        if copy_is_noop {
            Self(Self::COPY_IS_NOOP)
        } else {
            Self(0)
        }
    }

    pub(super) const fn copy_is_noop(self) -> bool {
        self.0 & Self::COPY_IS_NOOP != 0
    }
}

/// Narrow read-only surface consumed by physical execution and plan metrics.
#[derive(Clone, Copy)]
pub(crate) struct PhysicalExecutionView<'plan> {
    functions: &'plan [PhysicalFunctionPlan],
}

impl<'plan> PhysicalExecutionView<'plan> {
    pub(super) const fn new(functions: &'plan [PhysicalFunctionPlan]) -> Self {
        Self { functions }
    }

    pub(crate) fn function_at(self, index: usize) -> PhysicalFunctionView<'plan> {
        PhysicalFunctionView {
            plan: &self.functions[index],
        }
    }
}

/// Compact frame and operand layout for one verified function.
#[derive(Clone, Copy)]
pub(crate) struct PhysicalFunctionView<'plan> {
    plan: &'plan PhysicalFunctionPlan,
}

impl<'plan> PhysicalFunctionView<'plan> {
    pub(crate) const fn lane_count(self) -> usize {
        self.plan.lanes.len()
    }

    pub(crate) const fn physical_op_count(self) -> usize {
        self.plan.physical_ops.len()
    }

    pub(crate) const fn canonical_pc_count(self) -> usize {
        self.plan.canonical_pcs.len()
    }

    pub(crate) const fn read_count(self) -> usize {
        self.plan.reads.len()
    }

    pub(crate) const fn write_count(self) -> usize {
        self.plan.writes.len()
    }

    pub(crate) fn lane_at(self, index: usize) -> PhysicalLane {
        self.plan.lanes[index]
    }

    pub(crate) fn physical_op(self, index: usize) -> PhysicalOpView<'plan> {
        PhysicalOpView {
            plan: &self.plan.physical_ops[index],
        }
    }

    pub(crate) fn canonical_pc_at(self, index: usize) -> PhysicalPcView<'plan> {
        PhysicalPcView {
            plan: &self.plan.canonical_pcs[index],
            reads: &self.plan.reads,
            writes: &self.plan.writes,
        }
    }

    pub(crate) fn validate_construction(self) -> Result<(), PhysicalLayoutViolation> {
        let mut expected_start = 0_usize;
        for owner_index in 0..self.physical_op_count() {
            let operation = self.physical_op(owner_index);
            let span = operation.canonical_span();
            if span.len() == 0 {
                return Err(PhysicalLayoutViolation::EmptyCanonicalSpan {
                    physical_op: owner_index,
                });
            }
            if span.start().index() != expected_start {
                return Err(PhysicalLayoutViolation::CanonicalSpanOrder {
                    physical_op: owner_index,
                    expected_start,
                    actual_start: span.start().index(),
                });
            }
            let Some(end) = expected_start.checked_add(span.len()) else {
                return Err(PhysicalLayoutViolation::CanonicalSpanBounds {
                    physical_op: owner_index,
                    end: usize::MAX,
                    canonical_pcs: self.canonical_pc_count(),
                });
            };
            if end > self.canonical_pc_count() {
                return Err(PhysicalLayoutViolation::CanonicalSpanBounds {
                    physical_op: owner_index,
                    end,
                    canonical_pcs: self.canonical_pc_count(),
                });
            }
            debug_assert!(operation.whole_span_eligible(span.len() as u64, false));
            debug_assert!(!operation.whole_span_eligible(u64::MAX, true));
            for (offset, pc_index) in (expected_start..end).enumerate() {
                self.validate_canonical_pc(owner_index, offset, pc_index)?;
            }
            expected_start = end;
        }
        if expected_start != self.canonical_pc_count() {
            return Err(PhysicalLayoutViolation::CanonicalCoverage {
                covered: expected_start,
                canonical_pcs: self.canonical_pc_count(),
            });
        }
        Ok(())
    }

    fn validate_canonical_pc(
        self,
        owner_index: usize,
        offset: usize,
        pc_index: usize,
    ) -> Result<(), PhysicalLayoutViolation> {
        let pc_view = self.canonical_pc_at(pc_index);
        if pc_view.owner() != owner_index {
            return Err(PhysicalLayoutViolation::ReverseOwner {
                canonical_pc: pc_index,
                expected_owner: owner_index,
                actual_owner: pc_view.owner(),
            });
        }
        if pc_view.owner_offset() != offset {
            return Err(PhysicalLayoutViolation::ReverseOffset {
                canonical_pc: pc_index,
                expected_offset: offset,
                actual_offset: pc_view.owner_offset(),
            });
        }
        let pc = &self.plan.canonical_pcs[pc_index];
        self.validate_read_bindings(pc_index, pc)?;
        self.validate_write_bindings(pc_index, pc)
    }

    fn validate_read_bindings(
        self,
        pc_index: usize,
        pc: &PhysicalPcPlan,
    ) -> Result<(), PhysicalLayoutViolation> {
        let Some(read_end) = pc.reads.end() else {
            return Err(PhysicalLayoutViolation::ReadSpanOverflow {
                canonical_pc: pc_index,
            });
        };
        if read_end > self.read_count() {
            return Err(PhysicalLayoutViolation::ReadSpanBounds {
                canonical_pc: pc_index,
                end: read_end,
                read_bindings: self.read_count(),
            });
        }
        for (operand, read) in self.plan.reads[pc.reads.start()..read_end]
            .iter()
            .enumerate()
        {
            if let PhysicalRead::Lane(lane) = read {
                if lane.index() >= self.lane_count() {
                    return Err(PhysicalLayoutViolation::ReadLaneBounds {
                        canonical_pc: pc_index,
                        operand,
                        lane: lane.index(),
                        physical_lanes: self.lane_count(),
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_write_bindings(
        self,
        pc_index: usize,
        pc: &PhysicalPcPlan,
    ) -> Result<(), PhysicalLayoutViolation> {
        let Some(write_end) = pc.writes.end() else {
            return Err(PhysicalLayoutViolation::WriteSpanOverflow {
                canonical_pc: pc_index,
            });
        };
        if write_end > self.write_count() {
            return Err(PhysicalLayoutViolation::WriteSpanBounds {
                canonical_pc: pc_index,
                end: write_end,
                write_bindings: self.write_count(),
            });
        }
        for (operand, lane) in self.plan.writes[pc.writes.start()..write_end]
            .iter()
            .enumerate()
        {
            if lane.index() >= self.lane_count() {
                return Err(PhysicalLayoutViolation::WriteLaneBounds {
                    canonical_pc: pc_index,
                    operand,
                    lane: lane.index(),
                    physical_lanes: self.lane_count(),
                });
            }
        }
        Ok(())
    }
}

/// Canonical progress owned by one physical executor entry.
#[derive(Clone, Copy)]
pub(crate) struct PhysicalOpView<'plan> {
    plan: &'plan PhysicalOpPlan,
}

impl PhysicalOpView<'_> {
    pub(crate) const fn canonical_span(self) -> CanonicalPcSpan {
        self.plan.canonical
    }

    /// Whether whole-span execution may be entered before semantic dispatch.
    ///
    /// A false result selects canonical-prefix micro-mode over the same plan
    /// and runtime primitives; it never selects a different semantic engine.
    pub(crate) fn whole_span_eligible(self, remaining_fuel: u64, probes_enabled: bool) -> bool {
        !probes_enabled && remaining_fuel >= self.plan.canonical.length.get().into()
    }
}

/// O(1) operand bindings and progress coordinates for one canonical PC.
#[derive(Clone, Copy)]
pub(crate) struct PhysicalPcView<'plan> {
    plan: &'plan PhysicalPcPlan,
    reads: &'plan [PhysicalRead],
    writes: &'plan [PhysicalLane],
}

impl<'plan> PhysicalPcView<'plan> {
    pub(crate) const fn owner(self) -> usize {
        self.plan.owner.index()
    }

    pub(crate) const fn owner_offset(self) -> usize {
        self.plan.owner_offset as usize
    }

    pub(super) const fn read_start(self) -> usize {
        self.plan.reads.start()
    }

    pub(super) const fn write_start(self) -> usize {
        self.plan.writes.start()
    }

    pub(crate) fn reads(self) -> &'plan [PhysicalRead] {
        let start = self.plan.reads.start();
        &self.reads[start..start + self.plan.reads.len()]
    }

    pub(crate) fn writes(self) -> &'plan [PhysicalLane] {
        let start = self.plan.writes.start();
        &self.writes[start..start + self.plan.writes.len()]
    }

    pub(crate) const fn copy_is_noop(self) -> bool {
        self.plan.flags.copy_is_noop()
    }
}
