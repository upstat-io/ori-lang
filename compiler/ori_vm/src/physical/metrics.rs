//! Exact physical-plan storage and transformation accounting.

use std::mem::size_of;

use crate::bytecode::{BytecodeFunction, Op, VerifiedProgram};

use super::facts::PlanningScratchMetrics;
use super::layout::{PhysicalFunctionPlan, PhysicalOpPlan, PhysicalPcPlan};
use super::{PhysicalLane, PhysicalRead, PrepareError};

/// Stable size and transformation metrics for one prepared physical plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalPlanMetrics {
    /// Logical register lanes in canonical verified bytecode.
    pub logical_lanes: usize,
    /// Runtime storage lanes in physical frame layouts.
    pub physical_lanes: usize,
    /// Canonical verified instructions covered by progress maps.
    pub canonical_ops: usize,
    /// Physical executor entries.
    pub physical_ops: usize,
    /// Ordered physical read operands.
    pub read_bindings: usize,
    /// Ordered physical destination lanes.
    pub write_bindings: usize,
    /// Reads replaced by path-proven concrete immediates.
    pub immediate_bindings: usize,
    /// Canonical copies proven to require no physical movement.
    pub coalesced_copies: usize,
    /// Payload bytes retained by plan-owned allocations.
    pub owned_plan_bytes: usize,
    /// Canonical instruction payload bytes retained by the borrowed program.
    pub canonical_op_bytes: usize,
    /// Canonical instruction payload plus plan-owned allocation payload.
    pub retained_canonical_and_plan_bytes: usize,
    /// Tracked construction-scratch capacity payload retained after construction.
    pub planning_scratch_current_payload_bytes: usize,
    /// Peak tracked construction-scratch capacity payload.
    ///
    /// This excludes allocator metadata, stack storage, and untracked allocations,
    /// so it is only a lower bound on total construction memory.
    pub planning_scratch_peak_payload_bytes_lower_bound: usize,
    /// Cumulative tracked construction-scratch capacity allocation.
    ///
    /// This excludes allocator metadata and untracked allocations.
    pub planning_scratch_cumulative_allocation_bytes_lower_bound: usize,
    /// Tracked release-validation scratch retained after validation.
    pub validation_scratch_current_payload_bytes: usize,
    /// Peak tracked release-validation scratch capacity payload.
    ///
    /// This remains separate from construction scratch and retained plan bytes.
    pub validation_scratch_peak_payload_bytes_lower_bound: usize,
    /// Cumulative tracked release-validation scratch capacity allocation.
    pub validation_scratch_cumulative_allocation_bytes_lower_bound: usize,
}

/// Exact host sizes of the canonical and physical plan table elements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalElementSizes {
    /// Size of one canonical bytecode instruction.
    pub canonical_op: usize,
    /// Size of one per-function physical-plan header.
    pub physical_function_plan: usize,
    /// Size of one physical executor entry.
    pub physical_op_plan: usize,
    /// Size of one reverse canonical-PC mapping entry.
    pub physical_pc_plan: usize,
    /// Size of one flattened physical read binding.
    pub physical_read: usize,
    /// Size of one flattened physical write binding.
    pub physical_write: usize,
    /// Size of one physical frame-lane identity.
    pub physical_lane: usize,
}

impl PhysicalElementSizes {
    pub(super) const fn current() -> Self {
        Self {
            canonical_op: size_of::<Op>(),
            physical_function_plan: size_of::<PhysicalFunctionPlan>(),
            physical_op_plan: size_of::<PhysicalOpPlan>(),
            physical_pc_plan: size_of::<PhysicalPcPlan>(),
            physical_read: size_of::<PhysicalRead>(),
            physical_write: size_of::<PhysicalLane>(),
            physical_lane: size_of::<PhysicalLane>(),
        }
    }
}

