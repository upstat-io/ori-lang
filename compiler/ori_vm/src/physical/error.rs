//! Typed failures at the physical-plan boundary.

use crate::bytecode::TableKind;

/// Release-gated invariant violated by an internally constructed layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PhysicalLayoutViolation {
    /// The physical lane table does not cover every canonical register.
    #[error("physical layout has {actual} lanes, expected {expected} canonical register lanes")]
    LaneCount { expected: usize, actual: usize },
    /// A physical lane does not retain its canonical register identity.
    #[error("physical lane {lane} stores identity {actual}, expected {expected}")]
    LaneIdentity {
        lane: usize,
        expected: usize,
        actual: usize,
    },
    /// The reverse map does not contain one entry for every canonical operation.
    #[error("physical layout covers {actual} canonical operations, expected {expected}")]
    CanonicalOpCount { expected: usize, actual: usize },
    /// A physical executor entry owns no canonical instruction.
    #[error("physical operation {physical_op} owns an empty canonical-PC span")]
    EmptyCanonicalSpan { physical_op: usize },
    /// Physical entries do not cover canonical PCs in contiguous order.
    #[error(
        "physical operation {physical_op} starts at canonical PC {actual_start}, expected {expected_start}"
    )]
    CanonicalSpanOrder {
        physical_op: usize,
        expected_start: usize,
        actual_start: usize,
    },
    /// A physical entry's canonical span exceeds the canonical operation table.
    #[error(
        "physical operation {physical_op} ends at canonical PC {end}, beyond {canonical_pcs} canonical PCs"
    )]
    CanonicalSpanBounds {
        physical_op: usize,
        end: usize,
        canonical_pcs: usize,
    },
    /// A canonical PC's reverse owner does not round-trip to its physical entry.
    #[error(
        "canonical PC {canonical_pc} maps to physical operation {actual_owner}, expected {expected_owner}"
    )]
    ReverseOwner {
        canonical_pc: usize,
        expected_owner: usize,
        actual_owner: usize,
    },
    /// A canonical PC's offset does not round-trip within its owner span.
    #[error(
        "canonical PC {canonical_pc} has owner offset {actual_offset}, expected {expected_offset}"
    )]
    ReverseOffset {
        canonical_pc: usize,
        expected_offset: usize,
        actual_offset: usize,
    },
    /// A flattened read span does not begin after the preceding canonical PC.
    #[error(
        "canonical PC {canonical_pc} read bindings start at {actual_start}, expected {expected_start}"
    )]
    ReadSpanOrder {
        canonical_pc: usize,
        expected_start: usize,
        actual_start: usize,
    },
    /// A flattened write span does not begin after the preceding canonical PC.
    #[error(
        "canonical PC {canonical_pc} write bindings start at {actual_start}, expected {expected_start}"
    )]
    WriteSpanOrder {
        canonical_pc: usize,
        expected_start: usize,
        actual_start: usize,
    },
    /// A canonical PC's flattened read span exceeds the read table.
    #[error(
        "canonical PC {canonical_pc} read span ends at {end}, beyond {read_bindings} read bindings"
    )]
    ReadSpanBounds {
        canonical_pc: usize,
        end: usize,
        read_bindings: usize,
    },
    /// A canonical PC's flattened read span overflows the host index type.
    #[error("canonical PC {canonical_pc} read span overflows the host index type")]
    ReadSpanOverflow { canonical_pc: usize },
    /// A canonical PC's flattened write span exceeds the write table.
    #[error(
        "canonical PC {canonical_pc} write span ends at {end}, beyond {write_bindings} write bindings"
    )]
    WriteSpanBounds {
        canonical_pc: usize,
        end: usize,
        write_bindings: usize,
    },
    /// A canonical PC's flattened write span overflows the host index type.
    #[error("canonical PC {canonical_pc} write span overflows the host index type")]
    WriteSpanOverflow { canonical_pc: usize },
    /// A physical read span has a different arity than its canonical operation.
    #[error(
        "canonical PC {canonical_pc} exposes {actual} physical reads, but its bytecode defines {expected}"
    )]
    ReadOperandCount {
        canonical_pc: usize,
        expected: usize,
        actual: usize,
    },
    /// A physical write span has a different arity than its canonical operation.
    #[error(
        "canonical PC {canonical_pc} exposes {actual} physical writes, but its bytecode defines {expected}"
    )]
    WriteOperandCount {
        canonical_pc: usize,
        expected: usize,
        actual: usize,
    },
    /// A lane-backed read does not name its canonical source register.
    #[error(
        "canonical PC {canonical_pc} read operand {operand} does not map to its canonical register"
    )]
    ReadBindingMismatch { canonical_pc: usize, operand: usize },
    /// An immediate read has no exact value-and-kind proof at its canonical PC.
    #[error(
        "canonical PC {canonical_pc} read operand {operand} has no exact value-and-kind proof for its immediate"
    )]
    ImmediateProofMismatch { canonical_pc: usize, operand: usize },
    /// A physical destination does not name its canonical destination register.
    #[error(
        "canonical PC {canonical_pc} write operand {operand} does not map to its canonical register"
    )]
    WriteBindingMismatch { canonical_pc: usize, operand: usize },
    /// A copy-noop flag disagrees with canonical source/destination identity.
    #[error(
        "canonical PC {canonical_pc} marks copy-noop as {actual}, but canonical identity proves {expected}"
    )]
    CopyNoopProofMismatch {
        canonical_pc: usize,
        expected: bool,
        actual: bool,
    },
    /// A lane-backed read references storage outside the physical frame.
    #[error(
        "canonical PC {canonical_pc} read operand {operand} references lane {lane}, beyond {physical_lanes} physical lanes"
    )]
    ReadLaneBounds {
        canonical_pc: usize,
        operand: usize,
        lane: usize,
        physical_lanes: usize,
    },
    /// A destination references storage outside the physical frame.
    #[error(
        "canonical PC {canonical_pc} write operand {operand} references lane {lane}, beyond {physical_lanes} physical lanes"
    )]
    WriteLaneBounds {
        canonical_pc: usize,
        operand: usize,
        lane: usize,
        physical_lanes: usize,
    },
    /// Physical executor spans leave canonical PCs uncovered.
    #[error("physical operations cover {covered} of {canonical_pcs} canonical PCs")]
    CanonicalCoverage {
        covered: usize,
        canonical_pcs: usize,
    },
    /// Physical read spans leave flattened read bindings uncovered.
    #[error("physical read spans cover {covered} of {read_bindings} read bindings")]
    ReadCoverage {
        covered: usize,
        read_bindings: usize,
    },
    /// Physical write spans leave flattened write bindings uncovered.
    #[error("physical write spans cover {covered} of {write_bindings} write bindings")]
    WriteCoverage {
        covered: usize,
        write_bindings: usize,
    },
}

