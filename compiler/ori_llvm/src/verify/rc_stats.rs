//! Typed JSON schema for RC stats reports.
//!
//! Converts per-block histograms from `rc_histogram` into a typed JSON
//! structure with schema versioning for backward-compatible evolution.
//! All JSON emission goes through [`RcStatsReport::emit_to_stderr`] — the
//! single canonical emission point.

use serde::Serialize;

use super::rc_histogram::{FunctionHistogram, RcOpKind};

/// Schema version for backward compatibility. Bump when adding fields.
pub const SCHEMA_VERSION: u32 = 1;

/// Top-level RC stats report emitted as JSON.
#[derive(Debug, Clone, Serialize)]
pub struct RcStatsReport {
    pub schema_version: u32,
    /// Whether this report covers optimized or unoptimized IR.
    pub optimized: bool,
    pub functions: Vec<FunctionStats>,
}

/// Per-function RC operation stats.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionStats {
    pub name: String,
    pub blocks: Vec<BlockStats>,
    /// Function-level totals (sum of all blocks).
    pub totals: OpCounts,
}

/// Per-basic-block RC operation counts.
#[derive(Debug, Clone, Serialize)]
pub struct BlockStats {
    pub label: String,
    pub counts: OpCounts,
}

/// Raw RC operation counts.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct OpCounts {
    pub alloc: u32,
    pub inc: u32,
    pub dec: u32,
    pub free: u32,
    pub cow: u32,
}

impl RcStatsReport {
    /// Build a report from per-block histograms.
    pub(super) fn from_histograms(histograms: &[FunctionHistogram], optimized: bool) -> Self {
        let functions = histograms
            .iter()
            .map(|fh| {
                let blocks: Vec<BlockStats> = fh
                    .blocks
                    .iter()
                    .map(|bh| BlockStats {
                        label: bh.label.clone(),
                        counts: OpCounts {
                            alloc: bh.counts[RcOpKind::Alloc as usize],
                            inc: bh.counts[RcOpKind::Inc as usize],
                            dec: bh.counts[RcOpKind::Dec as usize],
                            free: bh.counts[RcOpKind::Free as usize],
                            cow: bh.counts[RcOpKind::Cow as usize],
                        },
                    })
                    .collect();

                let totals = blocks.iter().fold(OpCounts::default(), |acc, b| OpCounts {
                    alloc: acc.alloc + b.counts.alloc,
                    inc: acc.inc + b.counts.inc,
                    dec: acc.dec + b.counts.dec,
                    free: acc.free + b.counts.free,
                    cow: acc.cow + b.counts.cow,
                });

                FunctionStats {
                    name: fh.name.clone(),
                    blocks,
                    totals,
                }
            })
            .collect();

        Self {
            schema_version: SCHEMA_VERSION,
            optimized,
            functions,
        }
    }

    /// Emit JSON to stderr with the canonical prefix.
    ///
    /// All hook points (verify/mod.rs, aot/object.rs, build/single.rs) call
    /// this method — prevents algorithmic duplication of the formatting contract.
    /// Prefix `codegen stats: json:` is distinct from `codegen audit:` to avoid
    /// breaking `codegen-audit.sh`.
    pub fn emit_to_stderr(&self) {
        eprintln!(
            "codegen stats: json: {}",
            serde_json::to_string(self).expect("RcStatsReport serialization")
        );
    }
}

#[cfg(test)]
mod tests;
