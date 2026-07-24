//! Authoritative bytecode and execution resource records.

use serde::{Deserialize, Serialize};

/// Stable bytecode counts plus explicit retained-byte availability.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BytecodeMetricsRecord {
    pub(crate) function_count_decimal: String,
    pub(crate) instruction_count_decimal: String,
    pub(crate) retained_bytes: RetainedByteMetric,
}

impl From<ori_vm::BytecodeMetrics> for BytecodeMetricsRecord {
    fn from(metrics: ori_vm::BytecodeMetrics) -> Self {
        Self {
            function_count_decimal: metrics.function_count.to_string(),
            instruction_count_decimal: metrics.instruction_count.to_string(),
            retained_bytes: RetainedByteMetric::Unavailable {
                reason: RetainedByteUnavailableReason::AuthoritativeMetricNotExposed,
            },
        }
    }
}

/// Every field exported by [`ori_vm::PhysicalPlanMetrics`], encoded in base-10.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PhysicalPlanMetricsRecord {
    pub(crate) logical_lanes: String,
    pub(crate) physical_lanes: String,
    pub(crate) canonical_ops: String,
    pub(crate) physical_ops: String,
    pub(crate) read_bindings: String,
    pub(crate) write_bindings: String,
    pub(crate) immediate_bindings: String,
    pub(crate) coalesced_copies: String,
    pub(crate) owned_plan_bytes: String,
    pub(crate) canonical_op_bytes: String,
    pub(crate) retained_canonical_and_plan_bytes: String,
    pub(crate) planning_scratch_current_payload_bytes: String,
    pub(crate) planning_scratch_peak_payload_bytes_lower_bound: String,
    pub(crate) planning_scratch_cumulative_allocation_bytes_lower_bound: String,
    pub(crate) validation_scratch_current_payload_bytes: String,
    pub(crate) validation_scratch_peak_payload_bytes_lower_bound: String,
    pub(crate) validation_scratch_cumulative_allocation_bytes_lower_bound: String,
}

impl From<ori_vm::PhysicalPlanMetrics> for PhysicalPlanMetricsRecord {
    fn from(metrics: ori_vm::PhysicalPlanMetrics) -> Self {
        Self {
            logical_lanes: metrics.logical_lanes.to_string(),
            physical_lanes: metrics.physical_lanes.to_string(),
            canonical_ops: metrics.canonical_ops.to_string(),
            physical_ops: metrics.physical_ops.to_string(),
            read_bindings: metrics.read_bindings.to_string(),
            write_bindings: metrics.write_bindings.to_string(),
            immediate_bindings: metrics.immediate_bindings.to_string(),
            coalesced_copies: metrics.coalesced_copies.to_string(),
            owned_plan_bytes: metrics.owned_plan_bytes.to_string(),
            canonical_op_bytes: metrics.canonical_op_bytes.to_string(),
            retained_canonical_and_plan_bytes: metrics
                .retained_canonical_and_plan_bytes
                .to_string(),
            planning_scratch_current_payload_bytes: metrics
                .planning_scratch_current_payload_bytes
                .to_string(),
            planning_scratch_peak_payload_bytes_lower_bound: metrics
                .planning_scratch_peak_payload_bytes_lower_bound
                .to_string(),
            planning_scratch_cumulative_allocation_bytes_lower_bound: metrics
                .planning_scratch_cumulative_allocation_bytes_lower_bound
                .to_string(),
            validation_scratch_current_payload_bytes: metrics
                .validation_scratch_current_payload_bytes
                .to_string(),
            validation_scratch_peak_payload_bytes_lower_bound: metrics
                .validation_scratch_peak_payload_bytes_lower_bound
                .to_string(),
            validation_scratch_cumulative_allocation_bytes_lower_bound: metrics
                .validation_scratch_cumulative_allocation_bytes_lower_bound
                .to_string(),
        }
    }
}

/// Bytecode-retention availability without an inferred approximation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RetainedByteMetric {
    Unavailable {
        reason: RetainedByteUnavailableReason,
    },
}

/// Why bytecode-owned bytes are absent from a record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RetainedByteUnavailableReason {
    AuthoritativeMetricNotExposed,
}

