//! Diagnostic Rendering
//!
//! Converts structured [`super::problem`] types into user-facing Diagnostic
//! messages. This separates the "what went wrong" (Problem) from "how to
//! display it" (Diagnostic).
//!
//! # Design
//!
//! Each problem type has an `into_diagnostic()` method that converts it to
//! a `Diagnostic` with:
//! - Error code for searchability
//! - Clear message explaining what went wrong
//! - Labeled spans showing where
//! - Notes providing context
//! - Suggestions for how to fix
//!
//! Type errors use `TypeErrorRenderer` for Pool-aware type name rendering.
//!
//! Parse errors are rendered directly by `ori_parse::ParseError::to_queued_diagnostic()`
//! and do not flow through this module.

pub mod typeck;

#[cfg(test)]
mod tests;
