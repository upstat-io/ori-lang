//! Read-only consumer-edge attribution for the provenance DAG.
//!
//! Walks the drop-descriptor tree and attributes each logical per-type drop
//! plan back to the `Idx` chain that produced it. Diagnostic-only: it consumes
//! [`super::compute_drop_info`] and never selects a physical projection.

use rustc_hash::FxHashSet;

use ori_types::{ConsumerEdge, Idx, Pool};

use crate::ArcClassification;

use super::{compute_drop_info, drop_info_child_types};

/// Read-only consumer attribution for the provenance DAG.
///
/// Walks the drop-descriptor tree from `root` (the same root the provenance DAG
/// walks) and records one [`ConsumerEdge`] per type with a structural drop plan,
/// attributing that plan to the `Idx` chain synthesis descended to reach it.
/// Makes an RC-on-scalar emission point at the generic-leaf chain that produced
/// it (the `Wrap#217{inner: T#141}` to leaf-plan thru-line).
///
/// Diagnostic-only: consumes [`compute_drop_info`] as a pure function of the
/// frozen pool and classifier; never alters drop realization. The walk is
/// depth-bounded and cycle-guarded, and attributes each plan once.
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
/// set, the per-plan dedup set, and the output so the recursion is one
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
    /// Record one consumer edge for `type_idx` on its first-discovered chain.
    fn record(&mut self, type_idx: Idx, chain: &[Idx]) {
        if self.emitted.insert(type_idx) {
            self.edges.push(ConsumerEdge {
                type_idx,
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
        // A scalar or iterator-runtime leaf has no structural per-type plan and
        // is neither recorded nor descended. Honor that distinction for roots
        // and descendants so attribution cannot invent a physical traversal.
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
