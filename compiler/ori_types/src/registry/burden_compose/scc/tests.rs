//! Matrix tests for SCC-based cycle detection + per-type compiled drop-glue
//! assignment.
//!
//! 8-cell matrix axes (4 axes × 2 polarities):
//!   (a) Self-loop singleton — positive / negative
//!   (b) Mutually-recursive pair — positive / negative
//!   (c) Mutually-recursive triple — positive / negative
//!   (d) Non-recursive baseline — positive / negative
//!
//! Plus boundary pins for the per-Idx `FnSym` distinctness contract +
//! `_ori_drop$<idx_raw>` mangling key invariant + decision-rule
//! `compiled_drop = Some iff (in non-singleton SCC) OR (self-loop) OR
//! (user_drop = Some)` clauses.

use core::num::NonZeroU32;

use ori_registry::burden::FnSym;

use super::{
    assign_compiled_drop_fn_syms, compute_scc_partition, mint_compiled_drop_fn_sym,
    needs_compiled_drop,
};
use crate::registry::burden::{
    UserBorrowedField, UserBurdenSpec, UserOwnedField, UserVariantBurden,
};
use crate::Idx;

// === Helpers ===

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
        self_heap_alloc: true,
        owned_fields: vec![spec_owned_field(field_type)],
        ..UserBurdenSpec::default()
    }
}

/// A `UserBurdenSpec` with no recursive edges — all-scalar fields shape.
fn spec_no_edges() -> UserBurdenSpec {
    UserBurdenSpec {
        self_heap_alloc: true,
        ..UserBurdenSpec::default()
    }
}

/// Look up the `FnSym` minted for `idx` in the assignment map, asserting it
/// matches the deterministic `_ori_drop$<idx_raw>` mangling key.
fn assert_fn_sym_matches_mangling(out: &rustc_hash::FxHashMap<Idx, FnSym>, idx: Idx) -> FnSym {
    match out.get(&idx) {
        Some(&fn_sym) => {
            // Per `mint_compiled_drop_fn_sym`: raw + 1 (saturated). For
            // dynamic Idx values (Idx::FIRST_DYNAMIC..), this matches.
            let expected = mint_compiled_drop_fn_sym(idx);
            assert_eq!(
                fn_sym, expected,
                "FnSym for {idx:?} should follow the Idx-derived mangling"
            );
            fn_sym
        }
        None => panic!("no FnSym minted for Idx {idx:?}"),
    }
}

// === (a) Self-loop singleton — positive ===

