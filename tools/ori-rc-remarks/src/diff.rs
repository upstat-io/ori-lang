//! opt-diff analog: two-build remark comparison.
//!
//! Compares a baseline stream against a candidate stream, keyed on the remark
//! identity tuple `(pass, name, function, debug_loc, ssa_value)` (LLVM
//! `opt-diff.py` analog). A remark present in the candidate but not the baseline
//! is a regression (a new surviving RC op); one present in the baseline but not
//! the candidate is an improvement (a now-eliminated RC op).

use std::collections::HashSet;

use crate::{Remark, Stream};

/// The identity of a remark for cross-build comparison.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemarkKey {
    /// Emitting pass.
    pub pass: String,
    /// Remark name.
    pub name: String,
    /// Enclosing function (the function-level fallback when `debug_loc` is None).
    pub function: Option<String>,
    /// Resolved source location `(file, line, column)`, when present.
    pub debug_loc: Option<(String, u32, u32)>,
    /// SSA value id.
    pub ssa_value: Option<u64>,
}

impl RemarkKey {
    /// Project a remark onto its cross-build identity.
    #[must_use]
    pub fn of(remark: &Remark) -> Self {
        Self {
            pass: remark.pass.clone(),
            name: remark.name.clone(),
            function: remark.function.clone(),
            debug_loc: remark
                .debug_loc
                .as_ref()
                .map(|loc| (loc.file.clone(), loc.line, loc.column)),
            ssa_value: remark.ssa_value,
        }
    }
}

/// The result of comparing two streams.
#[derive(Debug, Clone, Default)]
pub struct StreamDiff {
    /// Remarks new in the candidate (regressions — new surviving RC ops), in
    /// candidate stream order, deduplicated by key.
    pub added: Vec<Remark>,
    /// Remarks gone from the baseline (improvements — eliminated RC ops), in
    /// baseline stream order, deduplicated by key.
    pub removed: Vec<Remark>,
    /// Count of distinct keys present in both streams.
    pub persisted: usize,
}

/// Diff `candidate` against `baseline`, keyed on [`RemarkKey`].
#[must_use]
pub fn diff_streams(baseline: &Stream, candidate: &Stream) -> StreamDiff {
    let base_keys: HashSet<RemarkKey> = baseline.remarks.iter().map(RemarkKey::of).collect();
    let cand_keys: HashSet<RemarkKey> = candidate.remarks.iter().map(RemarkKey::of).collect();

    let mut diff = StreamDiff::default();

    let mut seen_added: HashSet<RemarkKey> = HashSet::new();
    for remark in &candidate.remarks {
        let key = RemarkKey::of(remark);
        if !base_keys.contains(&key) && seen_added.insert(key) {
            diff.added.push(remark.clone());
        }
    }

    let mut seen_removed: HashSet<RemarkKey> = HashSet::new();
    for remark in &baseline.remarks {
        let key = RemarkKey::of(remark);
        if !cand_keys.contains(&key) && seen_removed.insert(key) {
            diff.removed.push(remark.clone());
        }
    }

    diff.persisted = base_keys.intersection(&cand_keys).count();
    diff
}

#[cfg(test)]
mod tests;
