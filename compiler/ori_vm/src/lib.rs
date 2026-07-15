//! Bytecode compilation, verification, and interpreted execution.
//!
//! The VM accepts only a validated [`ori_repr::executable::ExecutableProgram`].
//! Compilation cannot access frontend queries, and execution accepts only a
//! [`VerifiedProgram`], keeping realization and runtime state on opposite sides
//! of explicit immutable boundaries.

mod bytecode;
mod compile;
mod error;
mod execute;
mod physical;
mod verify;

pub use bytecode::{BytecodeMetrics, BytecodeProgram, OpcodeKind, TableKind, VerifiedProgram};
pub use compile::{compile, compile_with_options, CompileOptions};
pub use error::{
    ArcInstructionKind, CompileError, ExecutionError, IndexKind, ValueKind, VerifyError,
};
pub use execute::{
    execute, execute_physical_profiled, execute_physical_report, execute_profiled, execute_report,
    ExecutionConfig, ExecutionMetrics, ExecutionOutcome, ExecutionProfile, ExecutionReport,
    ExitValue, OpcodeCount, OpcodePairCount, ProfileFunctionId, ProfilePc, ProfiledExecutionReport,
    RegionCount,
};
pub use physical::{
    prepare, PhysicalElementSizes, PhysicalFunctionStorageMetrics, PhysicalLayoutViolation,
    PhysicalOptions, PhysicalPlanMetrics, PhysicalVmPlan, PrepareError,
};
pub use verify::verify;
