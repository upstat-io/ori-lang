//! Matrix tests for SCC-based cycle detection and per-type logical cleanup
//! identity assignment.
//!
//! 8-cell matrix axes (4 axes × 2 polarities):
//!   (a) Self-loop singleton — positive / negative
//!   (b) Mutually-recursive pair — positive / negative
//!   (c) Mutually-recursive triple — positive / negative
//!   (d) Non-recursive baseline — positive / negative
//!
//! Plus boundary pins for the per-Idx operation-identity contract and the
//! decision rule
//! `drop_operation = Some iff (in non-singleton SCC) OR (self-loop) OR
//! (user_drop = Some)` clauses.

use core::num::NonZeroU32;

use ori_registry::burden::FnSym;

use super::{
    assign_drop_operation_syms, compute_scc_partition, mint_drop_operation_sym,
    needs_drop_operation,
};
use crate::registry::burden::{
    UserBorrowedField, UserBurdenSpec, UserOwnedField, UserVariantBurden,
};
use crate::Idx;

// Helpers

fn idx(raw: u32) -> Idx {
    // Use the dynamic range explicitly so synthetic node Idx values do not
    // alias pre-interned primitives.
    Idx::from_raw(Idx::FIRST_DYNAMIC + raw)
}

fn spec_owned_field(field_type: Idx) -> UserOwnedField {
    UserOwnedField {
        field_path: vec![0],
        field_type,
    }
}

fn spec_borrowed_field(field_type: Idx) -> UserBorrowedField {
    UserBorrowedField {
        field_path: vec![0],
        field_type,
    }
}

fn spec_variant_retained(field_type: Idx) -> UserVariantBurden {
    let Some(one) = NonZeroU32::new(1) else {
        unreachable!("1 is non-zero")
    };
    UserVariantBurden {
        variant_id: ori_registry::burden::VariantId::new(one),
        transfers_on_match: vec![],
        retained_owned: vec![spec_owned_field(field_type)],
    }
}

/// A `UserBurdenSpec` with a single owned field of the given type.
fn spec_with_owned_field(field_type: Idx) -> UserBurdenSpec {
    UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![spec_owned_field(field_type)],
        ..UserBurdenSpec::default()
    }
}

/// A `UserBurdenSpec` with no recursive edges — all-scalar fields shape.
fn spec_no_edges() -> UserBurdenSpec {
    UserBurdenSpec {
        self_owned_identity: true,
        ..UserBurdenSpec::default()
    }
}

/// Look up the cleanup identity minted for `idx` and verify its canonical
/// registry-local mapping.
fn assert_operation_identity_matches_idx(
    out: &rustc_hash::FxHashMap<Idx, FnSym>,
    idx: Idx,
) -> FnSym {
    match out.get(&idx) {
        Some(&fn_sym) => {
            // Per `mint_drop_operation_sym`: raw + 1 (saturated). For
            // dynamic Idx values (Idx::FIRST_DYNAMIC..), this matches.
            let expected = mint_drop_operation_sym(idx);
            assert_eq!(
                fn_sym, expected,
                "cleanup identity for {idx:?} must follow the Idx mapping"
            );
            fn_sym
        }
        None => panic!("no cleanup identity minted for Idx {idx:?}"),
    }
}

// (a) Self-loop singleton — positive

