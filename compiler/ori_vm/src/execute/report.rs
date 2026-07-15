//! Stable execution configuration, outcomes, and resource metrics.

use crate::ExecutionError;

use super::value::ExitValue;

/// Deterministic resource limits for one VM session.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionConfig {
    /// Maximum number of dispatched bytecode instructions.
    pub max_steps: u64,
    /// Maximum number of simultaneously active function frames.
    pub max_frames: usize,
    /// Maximum number of live reference-counted heap objects.
    pub max_heap_objects: usize,
    /// Maximum elements in any list or builder and bytes in any VM string.
    pub max_collection_elements: usize,
    /// Maximum simultaneously live aggregate and iterator arena entries.
    pub max_value_arena_entries: usize,
    /// Maximum bytes captured from interpreted print operations.
    pub max_output_bytes: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_steps: 2_000_000_000,
            max_frames: 1_000_000,
            max_heap_objects: 10_000_000,
            max_collection_elements: 100_000_000,
            max_value_arena_entries: 1_000_000,
            max_output_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Runtime metrics from one isolated execution session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionMetrics {
    /// Number of dispatched bytecode instructions.
    pub steps: u64,
    /// Peak number of active call frames.
    pub peak_frames: usize,
    /// Number of successful logical VM heap-object allocations.
    pub cumulative_heap_allocations: u64,
    /// Lower bound on payload capacity bytes acquired by VM heap objects.
    ///
    /// This includes capacity retained at object creation and later capacity
    /// growth. It excludes allocator metadata and transient host temporaries.
    pub cumulative_heap_payload_bytes_lower_bound: u128,
    /// Live heap objects when bytecode execution stopped, before session cleanup.
    pub exit_live_heap_objects: usize,
    /// Owned heap payload capacity when execution stopped, before cleanup.
    pub exit_live_heap_payload_bytes: usize,
    /// Live heap objects after explicit session cleanup.
    pub final_live_heap_objects: usize,
    /// Owned heap payload capacity after explicit session cleanup.
    pub final_live_heap_payload_bytes: usize,
    /// Peak number of live heap objects.
    pub peak_heap_objects: usize,
    /// Peak simultaneously live heap-object payload capacity.
    pub peak_heap_payload_bytes: usize,
    /// Peak backing storage retained by the heap slot and free-list tables.
    pub peak_heap_table_owned_bytes: usize,
    /// Number of aggregate and iterator entries allocated by the value arena.
    pub cumulative_value_arena_allocations: u64,
    /// Number of aggregate entries allocated by the value arena.
    pub cumulative_value_arena_aggregate_allocations: u64,
    /// Number of iterator entries allocated by the value arena.
    pub cumulative_value_arena_iterator_allocations: u64,
    /// Number of tracing collections performed by the value arena.
    pub cumulative_value_arena_collections: u64,
    /// Number of unreachable entries reclaimed by tracing collections.
    pub cumulative_value_arena_reclaimed_entries: u64,
    /// Number of allocations served by generation-safe slot reuse.
    pub cumulative_value_arena_reused_entries: u64,
    /// Live value-arena entries when bytecode execution stopped.
    pub exit_value_arena_entries: usize,
    /// Physical value-arena slots retained when bytecode execution stopped.
    pub exit_value_arena_slots: usize,
    /// Value-arena backing storage retained when bytecode execution stopped.
    pub exit_value_arena_owned_bytes: usize,
    /// Live value-arena entries retained after explicit session cleanup.
    pub final_value_arena_entries: usize,
    /// Physical value-arena slots retained after explicit session cleanup.
    pub final_value_arena_slots: usize,
    /// Value-arena backing storage retained after explicit session cleanup.
    pub final_value_arena_owned_bytes: usize,
    /// Peak number of simultaneously live value-arena entries.
    pub peak_value_arena_entries: usize,
    /// Peak number of physical value-arena slots.
    pub peak_value_arena_slots: usize,
    /// Peak backing storage retained by the value arena and its GC scratch.
    pub peak_value_arena_owned_bytes: usize,
    /// Peak backing storage retained by the frame table.
    ///
    /// Register and parallel-move scratch storage are reported separately.
    pub peak_frame_owned_bytes: usize,
    /// Peak backing storage retained by frame register files.
    pub peak_register_owned_bytes: usize,
    /// Peak backing storage retained for parallel-move scratch values.
    pub peak_scratch_owned_bytes: usize,
    /// Peak backing storage retained by transitive ownership traversal.
    pub peak_ownership_scratch_owned_bytes: usize,
    /// Peak backing storage retained by captured interpreted output.
    pub peak_output_owned_bytes: usize,
}

/// Observable result of a successful interpreted run.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionOutcome {
    /// Materialized return value of `main`.
    pub value: ExitValue,
    /// Bytes emitted by interpreted print operations.
    pub output: Vec<u8>,
    /// Deterministic execution metrics.
    pub metrics: ExecutionMetrics,
}

/// Complete report from one interpreted session, including failed runs.
#[derive(Debug)]
pub struct ExecutionReport {
    /// Materialized return value or the execution failure that stopped the VM.
    pub result: Result<ExitValue, ExecutionError>,
    /// Bytes emitted before success or failure.
    pub output: Vec<u8>,
    /// Metrics captured after deterministic session cleanup.
    pub metrics: ExecutionMetrics,
}
