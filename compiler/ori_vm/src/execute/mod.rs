//! Quota-bounded bytecode interpreter.

mod arena;
mod calls;
mod dispatch;
mod frame;
mod heap;
mod objects;
mod operands;
mod ownership;
mod primitives;
mod profile;
mod report;
mod runtime;
mod value;

#[cfg(test)]
mod tests;

use ori_repr::executable::FunctionId;

#[cfg(test)]
use crate::bytecode::Register;
use crate::bytecode::{CallArgument, Pc, VerifiedProgram};
use crate::error::invalid_verified_index;
use crate::ExecutionError;

use arena::ValueArena;
use frame::Frame;
use heap::Heap;
use operands::{CanonicalDispatch, DispatchLayout, ExecutionLayout, FrameSlot, PhysicalDispatch};
use ownership::AdapterRetainWork;
use profile::{DispatchProbe, FrequencyProbe, NoDispatchProbe};
pub use profile::{
    ExecutionProfile, OpcodeCount, OpcodePairCount, ProfileFunctionId, ProfilePc,
    ProfiledExecutionReport, RegionCount,
};
pub use report::{ExecutionConfig, ExecutionMetrics, ExecutionOutcome, ExecutionReport};
pub use value::ExitValue;
use value::VmValue;

/// Execute verified bytecode in a fresh isolated VM session.
pub fn execute(
    program: &VerifiedProgram,
    config: ExecutionConfig,
) -> Result<ExecutionOutcome, ExecutionError> {
    let ExecutionReport {
        result,
        output,
        metrics,
    } = execute_report(program, config);
    result.map(|value| ExecutionOutcome {
        value,
        output,
        metrics,
    })
}

/// Execute verified bytecode and retain metrics for both success and failure.
pub fn execute_report(program: &VerifiedProgram, config: ExecutionConfig) -> ExecutionReport {
    Interpreter::new(program, config)
        .run_with_probe(NoDispatchProbe)
        .0
}

/// Execute through a construction-validated physical plan and retain runtime failures in the report.
pub fn execute_physical_report(
    plan: &crate::physical::PhysicalVmPlan<'_>,
    config: ExecutionConfig,
) -> ExecutionReport {
    let physical = plan.execution_view();
    Interpreter::new_with_layout(plan.program(), config, ExecutionLayout::Physical(physical))
        .run_with_probe(NoDispatchProbe)
        .0
}

/// Execute verified bytecode and collect deterministic dispatch frequencies.
pub fn execute_profiled(
    program: &VerifiedProgram,
    config: ExecutionConfig,
) -> ProfiledExecutionReport<'_> {
    let (execution, probe) =
        Interpreter::new(program, config).run_with_probe(FrequencyProbe::new(program));
    ProfiledExecutionReport {
        execution,
        profile: probe.finish(program),
    }
}

/// Execute through a validated physical plan and collect canonical dispatch frequencies.
pub fn execute_physical_profiled<'plan>(
    plan: &'plan crate::physical::PhysicalVmPlan<'_>,
    config: ExecutionConfig,
) -> ProfiledExecutionReport<'plan> {
    let physical = plan.execution_view();
    let program = plan.program();
    let (execution, probe) =
        Interpreter::new_with_layout(program, config, ExecutionLayout::Physical(physical))
            .run_with_probe(FrequencyProbe::new(program));
    ProfiledExecutionReport {
        execution,
        profile: probe.finish(program),
    }
}

struct Interpreter<'a> {
    program: &'a crate::bytecode::BytecodeProgram,
    layout: ExecutionLayout<'a>,
    config: ExecutionConfig,
    frames: Vec<Frame>,
    depth: usize,
    value_arena: ValueArena,
    heap: Heap,
    output: Vec<u8>,
    closure_scratch: Vec<VmValue>,
    ownership_scratch: Vec<VmValue>,
    ownership_count_scratch: Vec<(VmValue, u32)>,
    adapter_retain_scratch: Vec<AdapterRetainWork>,
    steps: u64,
    peak_frames: usize,
    pending_panic: Option<ExecutionError>,
}

#[derive(Clone, Copy)]
struct ArenaAllocationRoots<'a> {
    overwritten_register: Option<(usize, FrameSlot)>,
    allocation_local: &'a [VmValue],
}

impl<'a> ArenaAllocationRoots<'a> {
    #[cfg(test)]
    const fn all_live() -> Self {
        Self {
            overwritten_register: None,
            allocation_local: &[],
        }
    }

    fn overwriting(
        frame: usize,
        register: impl Into<FrameSlot>,
        allocation_local: &'a [VmValue],
    ) -> Self {
        Self {
            overwritten_register: Some((frame, register.into())),
            allocation_local,
        }
    }

    fn preserving(allocation_local: &'a [VmValue]) -> Self {
        Self {
            overwritten_register: None,
            allocation_local,
        }
    }
}

impl<'a> Interpreter<'a> {
    fn new(program: &'a VerifiedProgram, config: ExecutionConfig) -> Self {
        Self::new_with_layout(program, config, ExecutionLayout::Canonical)
    }

