//! Diagnostic system for rich error reporting.
//!
//! Per design spec 02-design-principlesmd:
//! - Error codes for searchability
//! - Clear messages (what went wrong)
//! - Primary span (where it went wrong)
//! - Context labels (why it's wrong)
//! - Suggestions (how to fix)
//!
//! # Error Guarantees
//!
//! The `ErrorGuaranteed` type provides type-level proof that at least one
//! error was emitted. This prevents "forgotten" error conditions where code
//! fails silently without reporting an error.
//!
//! ```text
//! // Can only get ErrorGuaranteed by emitting an error
//! let guarantee = queue.emit_error(diagnostic);
//!
//! // Functions can return ErrorGuaranteed to prove they reported errors
//! fn type_check() -> Result<TypedModule, ErrorGuaranteed> { ... }
//! ```

mod diagnostic;
pub mod emitter;
mod error_code;
pub mod errors;
pub mod fixes;
mod guarantee;
pub mod queue;
pub mod span_utils;

pub use errors::ErrorDocs;

// Re-export all public types at crate root for backwards compatibility
pub use diagnostic::{
    expected_expression, missing_pattern_arg, type_mismatch, unclosed_delimiter, unexpected_token,
    unknown_identifier, unknown_pattern_arg, Applicability, Diagnostic, Label, Severity,
    Substitution, Suggestion,
};
pub use error_code::ErrorCode;
pub use guarantee::ErrorGuaranteed;
pub use queue::DiagnosticSeverity;