/// Failure while preparing or opening a physical plan over verified bytecode.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PrepareError {
    /// The physical plan and verified program expose different function sets.
    #[error("physical plan covers {actual} functions, expected {expected}")]
    FunctionCountMismatch {
        /// Canonical verified function count.
        expected: usize,
        /// Physical function-plan count.
        actual: usize,
    },
    /// A function needs more physical lanes than the compact identity permits.
    #[error(
        "function {function} requires {logical_lanes} VM lanes, exceeding the physical-plan limit"
    )]
    LaneCountOverflow {
        /// Zero-based function index.
        function: usize,
        /// Canonical logical lanes requested by the function.
        logical_lanes: usize,
    },
    /// A function needs more physical entries than the compact identity permits.
    #[error(
        "function {function} requires {physical_ops} physical operations, exceeding the physical-plan limit"
    )]
    PhysicalOpCountOverflow {
        /// Zero-based function index.
        function: usize,
        /// Physical entries requested by the function.
        physical_ops: usize,
    },
    /// A function's read bindings cannot fit the compact span identity.
    #[error(
        "function {function} requires {read_bindings} physical read bindings, exceeding the physical-plan limit"
    )]
    ReadBindingCountOverflow {
        /// Zero-based function index.
        function: usize,
        /// Flattened read bindings requested by the function.
        read_bindings: usize,
    },
    /// A function's write bindings cannot fit the compact span identity.
    #[error(
        "function {function} requires {write_bindings} physical write bindings, exceeding the physical-plan limit"
    )]
    WriteBindingCountOverflow {
        /// Zero-based function index.
        function: usize,
        /// Flattened write bindings requested by the function.
        write_bindings: usize,
    },
    /// Verified bytecode unexpectedly contains an invalid operand table identity.
    #[error(
        "verified function {function} at canonical PC {canonical_pc} references invalid {table:?} table index {index}; bound is {bound}"
    )]
    CanonicalOperandTable {
        /// Zero-based function index.
        function: usize,
        /// Canonical PC containing the reference.
        canonical_pc: usize,
        /// Referenced side-table category.
        table: TableKind,
        /// Invalid table index.
        index: usize,
        /// Referenced table's exclusive upper bound.
        bound: usize,
    },
    /// A constructed physical function does not faithfully project bytecode.
    #[error("function {function} produced an invalid physical layout: {violation}")]
    InvalidLayout {
        /// Zero-based function index.
        function: usize,
        /// Exact rejected invariant.
        violation: PhysicalLayoutViolation,
    },
    /// A per-function metrics query names no physical function.
    #[error(
        "physical-plan function index {function} is out of bounds for {function_count} functions"
    )]
    FunctionIndexOutOfBounds {
        /// Requested function index.
        function: usize,
        /// Number of functions in the plan.
        function_count: usize,
    },
    /// A whole-plan metric cannot be represented exactly on this host.
    #[error("physical-plan {metric} exceeds this host's addressable size; split the program into smaller modules")]
    MetricOverflow {
        /// Count or byte subtotal whose addition overflowed.
        metric: &'static str,
    },
    /// One function's retained payload cannot be represented exactly.
    #[error("function {function} physical-plan {table} storage exceeds this host's addressable size; split the function into smaller functions")]
    FunctionStorageSizeOverflow {
        /// Zero-based function index.
        function: usize,
        /// Table or subtotal whose byte count overflowed.
        table: &'static str,
    },
    /// Planning-scratch payload arithmetic cannot be represented exactly.
    #[error("function {function} planning-scratch {metric} exceeds this host's addressable size")]
    PlanningScratchOverflow {
        /// Zero-based function index.
        function: usize,
        /// Scratch table or subtotal whose byte count overflowed.
        metric: &'static str,
    },
    /// Planning-scratch ownership accounting attempted an invalid release.
    #[error("function {function} planning-scratch {metric} has inconsistent ownership accounting")]
    PlanningScratchAccounting {
        /// Zero-based function index.
        function: usize,
        /// Scratch table whose capacity accounting was inconsistent.
        metric: &'static str,
    },
    /// Planning scratch unexpectedly remains retained after preparation.
    #[error("function {function} retains {bytes} bytes of planning scratch after preparation")]
    PlanningScratchRetained {
        /// Zero-based function index.
        function: usize,
        /// Explicitly tracked scratch payload still live.
        bytes: usize,
    },
    /// Sparse planning did not materialize a boundary for a verified CFG target.
    #[error("function {function} has no sparse planning block at canonical PC {target}")]
    PlanningBlockTargetMissing {
        /// Zero-based function index.
        function: usize,
        /// Canonical control-flow target lacking a block boundary.
        target: usize,
    },
}
