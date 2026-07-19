//! Per-`(ArcVarId, FieldPath)` birth-site union-find over field lineage nodes.
//!
//! Admission per the T1 partition calculus (`AimsProof.Partition`): tier-1
//! unconditional same-allocation edges (fund / view / alias) union directly;
//! phi/Select merges are admitted ONLY under the singleton birth-site witness
//! (every predecessor class resolves to ONE known birth site). A refused merge
//! keeps its own representative (`distinct_birthsite_no_phi_admission`); the
//! maintained invariant is `samerep_birthsite_sound` — same representative
//! implies same birth site. A COW-mutating use is a class boundary: the class
//! taint survives unions, so consumers stop treating members as
//! same-allocation views past a mutation.

use rustc_hash::FxHashMap;
use smallvec::{smallvec, SmallVec};

use crate::ir::ArcVarId;

/// Multi-hop projection path from a variable root; empty = the whole variable.
///
/// Holds the field indices of a nested projection chain in root-to-leaf order
/// (`a.b.c` is `[b_index, c_index]` rooted at `a`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct FieldPath(SmallVec<[u32; 2]>);

impl FieldPath {
    /// The whole-variable path (no projection hops).
    pub(crate) fn whole_var() -> Self {
        Self(SmallVec::new())
    }

    /// A single-hop projection path.
    pub(crate) fn single(field: u32) -> Self {
        Self(smallvec![field])
    }

    /// The sole field index of a single-hop path; `None` for whole-var or
    /// nested paths.
    pub(crate) fn single_index(&self) -> Option<u32> {
        match self.0.as_slice() {
            [index] => Some(*index),
            _ => None,
        }
    }

    /// Whether this is the whole-variable path (no projection hops).
    pub(crate) fn is_whole_var(&self) -> bool {
        self.0.is_empty()
    }

    /// Append one projection hop in place.
    pub(crate) fn push(&mut self, field: u32) {
        self.0.push(field);
    }

    /// A copy of this path with one more projection hop appended.
    #[must_use]
    #[cfg_attr(not(test), expect(dead_code, reason = "read only in tests"))]
    pub(crate) fn extended(&self, field: u32) -> Self {
        let mut path = self.clone();
        path.push(field);
        path
    }
}

/// Allocation-site identity, minted by the caller one per fresh allocation
/// site. Two nodes may share a class only when their known birth sites agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct BirthSiteId(u32);

impl BirthSiteId {
    /// Create a birth-site id from a raw index.
    pub(crate) fn new(raw: u32) -> Self {
        Self(raw)
    }
}

/// Dense index of an interned `(ArcVarId, FieldPath)` node.
///
/// Ordered by intern index so class collections sort deterministically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct NodeIdx(u32);

/// Union-find over `(ArcVarId, FieldPath)` nodes partitioned by allocation
/// birth site, with per-class COW-boundary taint.
///
/// Class-level facts (known birth site, COW taint) are authoritative at the
/// class root and re-rooted on every union.
#[derive(Clone, Default)]
pub(crate) struct BirthSitePartition {
    /// Intern table: node key -> dense node index.
    nodes: FxHashMap<(ArcVarId, FieldPath), NodeIdx>,
    /// Reverse of `nodes`, indexed by `NodeIdx` (dense — `register_node`
    /// pushes both tables together). Backs the O(1) `node_key` reverse
    /// lookup the diagnostic/tracing surface calls unconditionally.
    keys: Vec<(ArcVarId, FieldPath)>,
    /// Union-find parent per node; `parent[i] == i` at a class root.
    parent: Vec<u32>,
    /// Known birth site per class, authoritative at the class root.
    birth_site: Vec<Option<BirthSiteId>>,
    /// COW-boundary taint per class, authoritative at the class root.
    cow_boundary: Vec<bool>,
}

impl BirthSitePartition {
    /// Create an empty partition.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The `(variable, field-path)` key interned at `node`, when present.
    ///
    /// O(1) dense-vector lookup; diagnostic-surface only (the class-ledger
    /// readiness report renders class identities through it, unconditionally
    /// on every `tracing::debug!`/`tracing::trace!` call site — the lookup
    /// must never cost more than an index + clone).
    pub(crate) fn node_key(&self, node: NodeIdx) -> Option<(ArcVarId, FieldPath)> {
        self.keys.get(node.0 as usize).cloned()
    }

