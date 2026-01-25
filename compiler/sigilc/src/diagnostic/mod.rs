//! Diagnostic system for rich error reporting.
//!
//! This module re-exports types from `sigil_diagnostic` and provides
//! sigilc-specific extensions.
//!
//! Per design spec 02-design-principles.md:
//! - Error codes for searchability
//! - Clear messages (what went wrong)
//! - Primary span (where it went wrong)
//! - Context labels (why it's wrong)
//! - Suggestions (how to fix)

// Re-export core diagnostic types from sigil_diagnostic
pub use sigil_diagnostic::{
    Applicability, Diagnostic, ErrorCode, Label, Severity, Substitution, Suggestion,
    // Helper functions
    expected_expression, missing_pattern_arg, type_mismatch, unclosed_delimiter,
    unknown_identifier, unknown_pattern_arg,
    // Queue and span utilities
    queue, span_utils,
};

// Re-export emitter and fixes submodules from sigil_diagnostic
pub use sigil_diagnostic::emitter;
pub use sigil_diagnostic::fixes;
