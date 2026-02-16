//! Context-aware type error infrastructure.
//!
//! This module provides Elm-quality error messages with rich context tracking:
//!
//! - [`Expected`]: Tracks WHY we expect a type (annotation, context, previous element)
//! - [`ContextKind`]: Classifies WHERE in code a type is expected (30+ contexts)
//! - [`TypeProblem`]: Identifies WHAT went wrong specifically (not just "mismatch")
//! - [`Suggestion`]: Provides actionable fixes
//! - [`TypeCheckError`]: The comprehensive error type with full context
//!
//! # Example Error Flow
//!
//! ```text
//! 1. During inference: Track Expected { ty: int, origin: Annotation("x", line 5) }
//! 2. At mismatch: Create TypeProblem::IntFloat (specific problem identified)
//! 3. Generate Suggestion: "use `int(x)` to convert"
//! 4. Build TypeCheckError with full context for rendering
//! ```
//!
//! # Design
//!
//! Based on patterns from:
//! - **Elm**: `Expected` with origin tracking, ordinal formatting ("2nd argument")
//! - **Gleam**: `UnifyErrorSituation` for context-specific errors
//! - **Rust**: `TypeFlags` for fast checks, structured `Diagnostic` output

mod check_error;
mod context;
mod diff;
mod expected;
mod problem;
mod suggest;

pub use check_error::{
    ArityMismatchKind, ErrorContext, ImportErrorKind, TypeCheckError, TypeErrorKind,
};
pub use context::ContextKind;
pub use diff::{diff_types, edit_distance, find_closest_field, suggest_field_typo};
pub use expected::{Expected, ExpectedOrigin, SequenceKind};
pub use problem::{Severity, TypeProblem};
// Suggestion is re-exported from ori_diagnostic (unified type).