/// Every field exported by [`ori_vm::ExecutionMetrics`], encoded in base-10.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutionMetricsRecord {
    pub(crate) steps: String,
    pub(crate) peak_frames: String,
    pub(crate) cumulative_heap_allocations: String,
    pub(crate) cumulative_heap_payload_bytes_lower_bound: String,
    pub(crate) exit_live_heap_objects: String,
    pub(crate) exit_live_heap_payload_bytes: String,
    pub(crate) final_live_heap_objects: String,
    pub(crate) final_live_heap_payload_bytes: String,
    pub(crate) peak_heap_objects: String,
    pub(crate) peak_heap_payload_bytes: String,
    pub(crate) peak_heap_table_owned_bytes: String,
    pub(crate) cumulative_value_arena_allocations: String,
    pub(crate) cumulative_value_arena_aggregate_allocations: String,
    pub(crate) cumulative_value_arena_iterator_allocations: String,
    pub(crate) cumulative_value_arena_collections: String,
    pub(crate) cumulative_value_arena_reclaimed_entries: String,
    pub(crate) cumulative_value_arena_reused_entries: String,
    pub(crate) exit_value_arena_entries: String,
    pub(crate) exit_value_arena_slots: String,
    pub(crate) exit_value_arena_owned_bytes: String,
    pub(crate) final_value_arena_entries: String,
    pub(crate) final_value_arena_slots: String,
    pub(crate) final_value_arena_owned_bytes: String,
    pub(crate) peak_value_arena_entries: String,
    pub(crate) peak_value_arena_slots: String,
    pub(crate) peak_value_arena_owned_bytes: String,
    pub(crate) peak_frame_owned_bytes: String,
    pub(crate) peak_register_owned_bytes: String,
    pub(crate) peak_scratch_owned_bytes: String,
    pub(crate) peak_ownership_scratch_owned_bytes: String,
    pub(crate) peak_output_owned_bytes: String,
}

impl From<&ori_vm::ExecutionMetrics> for ExecutionMetricsRecord {
    fn from(metrics: &ori_vm::ExecutionMetrics) -> Self {
        Self {
            steps: metrics.steps.to_string(),
            peak_frames: metrics.peak_frames.to_string(),
            cumulative_heap_allocations: metrics.cumulative_heap_allocations.to_string(),
            cumulative_heap_payload_bytes_lower_bound: metrics
                .cumulative_heap_payload_bytes_lower_bound
                .to_string(),
            exit_live_heap_objects: metrics.exit_live_heap_objects.to_string(),
            exit_live_heap_payload_bytes: metrics.exit_live_heap_payload_bytes.to_string(),
            final_live_heap_objects: metrics.final_live_heap_objects.to_string(),
            final_live_heap_payload_bytes: metrics.final_live_heap_payload_bytes.to_string(),
            peak_heap_objects: metrics.peak_heap_objects.to_string(),
            peak_heap_payload_bytes: metrics.peak_heap_payload_bytes.to_string(),
            peak_heap_table_owned_bytes: metrics.peak_heap_table_owned_bytes.to_string(),
            cumulative_value_arena_allocations: metrics
                .cumulative_value_arena_allocations
                .to_string(),
            cumulative_value_arena_aggregate_allocations: metrics
                .cumulative_value_arena_aggregate_allocations
                .to_string(),
            cumulative_value_arena_iterator_allocations: metrics
                .cumulative_value_arena_iterator_allocations
                .to_string(),
            cumulative_value_arena_collections: metrics
                .cumulative_value_arena_collections
                .to_string(),
            cumulative_value_arena_reclaimed_entries: metrics
                .cumulative_value_arena_reclaimed_entries
                .to_string(),
            cumulative_value_arena_reused_entries: metrics
                .cumulative_value_arena_reused_entries
                .to_string(),
            exit_value_arena_entries: metrics.exit_value_arena_entries.to_string(),
            exit_value_arena_slots: metrics.exit_value_arena_slots.to_string(),
            exit_value_arena_owned_bytes: metrics.exit_value_arena_owned_bytes.to_string(),
            final_value_arena_entries: metrics.final_value_arena_entries.to_string(),
            final_value_arena_slots: metrics.final_value_arena_slots.to_string(),
            final_value_arena_owned_bytes: metrics.final_value_arena_owned_bytes.to_string(),
            peak_value_arena_entries: metrics.peak_value_arena_entries.to_string(),
            peak_value_arena_slots: metrics.peak_value_arena_slots.to_string(),
            peak_value_arena_owned_bytes: metrics.peak_value_arena_owned_bytes.to_string(),
            peak_frame_owned_bytes: metrics.peak_frame_owned_bytes.to_string(),
            peak_register_owned_bytes: metrics.peak_register_owned_bytes.to_string(),
            peak_scratch_owned_bytes: metrics.peak_scratch_owned_bytes.to_string(),
            peak_ownership_scratch_owned_bytes: metrics
                .peak_ownership_scratch_owned_bytes
                .to_string(),
            peak_output_owned_bytes: metrics.peak_output_owned_bytes.to_string(),
        }
    }
}