    fn new_with_layout(
        program: &'a VerifiedProgram,
        config: ExecutionConfig,
        layout: ExecutionLayout<'a>,
    ) -> Self {
        Self {
            program: &program.program,
            layout,
            config,
            frames: Vec::new(),
            depth: 0,
            value_arena: ValueArena::default(),
            heap: Heap::default(),
            output: Vec::new(),
            closure_scratch: Vec::new(),
            ownership_scratch: Vec::new(),
            ownership_count_scratch: Vec::new(),
            adapter_retain_scratch: Vec::new(),
            steps: 0,
            peak_frames: 0,
            pending_panic: None,
        }
    }

    fn run_with_probe<P: DispatchProbe>(mut self, mut probe: P) -> (ExecutionReport, P) {
        let result = self.run_session(&mut probe);
        let exit_heap = self.heap.metrics();
        let exit_value_arena = self.value_arena.metrics();
        let (peak_frame_owned_bytes, peak_register_owned_bytes, peak_scratch_owned_bytes) =
            self.frame_owned_bytes();
        self.heap.clear();
        self.value_arena.clear();
        let final_heap = self.heap.metrics();
        let final_value_arena = self.value_arena.metrics();
        let metrics = ExecutionMetrics {
            steps: self.steps,
            peak_frames: self.peak_frames,
            cumulative_heap_allocations: exit_heap.cumulative_allocations,
            cumulative_heap_payload_bytes_lower_bound: exit_heap
                .cumulative_payload_bytes_lower_bound,
            exit_live_heap_objects: exit_heap.live_objects,
            exit_live_heap_payload_bytes: exit_heap.live_payload_bytes,
            final_live_heap_objects: final_heap.live_objects,
            final_live_heap_payload_bytes: final_heap.live_payload_bytes,
            peak_heap_objects: exit_heap.peak_live_objects,
            peak_heap_payload_bytes: exit_heap.peak_live_payload_bytes,
            peak_heap_table_owned_bytes: exit_heap.peak_table_owned_bytes,
            cumulative_value_arena_allocations: exit_value_arena.cumulative_allocations,
            cumulative_value_arena_aggregate_allocations: exit_value_arena
                .cumulative_aggregate_allocations,
            cumulative_value_arena_iterator_allocations: exit_value_arena
                .cumulative_iterator_allocations,
            cumulative_value_arena_collections: exit_value_arena.cumulative_collections,
            cumulative_value_arena_reclaimed_entries: exit_value_arena.cumulative_reclaimed_entries,
            cumulative_value_arena_reused_entries: exit_value_arena.cumulative_reused_entries,
            exit_value_arena_entries: exit_value_arena.live_entries,
            exit_value_arena_slots: exit_value_arena.slots,
            exit_value_arena_owned_bytes: exit_value_arena.owned_bytes,
            final_value_arena_entries: final_value_arena.live_entries,
            final_value_arena_slots: final_value_arena.slots,
            final_value_arena_owned_bytes: final_value_arena.owned_bytes,
            peak_value_arena_entries: exit_value_arena.peak_live_entries,
            peak_value_arena_slots: exit_value_arena.peak_slots,
            peak_value_arena_owned_bytes: exit_value_arena.peak_owned_bytes,
            peak_frame_owned_bytes,
            peak_register_owned_bytes,
            peak_scratch_owned_bytes,
            peak_ownership_scratch_owned_bytes: owned_capacity_bytes::<VmValue>(
                self.ownership_scratch.capacity(),
            )
            .saturating_add(owned_capacity_bytes::<VmValue>(
                self.closure_scratch.capacity(),
            ))
            .saturating_add(owned_capacity_bytes::<(VmValue, u32)>(
                self.ownership_count_scratch.capacity(),
            ))
            .saturating_add(owned_capacity_bytes::<AdapterRetainWork>(
                self.adapter_retain_scratch.capacity(),
            )),
            peak_output_owned_bytes: owned_capacity_bytes::<u8>(self.output.capacity()),
        };
        (
            ExecutionReport {
                result,
                output: self.output,
                metrics,
            },
            probe,
        )
    }

    fn run_session<P: DispatchProbe>(
        &mut self,
        probe: &mut P,
    ) -> Result<ExitValue, ExecutionError> {
        self.push_root()?;
        match self.layout {
            ExecutionLayout::Canonical => self.run_dispatch_loop(probe, CanonicalDispatch),
            ExecutionLayout::Physical(physical) => {
                self.run_dispatch_loop(probe, PhysicalDispatch::new(physical))
            }
        }
    }