/// Exact retained instruction and physical-plan payload for one function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalFunctionStorageMetrics {
    /// Canonical instruction count.
    pub canonical_ops: usize,
    /// Physical executor-entry count.
    pub physical_ops: usize,
    /// Reverse canonical-PC entry count.
    pub physical_pcs: usize,
    /// Flattened read-binding count.
    pub reads: usize,
    /// Flattened write-binding count.
    pub writes: usize,
    /// Physical frame-lane count.
    pub lanes: usize,
    /// Canonical instruction payload bytes.
    pub canonical_op_bytes: usize,
    /// Per-function physical-plan header bytes.
    pub physical_function_plan_bytes: usize,
    /// Physical executor-entry payload bytes.
    pub physical_op_bytes: usize,
    /// Reverse canonical-PC payload bytes.
    pub physical_pc_bytes: usize,
    /// Flattened read-binding payload bytes.
    pub physical_read_bytes: usize,
    /// Flattened write-binding payload bytes.
    pub physical_write_bytes: usize,
    /// Physical frame-lane identity payload bytes.
    pub physical_lane_bytes: usize,
    /// Total plan-owned allocation payload for this function.
    pub owned_plan_bytes: usize,
    /// Canonical instruction payload plus plan-owned allocation payload.
    pub retained_canonical_and_plan_bytes: usize,
}

pub(super) fn function_storage_metrics(
    function_index: usize,
    canonical: &BytecodeFunction,
    physical: &PhysicalFunctionPlan,
) -> Result<PhysicalFunctionStorageMetrics, PrepareError> {
    let canonical_op_bytes =
        checked_storage_mul::<Op>(canonical.ops.len(), function_index, "canonical op")?;
    let physical_function_plan_bytes = size_of::<PhysicalFunctionPlan>();
    let physical_op_bytes = checked_storage_mul::<PhysicalOpPlan>(
        physical.physical_ops.len(),
        function_index,
        "physical op",
    )?;
    let physical_pc_bytes = checked_storage_mul::<PhysicalPcPlan>(
        physical.canonical_pcs.len(),
        function_index,
        "canonical-PC map",
    )?;
    let physical_read_bytes =
        checked_storage_mul::<PhysicalRead>(physical.reads.len(), function_index, "read binding")?;
    let physical_write_bytes = checked_storage_mul::<PhysicalLane>(
        physical.writes.len(),
        function_index,
        "write binding",
    )?;
    let physical_lane_bytes =
        checked_storage_mul::<PhysicalLane>(physical.lanes.len(), function_index, "lane identity")?;
    let owned_plan_bytes = [
        physical_op_bytes,
        physical_pc_bytes,
        physical_read_bytes,
        physical_write_bytes,
        physical_lane_bytes,
    ]
    .into_iter()
    .try_fold(physical_function_plan_bytes, |total, bytes| {
        checked_storage_add(total, bytes, function_index, "owned plan payload")
    })?;
    let retained_canonical_and_plan_bytes = checked_storage_add(
        canonical_op_bytes,
        owned_plan_bytes,
        function_index,
        "retained canonical and plan payload",
    )?;
    Ok(PhysicalFunctionStorageMetrics {
        canonical_ops: canonical.ops.len(),
        physical_ops: physical.physical_ops.len(),
        physical_pcs: physical.canonical_pcs.len(),
        reads: physical.reads.len(),
        writes: physical.writes.len(),
        lanes: physical.lanes.len(),
        canonical_op_bytes,
        physical_function_plan_bytes,
        physical_op_bytes,
        physical_pc_bytes,
        physical_read_bytes,
        physical_write_bytes,
        physical_lane_bytes,
        owned_plan_bytes,
        retained_canonical_and_plan_bytes,
    })
}