#[test]
fn recursive_singleton_node_assigns_compiled_drop_fn_sym() {
    // type Node { next: Option<Node> } — Option<Node> contributes an
    // element_burden edge back to Node, producing a self-loop singleton
    // SCC. Per the decision rule, this MUST receive compiled_drop.
    let node_idx = idx(0);
    let node_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![spec_owned_field(node_idx)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(node_idx, &node_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(
        out.contains_key(&node_idx),
        "self-loop singleton MUST carry compiled_drop"
    );
    assert_fn_sym_matches_mangling(&out, node_idx);
}

// === (a) Self-loop singleton — negative ===

#[test]
fn non_self_loop_singleton_leaves_compiled_drop_none() {
    // type Leaf { value: int } — value is a primitive scalar (Idx::INT
    // is outside the corpus so it produces no edge). Singleton SCC with
    // no self-loop AND no user_drop → compiled_drop stays None.
    let leaf_idx = idx(1);
    let leaf_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![spec_owned_field(Idx::INT)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(leaf_idx, &leaf_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(
        !out.contains_key(&leaf_idx),
        "non-recursive singleton (no user_drop) MUST NOT carry compiled_drop"
    );
}

// === (b) Mutually-recursive pair — positive ===

#[test]
fn mutually_recursive_pair_each_carries_distinct_compiled_drop_fn_sym() {
    // type Tree { children: [Forest] } + type Forest { trees: [Tree] }
    // — each member's owned_field references the other, forming a
    // 2-member SCC. Both MUST receive compiled_drop, and the FnSyms
    // MUST be distinct (matching the `_ori_drop$<idx_raw>` per-type
    // mangling at drop_gen.rs:45).
    let tree_idx = idx(10);
    let forest_idx = idx(11);
    let tree_spec = spec_with_owned_field(forest_idx);
    let forest_spec = spec_with_owned_field(tree_idx);

    let corpus: Vec<(Idx, &UserBurdenSpec)> =
        vec![(tree_idx, &tree_spec), (forest_idx, &forest_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    let tree_fn_sym = assert_fn_sym_matches_mangling(&out, tree_idx);
    let forest_fn_sym = assert_fn_sym_matches_mangling(&out, forest_idx);

    assert_ne!(
        tree_fn_sym, forest_fn_sym,
        "mutually-recursive pair members MUST receive DISTINCT FnSyms per the per-type mangling"
    );
}

// === (b) Mutually-recursive pair — negative ===

#[test]
fn non_recursive_pair_of_types_leaves_compiled_drop_none() {
    // type A { b: B } + type B { x: int }
    // — A → B, but B has no back-edge to A. No cycle, both singletons,
    // no user_drop. compiled_drop stays None for both.
    let a_idx = idx(20);
    let b_idx = idx(21);
    let a_spec = spec_with_owned_field(b_idx);
    let b_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![spec_owned_field(Idx::INT)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(a_idx, &a_spec), (b_idx, &b_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(!out.contains_key(&a_idx));
    assert!(!out.contains_key(&b_idx));
}

// === (c) Mutually-recursive triple — positive ===

#[test]
fn mutually_recursive_triple_each_carries_distinct_compiled_drop_fn_sym() {
    // type A { b: B } + type B { c: C } + type C { a: A }
    // — A → B → C → A cycle, 3-member SCC. All three MUST receive
    // distinct FnSyms.
    let a_idx = idx(30);
    let b_idx = idx(31);
    let c_idx = idx(32);
    let a_spec = spec_with_owned_field(b_idx);
    let b_spec = spec_with_owned_field(c_idx);
    let c_spec = spec_with_owned_field(a_idx);

    let corpus: Vec<(Idx, &UserBurdenSpec)> =
        vec![(a_idx, &a_spec), (b_idx, &b_spec), (c_idx, &c_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    let a_fn = assert_fn_sym_matches_mangling(&out, a_idx);
    let b_fn = assert_fn_sym_matches_mangling(&out, b_idx);
    let c_fn = assert_fn_sym_matches_mangling(&out, c_idx);

    assert_ne!(a_fn, b_fn, "triple SCC: A vs B FnSym MUST differ");
    assert_ne!(b_fn, c_fn, "triple SCC: B vs C FnSym MUST differ");
    assert_ne!(a_fn, c_fn, "triple SCC: A vs C FnSym MUST differ");

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

// === (c) Mutually-recursive triple — negative ===

#[test]
fn non_recursive_triple_leaves_compiled_drop_none() {
    // type A { b: B } + type B { c: C } + type C { x: int }
    // — Linear chain A → B → C → (no back edge). All three singletons,
    // no user_drop. compiled_drop stays None for all.
    let a_idx = idx(40);
    let b_idx = idx(41);
    let c_idx = idx(42);
    let a_spec = spec_with_owned_field(b_idx);
    let b_spec = spec_with_owned_field(c_idx);
    let c_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![spec_owned_field(Idx::INT)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> =
        vec![(a_idx, &a_spec), (b_idx, &b_spec), (c_idx, &c_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(!out.contains_key(&a_idx));
    assert!(!out.contains_key(&b_idx));
    assert!(!out.contains_key(&c_idx));
}

// === (d) Non-recursive baseline — positive (no compiled_drop) ===

#[test]
fn non_recursive_pair_leaves_compiled_drop_none() {
    // type Pair { a: int, b: str }
    // — both fields are out-of-corpus (primitive scalars and string).
    // Singleton SCC, no self-loop, no user_drop → compiled_drop = None.
    let pair_idx = idx(50);
    let pair_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![spec_owned_field(Idx::INT), spec_owned_field(Idx::STR)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(pair_idx, &pair_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(
        !out.contains_key(&pair_idx),
        "non-recursive pair MUST NOT carry compiled_drop"
    );
}

// === (d) Non-recursive negative — wrapping a recursive type ===

#[test]
fn non_recursive_with_recursive_field_assigns_compiled_drop_fn_sym() {
    // type Node { next: Option<Node> }  — recursive (self-loop singleton)
    // type Wrapper { inner: Node }      — owns a Node field; Wrapper is
    //                                     itself NOT recursive (no cycle
    //                                     back to Wrapper).
    //
    // Per cycle-detection invariant: per-type, not transitive through
    // fields. Wrapper drops Node via the field walk (Node's own
    // compiled_drop), NOT by inheriting compiled_drop. Wrapper's
    // compiled_drop stays None; Node's compiled_drop is Some.
    let node_idx = idx(60);
    let wrapper_idx = idx(61);
    let node_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![spec_owned_field(node_idx)], // self-loop via Option-as-edge
        ..UserBurdenSpec::default()
    };
    let wrapper_spec = spec_with_owned_field(node_idx);

    let corpus: Vec<(Idx, &UserBurdenSpec)> =
        vec![(node_idx, &node_spec), (wrapper_idx, &wrapper_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    // Positive half: recursive Node gets compiled_drop.
    assert!(
        out.contains_key(&node_idx),
        "recursive Node MUST carry compiled_drop"
    );
    // Negative half: Wrapper does NOT inherit compiled_drop from its
    // recursive field.
    assert!(
        !out.contains_key(&wrapper_idx),
        "Wrapper MUST NOT inherit compiled_drop from its recursive field — \
         cycle detection is per-type, not transitive"
    );
}

// === Decision-rule clause pin: user_drop drives compiled_drop ===

#[test]
fn user_drop_some_forces_compiled_drop_even_when_non_recursive() {
    // type Leaf { value: int } with user @drop impl.
    // Per §04.1: compiled_drop = Some iff (non-singleton SCC) OR (self-loop)
    // OR (user_drop = Some). The AUGMENT-shape compiled drop wraps the user
    // @drop body, so a Drop-impl type MUST have compiled_drop populated.
    let leaf_idx = idx(70);
    let Some(user_drop_one) = NonZeroU32::new(99) else {
        unreachable!("99 is non-zero")
    };
    let leaf_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![spec_owned_field(Idx::INT)],
        user_drop: Some(FnSym::new(user_drop_one)),
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(leaf_idx, &leaf_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(
        out.contains_key(&leaf_idx),
        "user_drop = Some MUST force compiled_drop = Some (AUGMENT wrapper needs an entry point)"
    );
}

// === Decision-rule unit-level coverage ===

#[test]
fn needs_compiled_drop_clauses_match_spec() {
    let dummy = spec_no_edges();
    // Clause 1: non-singleton SCC
    assert!(needs_compiled_drop(2, false, &dummy));
    assert!(needs_compiled_drop(3, false, &dummy));
    // Clause 2: self-loop on singleton
    assert!(needs_compiled_drop(1, true, &dummy));
    // Clause 3: user_drop = Some
    let Some(one) = NonZeroU32::new(1) else {
        unreachable!("1 is non-zero")
    };
    let with_user_drop = UserBurdenSpec {
        user_drop: Some(FnSym::new(one)),
        ..dummy.clone()
    };
    assert!(needs_compiled_drop(1, false, &with_user_drop));
    // Negative: none of the clauses
    assert!(!needs_compiled_drop(1, false, &dummy));
}

// === Variant-burden edge: enum SCC participation ===

#[test]
fn variant_retained_owned_field_contributes_to_scc() {
    // type Tree = Leaf | Branch(Tree, Tree)
    // — Branch's retained_owned[*].field_type references Tree itself,
    // forming a self-loop singleton SCC via the variant_burdens edge
    // path (§04.1 (d)).
    let tree_idx = idx(80);
    let tree_spec = UserBurdenSpec {
        self_heap_alloc: true,
        variant_burdens: vec![spec_variant_retained(tree_idx)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(tree_idx, &tree_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(
        out.contains_key(&tree_idx),
        "variant-retained self-reference MUST be detected as a self-loop"
    );
}

// === element_burden edge participation ===

#[test]
fn element_burden_contributes_to_scc() {
    // type Container { ... } whose element_burden references itself
    // forms a self-loop via the §04.1 (c) edge path.
    let container_idx = idx(90);
    let container_spec = UserBurdenSpec {
        self_heap_alloc: true,
        element_burden: Some(container_idx),
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(container_idx, &container_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(
        out.contains_key(&container_idx),
        "element_burden self-reference MUST be detected as a self-loop"
    );
}

// === borrowed_field edge participation ===

#[test]
fn borrowed_field_contributes_to_scc() {
    // type SelfBorrow { back: &Self }  (target-only Tag::Borrowed, but the
    // SCC edge still applies via §04.1 (b)).
    let sb_idx = idx(100);
    let sb_spec = UserBurdenSpec {
        self_heap_alloc: true,
        borrowed_fields: vec![spec_borrowed_field(sb_idx)],
        ..UserBurdenSpec::default()
    };

    let corpus: Vec<(Idx, &UserBurdenSpec)> = vec![(sb_idx, &sb_spec)];
    let out = assign_compiled_drop_fn_syms(&corpus);

    assert!(
        out.contains_key(&sb_idx),
        "borrowed_field self-reference (target-only borrow) MUST be detected as a self-loop"
    );
}

// === FnSym mangling key invariant pin ===

#[test]
fn mint_compiled_drop_fn_sym_carries_idx_raw_plus_one() {
    // Per `_ori_drop$<idx_raw>` mangling at drop_gen.rs:45, the FnSym
    // MUST carry the type's Idx (offset by +1 to preserve NonZeroU32
    // distinctness for Idx::INT (raw 0)).
    let a = idx(0); // raw 64
    let b = idx(1); // raw 65
    let fn_a = mint_compiled_drop_fn_sym(a);
    let fn_b = mint_compiled_drop_fn_sym(b);

    assert_eq!(
        fn_a.get().get(),
        a.raw() + 1,
        "FnSym raw value MUST be Idx::raw + 1 (NonZeroU32 lift)"
    );
    assert_eq!(fn_b.get().get(), b.raw() + 1);
    assert_ne!(fn_a, fn_b, "distinct Idx MUST yield distinct FnSym");
}

// === Empty corpus determinism ===

#[test]
fn empty_corpus_yields_empty_partition() {
    let partition = compute_scc_partition(&[]);
    assert!(partition.is_empty());

    let out = assign_compiled_drop_fn_syms(&[]);
    assert!(out.is_empty());
}
