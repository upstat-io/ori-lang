//! Diagnostic system for rich error reporting.
//!
//! This module re-exports types from `ori_diagnostic` and provides
//! oric-specific extensions.
//!
//! Per design spec 02-design-principles.md:
//! - Error codes for searchability
//! - Clear messages (what went wrong)
//! - Primary span (where it went wrong)
//! - Context labels (why it's wrong)
//! - Suggestions (how to fix)

// Re-export core diagnostic types from ori_diagnostic
pub use ori_diagnostic::{
    // Helper functions
    expected_expression,
    missing_pattern_arg,
    // Queue and span utilities
    queue,
    span_utils,
    type_mismatch,
    unclosed_delimiter,
    unknown_identifier,
    unknown_pattern_arg,
    Applicability,
    Diagnostic,
    ErrorCode,
    Label,
    Severity,
    Substitution,
    Suggestion,
};

// Re-export emitter and fixes submodules from ori_diagnostic
pub use ori_diagnostic::emitter;
pub use ori_diagnostic::fixes;
