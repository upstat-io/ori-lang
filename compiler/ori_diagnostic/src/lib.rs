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
//! ```ignore
//! // Can only get ErrorGuaranteed by emitting an error
//! let guarantee = queue.emit_error(diagnostic);
//!
//! // Functions can return ErrorGuaranteed to prove they reported errors
//! fn type_check() -> Result<TypedModule, ErrorGuaranteed> { ... }
//! ```

pub mod emitter;
mod diagnostic;
mod error_code;
mod guarantee;
pub mod errors;
pub mod fixes;
pub mod queue;
pub mod span_utils;

pub use errors::ErrorDocs;

// Re-export all public types at crate root for backwards compatibility
pub use error_code::ErrorCode;
pub use diagnostic::{
    Applicability, Diagnostic, Label, Severity, Substitution, Suggestion,
    type_mismatch, unexpected_token, expected_expression, unclosed_delimiter,
    unknown_identifier, missing_pattern_arg, unknown_pattern_arg,
};
pub use guarantee::ErrorGuaranteed;
