//! RC operation counting for pipeline comparison.
//!
//! Counts `RcInc` and `RcDec` instructions in `ArcFunction`s to enable
//! quantitative comparison between AIMS and legacy pipelines.

use crate::ir::{ArcFunction, ArcInstr};

/// RC operation counts for a single function.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RcOpCount {
    /// Total `RcInc` operations (summing batched counts).
    pub inc: usize,
    /// Total `RcDec` operations.
    pub dec: usize,
}

impl RcOpCount {
    /// Total RC operations (`inc + dec`).
    pub fn total(self) -> usize {
        self.inc + self.dec
    }
}

/// Count RC operations in a single function.
///
/// Counts `RcInc` (summing batched `count` fields) and `RcDec` instructions
/// across all basic blocks.
pub(crate) fn count_rc_ops(func: &ArcFunction) -> RcOpCount {
    let mut result = RcOpCount::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { count, .. } => result.inc += *count as usize,
                ArcInstr::RcDec { .. } => result.dec += 1,
                _ => {}
            }
        }
    }
    result
}

impl std::fmt::Display for RcOpCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}inc/{}dec ({}total)", self.inc, self.dec, self.total())
    }
}

#[cfg(test)]
mod tests;
