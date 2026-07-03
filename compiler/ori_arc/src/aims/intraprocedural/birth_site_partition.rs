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
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
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

    /// Append one projection hop in place.
    pub(crate) fn push(&mut self, field: u32) {
        self.0.push(field);
    }

    /// A copy of this path with one more projection hop appended.
    #[must_use]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "reserved for the partition's Phase-6/Phase-7 consumers"
        )
    )]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct NodeIdx(u32);

/// Union-find over `(ArcVarId, FieldPath)` nodes partitioned by allocation
/// birth site, with per-class COW-boundary taint.
///
/// Class-level facts (known birth site, COW taint) are authoritative at the
/// class root and re-rooted on every union.
#[derive(Default)]
pub(crate) struct BirthSitePartition {
    /// Intern table: node key -> dense node index.
    nodes: FxHashMap<(ArcVarId, FieldPath), NodeIdx>,
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
    /// class with a known one is an admission bug (debug-asserted).
    pub(crate) fn set(&mut self, node: NodeIdx, site: BirthSiteId) {
        let root = self.find(node.0);
        let slot = &mut self.birth_site[root as usize];
        debug_assert!(
            slot.is_none() || *slot == Some(site),
            "birth-site reassignment on a class with a distinct known site: {slot:?} vs {site:?}"
        );
        *slot = Some(site);
    }

    /// Union two nodes over an unconditional tier-1 same-allocation edge — a
    /// Let-Var whole-var alias, a Project field-path composition, a Construct
    /// field funding, or a `ReturnAliasShape` contract alias. The edge KIND is
    /// the caller's classification; the partition records the edge. Two sides
    /// carrying DISTINCT known birth sites is an admission bug
    /// (debug-asserted); a single known birth site propagates onto the merged
    /// class.
    pub(crate) fn union_tier1(&mut self, a: NodeIdx, b: NodeIdx) {
        self.union_roots(a, b);
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
        self.union_roots(merge_node, first_pred);
        true
    }

    /// Mark `node`'s class as a COW boundary: a COW-mutating use of any
    /// member stops consumers from treating the class as same-allocation
    /// views past the mutation.
    pub(crate) fn mark_cow_boundary(&mut self, node: NodeIdx) {
        let root = self.find(node.0);
        self.cow_boundary[root as usize] = true;
    }

