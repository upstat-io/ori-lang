//! Value range analysis types.
//!
//! **Placeholder only in §01** — exports `ValueRange` so that
//! `DecisionReason::RangeFits` compiles. Replaced in §03 with
//! the full interval lattice and transfer function framework.

/// Placeholder for value range information.
///
/// Replaced by the full `ValueRange` interval type in §03 (range analysis).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValueRange;
