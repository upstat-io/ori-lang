//! Read-only consumer-edge attribution for the provenance DAG.
//!
//! Walks the drop-descriptor tree and attributes each generated per-type
//! drop-glue symbol (`_ori_drop$<idx>`) back to the `Idx` chain that produced
//! it. Diagnostic-only — consumes the [`super::compute_drop_info`] descriptor
//! and the [`super::drop_glue_symbol`] naming SSOT; never alters drop emission.

use rustc_hash::FxHashSet;

use ori_types::{ConsumerEdge, Idx, Pool};

use crate::ArcClassification;

use super::{compute_drop_info, drop_glue_symbol, drop_info_child_types};

/// Read-only consumer attribution for the provenance DAG.
///
/// Walks the drop-descriptor tree from `root` (the same root the provenance DAG
/// walks) and records one [`ConsumerEdge`] per type whose refcount-zero
/// teardown generates a per-type drop function (`_ori_drop$<idx>`), attributing
/// that symbol to the `Idx` chain the drop-synthesis walk descended to reach it.
/// Makes an RC-on-scalar emission point at the generic-leaf chain that produced
/// it (the `Wrap#217{inner: T#141}` -> `_ori_drop$141` thru-line).
///
/// Diagnostic-only: consumes [`compute_drop_info`] (a pure function of the
/// frozen pool + classifier) and the [`drop_glue_symbol`] naming SSOT; NEVER
/// alters drop emission. Depth-bounded + cycle-guarded, mirroring the DAG walk;
/// each generated symbol is attributed exactly once (first-discovered chain).
#[must_use]
pub fn compute_consumer_attribution(
    root: Idx,
    classifier: &dyn ArcClassification,
    pool: &Pool,
    max_depth: usize,
) -> Vec<ConsumerEdge> {
    let resolved_root = pool.resolve_fully(root);
    if !pool.is_valid_idx(resolved_root) {
        return Vec::new();
    }
    let mut state = ConsumerAttribution {
        classifier,
        pool,
        max_depth,
        visited: FxHashSet::default(),
        emitted: FxHashSet::default(),
        edges: Vec::new(),
    };
    // The walk records every drop-bearing node it visits (root + each
    // descendant) through one gated path; a non-drop-bearing node (scalar, OR
    // an iterator dispatched inline) is neither recorded nor descended.
    state.walk(root, &mut vec![root], 0);
    state.edges
}

/// Mutable accumulator threaded through the recursive drop-descriptor walk —
/// bundles the pool/classifier borrows, the depth cap, the cycle-guard visited
/// set, the per-symbol dedup set, and the output so the recursion is one
/// `&mut self` method, not a 7-arg helper.
struct ConsumerAttribution<'a> {
    classifier: &'a dyn ArcClassification,
    pool: &'a Pool,
    max_depth: usize,
    visited: FxHashSet<Idx>,
    emitted: FxHashSet<Idx>,
    edges: Vec<ConsumerEdge>,
}

impl ConsumerAttribution<'_> {
    /// Record one consumer edge for `type_idx` (deduped — each generated symbol
    /// is attributed once, on its first-discovered chain).
    fn record(&mut self, type_idx: Idx, chain: &[Idx]) {
        if self.emitted.insert(type_idx) {
            self.edges.push(ConsumerEdge {
                type_idx,
                drop_glue_symbol: drop_glue_symbol(type_idx),
                walked_chain: chain.to_vec(),
            });
        }
    }

    fn walk(&mut self, ty: Idx, chain: &mut Vec<Idx>, depth: usize) {
        if depth >= self.max_depth {
            return;
        }
        // `compute_drop_info` resolves Named/Applied internally; cycle-guard on
        // the resolved index so a recursive type graph terminates.
        let resolved = self.pool.resolve_fully(ty);
        if !self.pool.is_valid_idx(resolved) || !self.visited.insert(resolved) {
            return;
        }
        // Gate on drop-glue existence: a node with NO per-type drop function (a
        // scalar, OR an iterator dispatched inline via `RcStrategy::Iterator`)
        // returns `None` here and is neither recorded nor descended — naming
        // `_ori_drop$<idx>` for it would drift from the emitted symbol set. This
        // is the same iterator gotcha `compute_drop_info` itself guards; the
        // descent MUST honor it for every node, root and descendant alike (a
        // drop-bearing parent's iterator-typed child still has no drop glue).
        let kids = match compute_drop_info(ty, self.classifier, self.pool) {
            None => return,
            Some(info) => drop_info_child_types(&info.kind),
        };
        // `chain` already ends with `ty` (the caller pushed it before recursing;
        // the root is seeded as `[root]`), so the recorded chain is root-first
        // with the last entry equal to `ty`.
        self.record(ty, chain);
        for kid in kids {
            chain.push(kid);
            self.walk(kid, chain, depth + 1);
            chain.pop();
        }
    }
}