#[test]
fn recursive_singleton_node_assigns_drop_operation_identity() {
    // type Node { next: Option<Node> } — Option<Node> contributes an
    // element_burden edge back to Node, producing a self-loop singleton
    // SCC. Per the decision rule, this MUST receive drop_operation.
    let node_idx = idx(0);
    let node_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![spec_owned_field(node_idx)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(node_idx, &node_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(
        out.contains_key(&node_idx),
        "self-loop singleton MUST carry drop_operation"
    );
    assert_operation_identity_matches_idx(&out, node_idx);
}

// (a) Self-loop singleton — negative

#[test]
fn non_self_loop_singleton_leaves_drop_operation_none() {
    // type Leaf { value: int } — value is a primitive scalar (Idx::INT
    // is outside the corpus so it produces no edge). Singleton SCC with
    // no self-loop AND no user_drop → drop_operation stays None.
    let leaf_idx = idx(1);
    let leaf_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![spec_owned_field(Idx::INT)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(leaf_idx, &leaf_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(
        !out.contains_key(&leaf_idx),
        "non-recursive singleton (no user_drop) MUST NOT carry drop_operation"
    );
}

// (b) Mutually-recursive pair — positive

#[test]
fn mutually_recursive_pair_each_carries_distinct_drop_operation_identity() {
    // type Tree { children: [Forest] } + type Forest { trees: [Tree] }
    // — each member's owned_field references the other, forming a
    // 2-member SCC. Both must receive distinct cleanup identities.
    let tree_idx = idx(10);
    let forest_idx = idx(11);
    let tree_spec = spec_with_owned_field(forest_idx);
    let forest_spec = spec_with_owned_field(tree_idx);

    let corpus: Vec<(Idx, &UserBurdenSpec)> =
        vec![(tree_idx, &tree_spec), (forest_idx, &forest_spec)];
    let out = assign_drop_operation_syms(&corpus);

    let tree_fn_sym = assert_operation_identity_matches_idx(&out, tree_idx);
    let forest_fn_sym = assert_operation_identity_matches_idx(&out, forest_idx);

    assert_ne!(
        tree_fn_sym, forest_fn_sym,
        "mutually-recursive pair members must receive distinct cleanup identities"
    );
}

// (b) Mutually-recursive pair — negative

#[test]
fn non_recursive_pair_of_types_leaves_drop_operation_none() {
    // type A { b: B } + type B { x: int }
    // — A → B, but B has no back-edge to A. No cycle, both singletons,
    // no user_drop. drop_operation stays None for both.
    let a_idx = idx(20);
    let b_idx = idx(21);
    let a_spec = spec_with_owned_field(b_idx);
    let b_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![spec_owned_field(Idx::INT)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(a_idx, &a_spec), (b_idx, &b_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(!out.contains_key(&a_idx));
    assert!(!out.contains_key(&b_idx));
}

// (c) Mutually-recursive triple — positive

#[test]
fn mutually_recursive_triple_each_carries_distinct_drop_operation_identity() {
    // type A { b: B } + type B { c: C } + type C { a: A }
    // — A → B → C → A cycle, 3-member SCC. All three MUST receive
    // distinct cleanup identities.
    let a_idx = idx(30);
    let b_idx = idx(31);
    let c_idx = idx(32);
    let a_spec = spec_with_owned_field(b_idx);
    let b_spec = spec_with_owned_field(c_idx);
    let c_spec = spec_with_owned_field(a_idx);

    let corpus: Vec<(Idx, &UserBurdenSpec)> =
        vec![(a_idx, &a_spec), (b_idx, &b_spec), (c_idx, &c_spec)];
    let out = assign_drop_operation_syms(&corpus);

    let a_fn = assert_operation_identity_matches_idx(&out, a_idx);
    let b_fn = assert_operation_identity_matches_idx(&out, b_idx);
    let c_fn = assert_operation_identity_matches_idx(&out, c_idx);

    assert_ne!(a_fn, b_fn, "triple SCC: A and B identities must differ");
    assert_ne!(b_fn, c_fn, "triple SCC: B and C identities must differ");
    assert_ne!(a_fn, c_fn, "triple SCC: A and C identities must differ");

    // Pin SCC partition shape: exactly one component of size 3 covering
    // {a, b, c}.
    let partition = compute_scc_partition(&corpus);
    let cycle_components: Vec<_> = partition.iter().filter(|c| c.len() == 3).collect();
    assert_eq!(
        cycle_components.len(),
        1,
        "exactly one 3-member SCC over {{A, B, C}}"
    );
    let cycle = cycle_components[0];
    assert!(cycle.contains(&a_idx));
    assert!(cycle.contains(&b_idx));
    assert!(cycle.contains(&c_idx));
}

// (c) Mutually-recursive triple — negative

#[test]
fn non_recursive_triple_leaves_drop_operation_none() {
    // type A { b: B } + type B { c: C } + type C { x: int }
    // — Linear chain A → B → C → (no back edge). All three singletons,
    // no user_drop. drop_operation stays None for all.
    let a_idx = idx(40);
    let b_idx = idx(41);
    let c_idx = idx(42);
    let a_spec = spec_with_owned_field(b_idx);
    let b_spec = spec_with_owned_field(c_idx);
    let c_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![spec_owned_field(Idx::INT)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> =
        vec![(a_idx, &a_spec), (b_idx, &b_spec), (c_idx, &c_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(!out.contains_key(&a_idx));
    assert!(!out.contains_key(&b_idx));
    assert!(!out.contains_key(&c_idx));
}

// (d) Non-recursive baseline — positive (no drop_operation)

#[test]
fn non_recursive_pair_leaves_drop_operation_none() {
    // type Pair { a: int, b: str }
    // — both fields are out-of-corpus (primitive scalars and string).
    // Singleton SCC, no self-loop, no user_drop → drop_operation = None.
    let pair_idx = idx(50);
    let pair_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![spec_owned_field(Idx::INT), spec_owned_field(Idx::STR)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(pair_idx, &pair_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(
        !out.contains_key(&pair_idx),
        "non-recursive pair MUST NOT carry drop_operation"
    );
}

// (d) Non-recursive negative — wrapping a recursive type

#[test]
fn non_recursive_with_recursive_field_assigns_drop_operation_identity() {
    // type Node { next: Option<Node> }  — recursive (self-loop singleton)
    // type Wrapper { inner: Node }      — owns a Node field; Wrapper is
    //                                     itself NOT recursive (no cycle
    //                                     back to Wrapper).
    //
    // Per cycle-detection invariant: per-type, not transitive through
    // fields. Wrapper drops Node via the field walk (Node's own
    // drop_operation), NOT by inheriting drop_operation. Wrapper's
    // drop_operation stays None; Node's drop_operation is Some.
    let node_idx = idx(60);
    let wrapper_idx = idx(61);
    let node_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![spec_owned_field(node_idx)], // self-loop via Option-as-edge
        ..UserBurdenSpec::default()
    };
    let wrapper_spec = spec_with_owned_field(node_idx);

    let corpus: Vec<(Idx, &UserBurdenSpec)> =
        vec![(node_idx, &node_spec), (wrapper_idx, &wrapper_spec)];
    let out = assign_drop_operation_syms(&corpus);

    // Positive half: recursive Node gets drop_operation.
    assert!(
        out.contains_key(&node_idx),
        "recursive Node MUST carry drop_operation"
    );
    // Negative half: Wrapper does NOT inherit drop_operation from its
    // recursive field.
    assert!(
        !out.contains_key(&wrapper_idx),
        "Wrapper MUST NOT inherit drop_operation from its recursive field — \
         cycle detection is per-type, not transitive"
    );
}

// Decision-rule clause pin: user_drop drives drop_operation

#[test]
fn user_drop_some_forces_drop_operation_even_when_non_recursive() {
    // type Leaf { value: int } with user @drop impl.
    // drop_operation = Some iff (non-singleton SCC) OR (self-loop)
    // OR (user_drop = Some). The logical operation composes user @drop with
    // field cleanup, so a Drop-impl type must carry an operation identity.
    let leaf_idx = idx(70);
    let Some(user_drop_one) = NonZeroU32::new(99) else {
        unreachable!("99 is non-zero")
    };
    let leaf_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![spec_owned_field(Idx::INT)],
        user_drop: Some(FnSym::new(user_drop_one)),
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(leaf_idx, &leaf_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(
        out.contains_key(&leaf_idx),
        "user_drop = Some must force a logical drop_operation identity"
    );
}

// Decision-rule unit-level coverage

#[test]
fn needs_drop_operation_clauses_match_spec() {
    let dummy = spec_no_edges();
    // Clause 1: non-singleton SCC
    assert!(needs_drop_operation(2, false, &dummy));
    assert!(needs_drop_operation(3, false, &dummy));
    // Clause 2: self-loop on singleton
    assert!(needs_drop_operation(1, true, &dummy));
    // Clause 3: user_drop = Some
    let Some(one) = NonZeroU32::new(1) else {
        unreachable!("1 is non-zero")
    };
    let with_user_drop = UserBurdenSpec {
        user_drop: Some(FnSym::new(one)),
        ..dummy.clone()
    };
    assert!(needs_drop_operation(1, false, &with_user_drop));
    // Negative: none of the clauses
    assert!(!needs_drop_operation(1, false, &dummy));
}

// Variant-burden edge: enum SCC participation

#[test]
fn variant_retained_owned_field_contributes_to_scc() {
    // type Tree = Leaf | Branch(Tree, Tree)
    // — Branch's retained_owned[*].field_type references Tree itself,
    // forming a self-loop singleton SCC via the variant_burdens edge
    // path (edge case (d)).
    let tree_idx = idx(80);
    let tree_spec = UserBurdenSpec {
        self_owned_identity: true,
        variant_burdens: vec![spec_variant_retained(tree_idx)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(tree_idx, &tree_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(
        out.contains_key(&tree_idx),
        "variant-retained self-reference MUST be detected as a self-loop"
    );
}

// element_burden edge participation

#[test]
fn element_burden_contributes_to_scc() {
    // type Container { ... } whose element_burden references itself
    // forms a self-loop via the edge-case (c) path.
    let container_idx = idx(90);
    let container_spec = UserBurdenSpec {
        self_owned_identity: true,
        element_burden: Some(container_idx),
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(container_idx, &container_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(
        out.contains_key(&container_idx),
        "element_burden self-reference MUST be detected as a self-loop"
    );
}

// borrowed_field edge participation

#[test]
fn borrowed_field_contributes_to_scc() {
    // type SelfBorrow { back: &Self }  (target-only Tag::Borrowed, but the
    // SCC edge still applies via edge case (b)).
    let sb_idx = idx(100);
    let sb_spec = UserBurdenSpec {
        self_owned_identity: true,
        borrowed_fields: vec![spec_borrowed_field(sb_idx)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(sb_idx, &sb_spec)];
    let out = assign_drop_operation_syms(&corpus);

    assert!(
        out.contains_key(&sb_idx),
        "borrowed_field self-reference (target-only borrow) MUST be detected as a self-loop"
    );
}

// Stable operation-identity mapping

#[test]
fn mint_drop_operation_sym_carries_idx_raw_plus_one() {
    // The registry-local identity carries the type Idx offset by one to
    // preserve `NonZeroU32` distinctness for `Idx::INT` (raw zero).
    let a = idx(0); // raw 64
    let b = idx(1); // raw 65
    let fn_a = mint_drop_operation_sym(a);
    let fn_b = mint_drop_operation_sym(b);

    assert_eq!(
        fn_a.get().get(),
        a.raw() + 1,
        "operation identity must be Idx::raw + 1"
    );
    assert_eq!(fn_b.get().get(), b.raw() + 1);
    assert_ne!(fn_a, fn_b, "distinct Idx must yield distinct identities");
}

// Empty corpus determinism

#[test]
fn empty_corpus_yields_empty_partition() {
    let partition = compute_scc_partition(&[]);
    assert!(partition.is_empty());

    let out = assign_drop_operation_syms(&[]);
    assert!(out.is_empty());
}
