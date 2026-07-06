//! Tests for the birth-site union-find partition.

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

/// A tier-1 union across classes with DISTINCT known birth sites is
/// REFUSED release-active: `false` returned, classes stay separate,
/// both sites intact (`samerep_birthsite_sound` conservatism — never a
/// debug-only guard).
#[test]
fn distinct_site_tier1_union_refused_release_active() {
    let mut partition = BirthSitePartition::new();
    let a = whole(&mut partition, 1);
    let b = whole(&mut partition, 2);
    partition.set(a, BirthSiteId::new(10));
    partition.set(b, BirthSiteId::new(20));

    assert!(!partition.union_tier1(a, b));
    assert!(!partition.same_rep(a, b));
    assert_eq!(partition.site(a), Some(BirthSiteId::new(10)));
    assert_eq!(partition.site(b), Some(BirthSiteId::new(20)));
    // Same-site and unknown-site unions still admit.
    let c = whole(&mut partition, 3);
    assert!(partition.union_tier1(c, a));
    assert!(partition.same_rep(c, a));
}

/// A birth-site write onto a class with a DISTINCT known site is
/// REFUSED: the class keeps its original site (a flipped site would
/// launder two allocations into one class).
#[test]
fn distinct_site_reassignment_refused_keeps_original() {
    let mut partition = BirthSitePartition::new();
    let a = whole(&mut partition, 1);
    partition.set(a, BirthSiteId::new(10));
    partition.set(a, BirthSiteId::new(99));
    assert_eq!(partition.site(a), Some(BirthSiteId::new(10)));
    // Same-site re-record stays a no-op.
    partition.set(a, BirthSiteId::new(10));
    assert_eq!(partition.site(a), Some(BirthSiteId::new(10)));
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
