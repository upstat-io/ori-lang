//! Escape analysis types.
//!
//! **Placeholder only in §01** — exports `EscapeInfo` so that
//! `ReprPlan::escape_info` compiles. Replaced in §08 with
//! the full connection graph and escape state framework.

/// Placeholder for per-function escape analysis information.
///
/// Replaced by the full `EscapeInfo` type in §08 (escape analysis).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EscapeInfo;