pub(super) fn plan_metrics(
    program: &VerifiedProgram,
    functions: &[PhysicalFunctionPlan],
    planning: PlanningScratchMetrics,
    validation: PlanningScratchMetrics,
) -> Result<PhysicalPlanMetrics, PrepareError> {
    let mut metrics = PhysicalPlanMetrics {
        logical_lanes: 0,
        physical_lanes: 0,
        canonical_ops: 0,
        physical_ops: 0,
        read_bindings: 0,
        write_bindings: 0,
        immediate_bindings: 0,
        coalesced_copies: 0,
        owned_plan_bytes: 0,
        canonical_op_bytes: 0,
        retained_canonical_and_plan_bytes: 0,
        planning_scratch_current_payload_bytes: planning.current_payload_bytes,
        planning_scratch_peak_payload_bytes_lower_bound: planning.peak_payload_bytes_lower_bound,
        planning_scratch_cumulative_allocation_bytes_lower_bound: planning
            .cumulative_allocation_bytes_lower_bound,
        validation_scratch_current_payload_bytes: validation.current_payload_bytes,
        validation_scratch_peak_payload_bytes_lower_bound: validation
            .peak_payload_bytes_lower_bound,
        validation_scratch_cumulative_allocation_bytes_lower_bound: validation
            .cumulative_allocation_bytes_lower_bound,
    };
    for (function_index, physical) in functions.iter().enumerate() {
        let canonical = &program.program.functions[function_index];
        let storage = function_storage_metrics(function_index, canonical, physical)?;
        metrics.logical_lanes = checked_metric_add(
            metrics.logical_lanes,
            canonical.register_count,
            "lane count",
        )?;
        metrics.physical_lanes =
            checked_metric_add(metrics.physical_lanes, physical.lanes.len(), "lane count")?;
        metrics.canonical_ops =
            checked_metric_add(metrics.canonical_ops, canonical.ops.len(), "op count")?;
        metrics.physical_ops = checked_metric_add(
            metrics.physical_ops,
            physical.physical_ops.len(),
            "op count",
        )?;
        metrics.read_bindings = checked_metric_add(
            metrics.read_bindings,
            physical.reads.len(),
            "read-binding count",
        )?;
        metrics.write_bindings = checked_metric_add(
            metrics.write_bindings,
            physical.writes.len(),
            "write-binding count",
        )?;
        metrics.immediate_bindings = checked_metric_add(
            metrics.immediate_bindings,
            physical
                .reads
                .iter()
                .filter(|read| matches!(read, PhysicalRead::Immediate(_)))
                .count(),
            "immediate-binding count",
        )?;
        metrics.coalesced_copies = checked_metric_add(
            metrics.coalesced_copies,
            physical
                .canonical_pcs
                .iter()
                .filter(|pc| pc.copy_is_noop())
                .count(),
            "coalesced-copy count",
        )?;
        metrics.owned_plan_bytes = checked_metric_add(
            metrics.owned_plan_bytes,
            storage.owned_plan_bytes,
            "owned payload bytes",
        )?;
        metrics.canonical_op_bytes = checked_metric_add(
            metrics.canonical_op_bytes,
            storage.canonical_op_bytes,
            "canonical op bytes",
        )?;
    }
    metrics.retained_canonical_and_plan_bytes = checked_metric_add(
        metrics.canonical_op_bytes,
        metrics.owned_plan_bytes,
        "retained canonical and plan bytes",
    )?;
    Ok(metrics)
}

pub(super) fn checked_metric_add(
    left: usize,
    right: usize,
    metric: &'static str,
) -> Result<usize, PrepareError> {
    left.checked_add(right)
        .ok_or(PrepareError::MetricOverflow { metric })
}

pub(super) fn checked_storage_mul<T>(
    count: usize,
    function: usize,
    table: &'static str,
) -> Result<usize, PrepareError> {
    size_of::<T>()
        .checked_mul(count)
        .ok_or(PrepareError::FunctionStorageSizeOverflow { function, table })
}

pub(super) fn checked_storage_add(
    left: usize,
    right: usize,
    function: usize,
    table: &'static str,
) -> Result<usize, PrepareError> {
    left.checked_add(right)
        .ok_or(PrepareError::FunctionStorageSizeOverflow { function, table })
}
