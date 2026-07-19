//! opt-viewer analog: source-annotated survivor view.
//!
//! Groups surviving RC ops by source location (LLVM `opt-viewer.py` analog).
//! When a remark carries a resolved `debug_loc`, the group key is
//! `file:line:column`; when the span is absent (synthetic RC ops — the common
//! case until per-op span propagation lands), the view falls back to
//! function-level grouping (`fn <name>`). Groups are location-sorted for stable
//! output; survivors stay in stream order within a group.

use std::collections::BTreeMap;

use crate::Stream;

/// One survivor rendered for the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurvivorLine {
    /// The surviving burden op (e.g. `burden_inc`).
    pub rc_op: String,
    /// SSA value id of the surviving var.
    pub ssa_value: Option<u64>,
    /// Blocking lattice dimension, when attributed.
    pub lattice_dim: Option<String>,
    /// Proof-failure clause, when attributed.
    pub proof_failure: Option<String>,
}

/// A location group: a source site (or function fallback) + its survivors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewGroup {
    /// `file:line:column` when a span resolved; else `fn <name>` / `fn <unknown>`.
    pub location: String,
    /// The survivors at this location, in stream order.
    pub survivors: Vec<SurvivorLine>,
}

/// Build the source-annotated survivor view, grouped by location.
#[must_use]
pub fn source_annotated_view(stream: &Stream) -> Vec<ViewGroup> {
    let mut groups: BTreeMap<String, Vec<SurvivorLine>> = BTreeMap::new();
    for remark in &stream.remarks {
        let location = match &remark.debug_loc {
            Some(loc) => format!("{}:{}:{}", loc.file, loc.line, loc.column),
            None => match &remark.function {
                Some(function) => format!("fn {function}"),
                None => "fn <unknown>".to_string(),
            },
        };
        let (lattice_dim, proof_failure) = match &remark.cause {
            Some(cause) => (cause.lattice_dim.clone(), Some(cause.proof_failure.clone())),
            None => (None, None),
        };
        groups.entry(location).or_default().push(SurvivorLine {
            rc_op: remark.rc_op.clone(),
            ssa_value: remark.ssa_value,
            lattice_dim,
            proof_failure,
        });
    }
    groups
        .into_iter()
        .map(|(location, survivors)| ViewGroup { location, survivors })
        .collect()
}

#[cfg(test)]
mod tests;