    /// Whether `node`'s class carries the COW-boundary taint.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "reserved for the partition's Phase-6/Phase-7 consumers"
        )
    )]
    pub(crate) fn is_cow_boundary(&mut self, node: NodeIdx) -> bool {
        let root = self.find(node.0);
        self.cow_boundary[root as usize]
    }

    /// Whether two nodes share a representative (same partition class).
    pub(crate) fn same_rep(&mut self, a: NodeIdx, b: NodeIdx) -> bool {
        self.find(a.0) == self.find(b.0)
    }

    /// The representative node of `node`'s class.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "reserved for the partition's Phase-6/Phase-7 consumers"
        )
    )]
    pub(crate) fn rep_of(&mut self, node: NodeIdx) -> NodeIdx {
        NodeIdx(self.find(node.0))
    }

    /// The known allocation birth site of `node`'s class, when recorded.
    pub(crate) fn site(&mut self, node: NodeIdx) -> Option<BirthSiteId> {
        let root = self.find(node.0);
        self.birth_site[root as usize]
    }

    /// Number of interned nodes.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "reserved for the partition's Phase-6/Phase-7 consumers"
        )
    )]
    pub(crate) fn len(&self) -> usize {
        self.parent.len()
    }

    /// Whether no node has been interned.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "reserved for the partition's Phase-6/Phase-7 consumers"
        )
    )]
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
    /// the COW-boundary taint (tainted union anything = tainted).
    fn union_roots(&mut self, a: NodeIdx, b: NodeIdx) {
        let root_a = self.find(a.0);
        let root_b = self.find(b.0);
        if root_a == root_b {
            return;
        }
        let site_a = self.birth_site[root_a as usize];
        let site_b = self.birth_site[root_b as usize];
        debug_assert!(
            !matches!((site_a, site_b), (Some(sa), Some(sb)) if sa != sb),
            "union across classes with distinct known birth sites: {site_a:?} vs {site_b:?}"
        );
        let tainted = self.cow_boundary[root_a as usize] || self.cow_boundary[root_b as usize];
        self.parent[root_a as usize] = root_b;
        self.birth_site[root_b as usize] = site_b.or(site_a);
        self.cow_boundary[root_b as usize] = tainted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Intern a whole-variable node for the raw var index.
    fn whole(partition: &mut BirthSitePartition, var: u32) -> NodeIdx {
        partition.register_node(ArcVarId::new(var), FieldPath::whole_var())
    }

    /// Mirrors `AimsProof.Partition::T1_items_view_unified` +
    /// `T1_agg_class_holds_no_field`: tier-1 view/alias edges unify within a
    /// class, propagate the known birth site in either union direction, and
    /// never leak across classes.
    #[test]
    fn tier1_view_edge_unifies_and_classes_stay_disjoint() {
        let mut partition = BirthSitePartition::new();
        let items_ctor = whole(&mut partition, 10);
        let items_view = whole(&mut partition, 11);
        let extra_view = whole(&mut partition, 13);
        let agg_root = whole(&mut partition, 30);
        let agg_alias = whole(&mut partition, 31);
        partition.set(items_ctor, BirthSiteId::new(100));
        partition.set(agg_root, BirthSiteId::new(300));

        partition.union_tier1(items_view, items_ctor);
        partition.union_tier1(agg_alias, agg_root);
        // Known-site side as the FIRST union argument.
        partition.union_tier1(items_ctor, extra_view);

        assert!(partition.same_rep(items_view, items_ctor));
        assert!(partition.same_rep(extra_view, items_view));
        assert!(partition.same_rep(agg_alias, agg_root));
        assert_eq!(partition.site(items_view), Some(BirthSiteId::new(100)));
        assert_eq!(partition.site(extra_view), Some(BirthSiteId::new(100)));
        assert_eq!(partition.site(agg_alias), Some(BirthSiteId::new(300)));
        assert!(!partition.same_rep(agg_root, items_ctor));
    }

    /// Mirrors `T1_items_header_unified_across_backedge`: a loop-header field
    /// fed the SAME loop-invariant allocation on the entry and latch
    /// (back-edge) predecessors carries the singleton witness, is admitted,
    /// and joins the birth class.
    #[test]
    fn loop_invariant_merge_admitted_under_singleton_witness() {
        let mut partition = BirthSitePartition::new();
        let items_ctor = whole(&mut partition, 10);
        let items_view = whole(&mut partition, 11);
        let items_hdr = whole(&mut partition, 12);
        partition.set(items_ctor, BirthSiteId::new(100));
        partition.union_tier1(items_view, items_ctor);

        // Entry and latch predecessors both resolve to the items class.
        assert!(partition.union_phi_witnessed(items_hdr, &[items_view, items_view]));

        assert!(partition.same_rep(items_hdr, items_ctor));
        assert_eq!(partition.site(items_hdr), Some(BirthSiteId::new(100)));
    }

    /// Mirrors `T1_backedge_keeps_distinct_from_entry` / `_latch` +
    /// `T1_distinct_label_allocs_not_unified`: a merge fed TWO distinct birth
    /// sites (per-iteration re-allocation across the back-edge) has no
    /// singleton witness; the refusal changes nothing and the merge node
    /// keeps its own representative.
    #[test]
    fn loop_varying_merge_refused_over_distinct_birth_sites() {
        let mut partition = BirthSitePartition::new();
        let label_b0 = whole(&mut partition, 20);
        let label_b1 = whole(&mut partition, 21);
        let label_hdr = whole(&mut partition, 22);
        partition.set(label_b0, BirthSiteId::new(200));
        partition.set(label_b1, BirthSiteId::new(201));

        assert!(!partition.union_phi_witnessed(label_hdr, &[label_b0, label_b1]));

        assert!(!partition.same_rep(label_hdr, label_b0));
        assert!(!partition.same_rep(label_hdr, label_b1));
        assert!(!partition.same_rep(label_b0, label_b1));
        assert_eq!(partition.rep_of(label_hdr), label_hdr);
        assert_eq!(partition.site(label_hdr), None);
    }

    /// Distinct field paths of ONE variable intern to distinct nodes and stay
    /// distinct classes absent an admitted edge.
    #[test]
    fn distinct_fields_of_one_var_are_distinct_classes() {
        let mut partition = BirthSitePartition::new();
        let agg = ArcVarId::new(0);
        let items_field = partition.register_node(agg, FieldPath::single(0));
        let label_field = partition.register_node(agg, FieldPath::single(1));
        let whole_agg = partition.register_node(agg, FieldPath::whole_var());

        assert_ne!(items_field, label_field);
        assert!(!partition.same_rep(items_field, label_field));
        assert!(!partition.same_rep(whole_agg, items_field));
        assert!(!partition.same_rep(whole_agg, label_field));
        assert_eq!(partition.len(), 3);
    }

    /// An UNKNOWN predecessor birth site is no witness: the merge refuses
    /// conservatively in every predecessor position, and nothing changes.
    #[test]
    fn unknown_birth_site_pred_refuses_phi_witness() {
        let mut partition = BirthSitePartition::new();
        let known = whole(&mut partition, 20);
        let unknown = whole(&mut partition, 21);
        let merge = whole(&mut partition, 22);
        partition.set(known, BirthSiteId::new(200));

        assert!(!partition.union_phi_witnessed(merge, &[known, unknown]));
        assert!(!partition.union_phi_witnessed(merge, &[unknown, known]));
        assert!(!partition.union_phi_witnessed(merge, &[unknown]));
        assert!(!partition.union_phi_witnessed(merge, &[]));

        assert!(!partition.same_rep(merge, known));
        assert!(!partition.same_rep(merge, unknown));
        assert_eq!(partition.site(merge), None);
    }

    /// A COW boundary taints the CLASS: the taint survives a later tier-1
    /// union in either argument direction; other classes stay untainted.
    #[test]
    fn cow_taint_survives_tier1_union_in_either_direction() {
        let mut partition = BirthSitePartition::new();
        let tainted_lhs = whole(&mut partition, 1);
        let clean_rhs = whole(&mut partition, 2);
        let clean_lhs = whole(&mut partition, 3);
        let tainted_rhs = whole(&mut partition, 4);
        let bystander = whole(&mut partition, 5);

        partition.mark_cow_boundary(tainted_lhs);
        assert!(partition.is_cow_boundary(tainted_lhs));
        partition.union_tier1(tainted_lhs, clean_rhs);
        assert!(partition.is_cow_boundary(clean_rhs));

        partition.mark_cow_boundary(tainted_rhs);
        partition.union_tier1(clean_lhs, tainted_rhs);
        assert!(partition.is_cow_boundary(clean_lhs));

        assert!(!partition.is_cow_boundary(bystander));
    }

    /// `a.b` extended by `.c` equals the hop-by-hop `[b, c]` path; equal
    /// paths hash equal and intern to ONE node.
    #[test]
    fn multi_hop_field_path_composition_interns_to_one_node() {
        let composed = FieldPath::single(3).extended(7);
        let mut pushed = FieldPath::single(3);
        pushed.push(7);
        let from_whole = FieldPath::whole_var().extended(3).extended(7);
        assert_eq!(composed, pushed);
        assert_eq!(composed, from_whole);
        assert_ne!(composed, FieldPath::single(3));

        let mut partition = BirthSitePartition::new();
        let var = ArcVarId::new(9);
        let first = partition.register_node(var, composed);
        let second = partition.register_node(var, pushed);
        assert_eq!(first, second);
        assert_eq!(partition.len(), 1);
    }

    /// `register_node` is an idempotent intern: re-registration returns the
    /// same index and allocates no second node.
    #[test]
    fn register_node_is_idempotent() {
        let mut partition = BirthSitePartition::new();
        assert!(partition.is_empty());

        let first = partition.register_node(ArcVarId::new(4), FieldPath::single(2));
        let second = partition.register_node(ArcVarId::new(4), FieldPath::single(2));

        assert_eq!(first, second);
        assert_eq!(partition.len(), 1);
        assert!(!partition.is_empty());
    }
}
