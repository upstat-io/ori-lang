//! opt-stats analog: cause-cluster ranking of surviving RC ops.
//!
//! Groups the surviving (non-eliminated) RC ops by their survival cause
//! `(lattice_dim, proof_failure)` and ranks the clusters by population — the
//! aims-burden proof-failure worklist (LLVM `opt-stats.py` analog). The biggest
//! cluster is the highest-leverage proof failure to attack first.

use std::collections::BTreeMap;

use crate::Stream;

/// Sentinel for a remark with no cause / no lattice-dim attribution.
const NONE_KEY: &str = "<none>";

/// One ranked cause cluster: a `(lattice_dim, proof_failure)` group + its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CauseCluster {
    /// The blocking lattice dimension (`<none>` when unattributed).
    pub lattice_dim: String,
    /// The proof-failure clause (`<none>` when the remark carried no cause).
    pub proof_failure: String,
    /// Number of surviving RC ops in this cluster.
    pub count: usize,
}

/// Rank survival causes by population, biggest first.
///
/// Ties break on `(lattice_dim, proof_failure)` ascending for determinism, so
/// the ranking is byte-stable across runs of the same stream.
#[must_use]
pub fn rank_cause_clusters(stream: &Stream) -> Vec<CauseCluster> {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for remark in &stream.remarks {
        let (dim, proof_failure) = match &remark.cause {
            Some(cause) => (
                cause.lattice_dim.clone().unwrap_or_else(|| NONE_KEY.to_string()),
                cause.proof_failure.clone(),
            ),
            None => (NONE_KEY.to_string(), NONE_KEY.to_string()),
        };
        *counts.entry((dim, proof_failure)).or_default() += 1;
    }

    let mut clusters: Vec<CauseCluster> = counts
        .into_iter()
        .map(|((lattice_dim, proof_failure), count)| CauseCluster {
            lattice_dim,
            proof_failure,
            count,
        })
        .collect();
    clusters.sort_by(|a, b| {
        b.count.cmp(&a.count).then_with(|| {
            (a.lattice_dim.as_str(), a.proof_failure.as_str())
                .cmp(&(b.lattice_dim.as_str(), b.proof_failure.as_str()))
        })
    });
    clusters
}

#[cfg(test)]
mod tests;