    /// Whether `class` holds any FIELD-PATH member (a `(var, path)` node
    /// with a non-empty projection path) — the structural mark of a
    /// container-held class (its reference lives in the container's field
    /// slot, funded by the container's own class).
    pub(crate) fn class_has_field_path_member(&mut self, class: NodeIdx) -> bool {
        // The collect ends the intern-table borrow before the mutable
        // path-compressing `find` calls.
        #[expect(
            clippy::needless_collect,
            reason = "find() takes &mut self; iterating self.nodes while calling it fails borrowck"
        )]
        let field_nodes: Vec<NodeIdx> = self
            .nodes
            .iter()
            .filter(|((_, path), _)| !path.0.is_empty())
            .map(|(_, &idx)| idx)
            .collect();
        let class_root = self.find(class.0);
        field_nodes
            .into_iter()
            .any(|node| self.find(node.0) == class_root)
    }

    /// Snapshot of every interned `(var, field-path) -> node` entry —
    /// diagnostic/gate surface (the class-ledger field-view hazard scan).
    pub(crate) fn nodes_snapshot(&self) -> Vec<(ArcVarId, FieldPath, NodeIdx)> {
        self.nodes
            .iter()
            .map(|((var, path), &idx)| (*var, path.clone(), idx))
            .collect()
    }

    /// Intern a `(variable, field-path)` node, returning its dense index.
    /// Idempotent: re-registering an existing key returns the same index.
    pub(crate) fn register_node(&mut self, var: ArcVarId, path: FieldPath) -> NodeIdx {
        let Ok(next_raw) = u32::try_from(self.parent.len()) else {
            unreachable!("partition node count exceeds u32::MAX");
        };
        match self.nodes.entry((var, path)) {
            std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let idx = NodeIdx(next_raw);
                self.keys.push(entry.key().clone());
                entry.insert(idx);
                self.parent.push(next_raw);
                self.birth_site.push(None);
                self.cow_boundary.push(false);
                idx
            }
        }
    }

    /// Record the allocation birth site of `node`'s class.
    ///
    /// Class-level fact: stored at the class root, visible through every
    /// member. Re-recording the same site is a no-op; a DISTINCT site on a
    /// class with a known one is an upstream admission bug — the write is
    /// REFUSED release-active (the class keeps its original site; a flipped
    /// site would launder two allocations into one class, the corruption
    /// `samerep_birthsite_sound` forbids).
    pub(crate) fn set(&mut self, node: NodeIdx, site: BirthSiteId) {
        let root = self.find(node.0);
        let slot = &mut self.birth_site[root as usize];
        if let Some(existing) = *slot {
            if existing != site {
                tracing::warn!(
                    target: "ori_arc::aims::intraprocedural",
                    ?existing,
                    ?site,
                    "birth-site reassignment refused: class already carries a distinct known site"
                );
            }
            return;
        }
        *slot = Some(site);
    }

    /// Union two nodes over an unconditional tier-1 same-allocation edge — a
    /// Let-Var whole-var alias, a Project field-path composition, a Construct
    /// field funding, or a `ReturnAliasShape` contract alias. The edge KIND is
    /// the caller's classification; the partition records the edge. Two sides
    /// carrying DISTINCT known birth sites is an upstream admission bug — the
    /// union is REFUSED release-active and `false` is returned (the classes
    /// stay separate: conservative under `samerep_birthsite_sound`, mirroring
    /// [`Self::union_phi_witnessed`]'s refusal shape). A single known birth
    /// site propagates onto the merged class.
    pub(crate) fn union_tier1(&mut self, a: NodeIdx, b: NodeIdx) -> bool {
        self.union_roots(a, b)
    }

    /// Admit a phi/Select merge ONLY under the singleton birth-site witness:
    /// every predecessor's class carries a KNOWN birth site, all predecessors
    /// agree on it, and the merge node's own class does not carry a distinct
    /// one. On admission the merge node is unioned with `preds[0]` (the class
    /// holding the witnessed birth site) and `true` is returned; on refusal
    /// nothing changes and `false` is returned. An empty predecessor list or
    /// any UNKNOWN predecessor birth site refuses conservatively
    /// (`distinct_birthsite_no_phi_admission`).
    pub(crate) fn union_phi_witnessed(&mut self, merge_node: NodeIdx, preds: &[NodeIdx]) -> bool {
        let Some((&first_pred, rest)) = preds.split_first() else {
            return false;
        };
        let Some(witness) = self.site(first_pred) else {
            return false;
        };
        for &pred in rest {
            if self.site(pred) != Some(witness) {
                return false;
            }
        }
        if let Some(existing) = self.site(merge_node) {
            if existing != witness {
                return false;
            }
        }
        self.union_roots(merge_node, first_pred)
    }

    /// Mark `node`'s class as a COW boundary: a COW-mutating use of any
    /// member stops consumers from treating the class as same-allocation
    /// views past the mutation.
    pub(crate) fn mark_cow_boundary(&mut self, node: NodeIdx) {
        let root = self.find(node.0);
        self.cow_boundary[root as usize] = true;
    }

    /// Whether `node`'s class carries the COW-boundary taint.
    #[cfg_attr(not(test), expect(dead_code, reason = "read only in tests"))]
    pub(crate) fn is_cow_boundary(&mut self, node: NodeIdx) -> bool {
        let root = self.find(node.0);
        self.cow_boundary[root as usize]
    }

    /// Whether two nodes share a representative (same partition class).
    pub(crate) fn same_rep(&mut self, a: NodeIdx, b: NodeIdx) -> bool {
        self.find(a.0) == self.find(b.0)
    }

    /// The representative node of `node`'s class.
    pub(crate) fn rep_of(&mut self, node: NodeIdx) -> NodeIdx {
        NodeIdx(self.find(node.0))
    }

    /// The known allocation birth site of `node`'s class, when recorded.
    pub(crate) fn site(&mut self, node: NodeIdx) -> Option<BirthSiteId> {
        let root = self.find(node.0);
        self.birth_site[root as usize]
    }

    /// Number of interned nodes.
    pub(crate) fn len(&self) -> usize {
        self.parent.len()
    }

    /// Whether no node has been interned.
    #[cfg_attr(not(test), expect(dead_code, reason = "read only in tests"))]
    pub(crate) fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Follow parents to the class root, halving the path while walking.
    fn find(&mut self, node: u32) -> u32 {
        let mut current = node;
        while self.parent[current as usize] != current {
            let parent = self.parent[current as usize];
            let grandparent = self.parent[parent as usize];
            self.parent[current as usize] = grandparent;
            current = grandparent;
        }
        current
    }

    /// Point `a`'s root at `b`'s root, merging the class facts: the known
    /// birth site (at most one side carries one under sound admission) and
    /// the COW-boundary taint (tainted union anything = tainted). Distinct
    /// known birth sites REFUSE release-active (returns `false`, nothing
    /// changes): merging them would be exactly the cross-allocation
    /// unification `samerep_birthsite_sound` proves never happens.
    fn union_roots(&mut self, a: NodeIdx, b: NodeIdx) -> bool {
        let root_a = self.find(a.0);
        let root_b = self.find(b.0);
        if root_a == root_b {
            return true;
        }
        let site_a = self.birth_site[root_a as usize];
        let site_b = self.birth_site[root_b as usize];
        if let (Some(sa), Some(sb)) = (site_a, site_b) {
            if sa != sb {
                tracing::warn!(
                    target: "ori_arc::aims::intraprocedural",
                    ?sa,
                    ?sb,
                    "tier-1 union refused: classes carry distinct known birth sites"
                );
                return false;
            }
        }
        let tainted = self.cow_boundary[root_a as usize] || self.cow_boundary[root_b as usize];
        self.parent[root_a as usize] = root_b;
        self.birth_site[root_b as usize] = site_b.or(site_a);
        self.cow_boundary[root_b as usize] = tainted;
        true
    }
}

#[cfg(test)]
mod tests;
