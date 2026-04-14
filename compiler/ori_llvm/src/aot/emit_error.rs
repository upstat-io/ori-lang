//! Error types for code emission and the verify → optimize → emit pipeline.

use std::fmt;

use super::passes::OptimizationError;
use super::target_features::TargetError;

/// Error type for object file emission operations.
#[derive(Debug, Clone)]
pub enum EmitError {
    /// Failed to create target machine.
    TargetMachine(TargetError),
    /// Failed to configure module with target settings.
    ModuleConfiguration(TargetError),
    /// Failed to emit object file.
    ObjectEmission { path: String, message: String },
    /// Failed to emit assembly file.
    AssemblyEmission { path: String, message: String },
    /// Failed to emit LLVM bitcode.
    BitcodeEmission { path: String, message: String },
    /// Failed to emit LLVM IR text.
    LlvmIrEmission { path: String, message: String },
    /// Output path is not valid.
    InvalidPath { path: String, reason: String },
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMachine(err) => {
                write!(f, "failed to create target machine: {err}")
            }
            Self::ModuleConfiguration(err) => {
                write!(f, "failed to configure module: {err}")
            }
            Self::ObjectEmission { path, message } => {
                write!(f, "failed to emit object file '{path}': {message}")
            }
            Self::AssemblyEmission { path, message } => {
                write!(f, "failed to emit assembly file '{path}': {message}")
            }
            Self::BitcodeEmission { path, message } => {
                write!(f, "failed to emit bitcode file '{path}': {message}")
            }
            Self::LlvmIrEmission { path, message } => {
                write!(f, "failed to emit LLVM IR file '{path}': {message}")
            }
            Self::InvalidPath { path, reason } => {
                write!(f, "invalid output path '{path}': {reason}")
            }
        }
    }
}

impl std::error::Error for EmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TargetMachine(err) | Self::ModuleConfiguration(err) => Some(err),
            _ => None,
        }
    }
}

impl From<TargetError> for EmitError {
    fn from(err: TargetError) -> Self {
        Self::TargetMachine(err)
    }
}

/// Error type for the full verify → optimize → emit pipeline.
///
/// Wraps the individual error types from each pipeline stage.
#[derive(Debug, Clone)]
pub enum ModulePipelineError {
    /// LLVM IR verification failed (compiler bug).
    Verification(String),
    /// Optimization pass pipeline failed.
    Optimization(OptimizationError),
    /// Object/bitcode/IR emission failed.
    Emission(EmitError),
    /// IR capture hook failed (e.g., writing pre/post-opt IR for Alive2).
    Capture(String),
}

impl fmt::Display for ModulePipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification(msg) => write!(f, "LLVM IR verification failed: {msg}"),
            Self::Optimization(err) => write!(f, "optimization failed: {err}"),
            Self::Emission(err) => write!(f, "emission failed: {err}"),
            Self::Capture(msg) => write!(f, "IR capture failed: {msg}"),
        }
    }
}

impl std::error::Error for ModulePipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Verification(_) | Self::Capture(_) => None,
            Self::Optimization(err) => Some(err),
            Self::Emission(err) => Some(err),
        }
    }
}
