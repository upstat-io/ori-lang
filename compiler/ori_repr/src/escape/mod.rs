//! Escape analysis types.
//!
//! **Placeholder** — exports `EscapeInfo` so that
//! `ReprPlan::escape_info` compiles. Replaced when escape analysis
//! is implemented with the full connection graph and escape state framework.

/// Placeholder for per-function escape analysis information.
///
/// Replaced by the full `EscapeInfo` type when escape analysis is implemented.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EscapeInfo;
