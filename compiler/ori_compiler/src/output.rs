//! Result types for the portable compiler pipeline.
//!
//! These types are the public interface between the compiler driver and its
//! consumers (playground WASM, CLI, tests). They carry all information needed
//! to present results without exposing internal compiler types.

use ori_diagnostic::Diagnostic;

/// Which compilation phase produced the error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorPhase {
    /// Parse errors (syntax).
    Parse,
    /// Type checking errors (semantic).
    Type,
    /// Runtime evaluation errors.
    Runtime,
}

/// Result of compiling and running Ori source code.
#[derive(Clone, Debug)]
pub struct CompileOutput {
    /// Whether execution completed without errors.
    pub success: bool,
    /// Formatted return value (empty for void).
    pub output: String,
    /// Captured `print`/`println` output.
    pub printed: String,
    /// Diagnostics from any phase.
    pub diagnostics: Vec<Diagnostic>,
    /// Which phase produced the error (if any).
    pub error_phase: Option<ErrorPhase>,
}

/// Result of formatting Ori source code.
#[derive(Clone, Debug)]
pub struct FormatOutput {
    /// Whether formatting completed without errors.
    pub success: bool,
    /// Formatted source code (if successful).
    pub formatted: Option<String>,
    /// Parse diagnostics (if formatting failed).
    pub diagnostics: Vec<Diagnostic>,
}