    fn run_dispatch_loop<'plan, P, L>(
        &mut self,
        probe: &mut P,
        layout: L,
    ) -> Result<ExitValue, ExecutionError>
    where
        P: DispatchProbe,
        L: DispatchLayout<'plan>,
    {
        let mut returned = VmValue::UNIT;
        while self.depth > 0 {
            self.record_step()?;
            if let Some(value) = self.step(probe, layout)? {
                returned = value;
            }
        }
        let exit_value = self.materialize(returned)?;
        self.release_owned_value(returned)?;
        if self.value_arena.metrics().live_entries > 0 {
            let max_entries = self.config.max_value_arena_entries;
            self.value_arena
                .collect(self.heap.value_roots(), max_entries)?;
        }
        Ok(exit_value)
    }

    fn frame_owned_bytes(&self) -> (usize, usize, usize) {
        let frame_table = owned_capacity_bytes::<Frame>(self.frames.capacity());
        let registers = self.frames.iter().fold(0_usize, |total, frame| {
            total.saturating_add(owned_capacity_bytes::<VmValue>(frame.registers.capacity()))
        });
        let scratch = self.frames.iter().fold(0_usize, |total, frame| {
            total.saturating_add(owned_capacity_bytes::<VmValue>(
                frame.move_scratch.capacity(),
            ))
        });
        (frame_table, registers, scratch)
    }

    fn prepare_value_arena_allocation(
        &mut self,
        roots: ArenaAllocationRoots<'_>,
    ) -> Result<(), ExecutionError> {
        let max_entries = self.config.max_value_arena_entries;
        if !self
            .value_arena
            .allocation_batch_requires_collection(1, max_entries)
        {
            return Ok(());
        }
        let overwritten = roots.overwritten_register;
        let frame_roots =
            self.frames[..self.depth]
                .iter()
                .enumerate()
                .flat_map(|(frame_index, frame)| {
                    frame
                        .registers
                        .iter()
                        .enumerate()
                        .filter_map(move |(register_index, &value)| {
                            let is_overwritten =
                                overwritten.is_some_and(|(killed_frame, register)| {
                                    killed_frame == frame_index
                                        && register.index() == register_index
                                });
                            (!is_overwritten).then_some(value)
                        })
                        .chain(frame.move_scratch.iter().copied())
                });
        let heap_roots = self.heap.value_roots();
        let ownership_roots = self.ownership_scratch.iter().copied();
        self.value_arena.collect(
            frame_roots
                .chain(heap_roots)
                .chain(ownership_roots)
                .chain(roots.allocation_local.iter().copied()),
            max_entries,
        )
    }

    fn prepare_value_arena_allocations(
        &mut self,
        count: usize,
        roots: ArenaAllocationRoots<'_>,
    ) -> Result<(), ExecutionError> {
        let max_entries = self.config.max_value_arena_entries;
        if !self
            .value_arena
            .allocation_batch_requires_collection(count, max_entries)
        {
            return self
                .value_arena
                .validate_aggregate_allocation_batch(count, max_entries);
        }
        let overwritten = roots.overwritten_register;
        let frame_roots =
            self.frames[..self.depth]
                .iter()
                .enumerate()
                .flat_map(|(frame_index, frame)| {
                    frame
                        .registers
                        .iter()
                        .enumerate()
                        .filter_map(move |(register_index, &value)| {
                            let is_overwritten =
                                overwritten.is_some_and(|(killed_frame, register)| {
                                    killed_frame == frame_index
                                        && register.index() == register_index
                                });
                            (!is_overwritten).then_some(value)
                        })
                        .chain(frame.move_scratch.iter().copied())
                });
        let heap_roots = self.heap.value_roots();
        let ownership_roots = self.ownership_scratch.iter().copied();
        self.value_arena.collect(
            frame_roots
                .chain(heap_roots)
                .chain(ownership_roots)
                .chain(roots.allocation_local.iter().copied()),
            max_entries,
        )?;
        self.value_arena
            .validate_aggregate_allocation_batch(count, max_entries)
    }

    fn record_step(&mut self) -> Result<(), ExecutionError> {
        if self.steps >= self.config.max_steps {
            return Err(ExecutionError::StepLimit {
                limit: self.config.max_steps,
            });
        }
        self.steps = self
            .steps
            .checked_add(1)
            .ok_or(ExecutionError::StepCounterOverflow)?;
        Ok(())
    }

    #[inline]
    #[cfg(test)]
    fn register(&self, frame: usize, register: Register) -> VmValue {
        let slot = self
            .layout
            .frame_slot(self.frames[frame].function, register);
        self.frames[frame].registers[slot.index()]
    }

    #[cfg(test)]
    fn set_register(&mut self, frame: usize, register: Register, value: VmValue) {
        let slot = self
            .layout
            .frame_slot(self.frames[frame].function, register);
        self.set_slot(frame, slot, value);
    }

    fn set_slot(&mut self, frame: usize, slot: FrameSlot, value: VmValue) {
        self.frames[frame].registers[slot.index()] = value;
    }

    #[inline]
    fn advance(&mut self, frame: usize) {
        self.frames[frame].pc = self.frames[frame].pc.next_verified();
    }

    fn function(&self, function: FunctionId) -> &crate::bytecode::BytecodeFunction {
        &self.program.functions[function.index()]
    }

    fn call_arguments(&self, index: usize) -> &[CallArgument] {
        &self.program.call_arguments[index]
    }

    fn switch_table(&self, index: usize) -> &[(u64, Pc)] {
        &self.program.switches[index]
    }
}

fn owned_capacity_bytes<T>(capacity: usize) -> usize {
    capacity.saturating_mul(std::mem::size_of::<T>())
}
