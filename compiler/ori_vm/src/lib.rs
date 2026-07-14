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
mod verify;

pub use bytecode::{BytecodeMetrics, BytecodeProgram, TableKind, VerifiedProgram};
pub use compile::{compile, compile_with_options, CompileOptions};
pub use error::{
    ArcInstructionKind, CompileError, ExecutionError, IndexKind, ValueKind, VerifyError,
};
pub use execute::{execute, ExecutionConfig, ExecutionMetrics, ExecutionOutcome, ExitValue};
pub use verify::verify;
