//! Parse error types.
//!
//! Provides structured error types for the parser with:
//! - Rich error variants capturing context
//! - Contextual hints for common mistakes
//! - Related location tracking for better diagnostics
//! - `ErrorContext` for Elm-style "while parsing X" messages
//!
//! # Error Construction Paths
//!
//! Two construction paths coexist:
//! - **`ParseError::new()`** — 87 call sites; simple (code, message, span) errors.
//! - **`ParseError::from_kind()`** — 8 call sites; rich structured errors via
//!   `ParseErrorKind` with title, empathetic message, hint, and educational note.
//!
//! New error sites should prefer `from_kind()`. Migration of existing `new()` sites
//! to `from_kind()` is a future feature task, not a hygiene issue.

mod context;
pub(crate) mod details;
mod kind;
pub(crate) mod mistakes;
mod parse_error;
mod warning;

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;

pub use context::ErrorContext;
pub use kind::ParseErrorKind;
#[allow(
    unused_imports,
    reason = "public API of ParseErrorKind variants; used by tests and future consumers"
)]
pub use kind::{ExprPosition, IdentContext, PatternArgError, PatternContext};
#[allow(
    unused_imports,
    reason = "public API for error token detection; used by tests and parse_error submodule"
)]
pub use mistakes::{check_common_keyword_mistake, detect_common_mistake};
pub use parse_error::ParseError;
pub use warning::{DetachmentReason, ParseWarning};
