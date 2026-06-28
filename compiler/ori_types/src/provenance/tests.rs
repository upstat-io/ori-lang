use ori_ir::StringInterner;

use super::{ProvenanceDag, MAX_DEPTH};
use crate::{EnumVariant, Idx, MonoInstance, Pool};

/// Nested structs (`Outer { inner: Inner }`, `Inner { val: int }`) yield a
/// non-empty STRUCTURE DAG with one edge per field. Semantic pin: fails if the
/// walk stops collecting struct-field edges.
#[test]
fn nested_structs_outer_root_nonempty_structure_dag() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let inner_name = interner.intern("Inner");
    let outer_name = interner.intern("Outer");
    let f_val = interner.intern("val");
    let f_inner = interner.intern("inner");

    let inner = pool.struct_type(inner_name, &[(f_val, Idx::INT)]);
    let outer = pool.struct_type(outer_name, &[(f_inner, inner)]);

    let dag = ProvenanceDag::walk(&pool, outer, MAX_DEPTH);

    assert!(
        !dag.is_empty(),
        "nested-struct root must yield structure edges"
    );
    assert_eq!(dag.edges.len(), 2, "Outer.inner + Inner.val");
    assert_eq!(dag.edges[0].parent_idx, outer);
    assert_eq!(dag.edges[0].field_name, f_inner);
    assert_eq!(dag.edges[0].child_idx, inner);
    assert_eq!(dag.edges[1].parent_idx, inner);
    assert_eq!(dag.edges[1].field_name, f_val);
    assert_eq!(dag.edges[1].child_idx, Idx::INT);
}

/// A primitive leaf root has no structure — empty DAG. Negative pin: rejects a
/// walk that fabricates edges on a scalar.
#[test]
fn primitive_leaf_root_yields_empty_dag() {
    let pool = Pool::new();
    let dag = ProvenanceDag::walk(&pool, Idx::INT, MAX_DEPTH);
    assert!(dag.is_empty(), "primitive root has no structure edges");
}

/// An out-of-range root (an invalid `ORI_TRACE_IDX`) yields an empty DAG and
/// never panics. Negative pin: guards the `Pool::tag` bounds-check path.
#[test]
fn out_of_range_root_yields_empty_dag_no_panic() {
    let pool = Pool::new();
    let bogus = Idx::from_raw(u32::MAX - 1);
    let dag = ProvenanceDag::walk(&pool, bogus, MAX_DEPTH);
    assert!(dag.is_empty(), "out-of-range root must yield no edges");
}

/// Rendering a DAG rooted at an out-of-range `Idx` yields an empty string and
/// never panics. Negative pin: `render` calls `format_type_with_idx(self.root)`,
/// which indexes `items` without a bounds check; this fails (panics) if the
/// render-side `is_valid_idx(self.root)` guard is removed.
#[test]
fn render_out_of_range_root_yields_empty_no_panic() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let bogus = Idx::from_raw(u32::MAX - 1);
    let dag = ProvenanceDag::walk(&pool, bogus, MAX_DEPTH);

    let rendered = dag.render(&pool, &interner);
    assert!(
        rendered.is_empty(),
        "out-of-range root must render to an empty string, not panic"
    );
}

/// A self-referential resolution (`N` resolves to `Struct { next: N }`) is the
/// `Wrap<Wrap<...>>`-class recursion shape. The walk terminates with bounded
/// edges via the visited-set cycle guard. Pin: fails (hangs / overflows) if the
/// cycle guard is removed.
#[test]
fn recursive_resolution_terminates_via_cycle_guard() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let node_name = interner.intern("Node");
    let next = interner.intern("next");

    let named = pool.named(node_name);
    let concrete = pool.struct_type(node_name, &[(next, named)]);
    pool.set_resolution(named, concrete);

    let dag = ProvenanceDag::walk(&pool, named, MAX_DEPTH);

    // One edge (concrete --next--> named); the re-entry into `concrete` is cut
    // by the visited set, so the walk does not recurse forever.
    assert_eq!(dag.edges.len(), 1, "cycle guard caps the recursive walk");
    assert_eq!(dag.edges[0].field_name, next);
}

/// A chain deeper than the cap stops at `max_depth` edges. Pin: fails if the
/// depth bound is dropped.
#[test]
fn depth_bound_caps_chain_walk() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let f = interner.intern("f");

    // Build a 5-deep struct chain bottom-up: s4{v:int}, s3{f:s4}, ... s0{f:s1}.
    let leaf = pool.struct_type(interner.intern("S4"), &[(interner.intern("v"), Idx::INT)]);
    let mut current = leaf;
    for i in (0..4).rev() {
        let name = interner.intern(&format!("S{i}"));
        current = pool.struct_type(name, &[(f, current)]);
    }

    let dag = ProvenanceDag::walk(&pool, current, 3);
    assert_eq!(dag.edges.len(), 3, "walk stops at max_depth=3 edges");
}

/// An enum-variant payload produces a STRUCTURE edge under the variant name.
#[test]
fn enum_variant_payload_yields_structure_edge() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let some = interner.intern("Some");
    let none = interner.intern("None");
    let enum_name = interner.intern("MyOption");

    let variants = vec![
        EnumVariant {
            name: some,
            field_types: vec![Idx::INT],
        },
        EnumVariant {
            name: none,
            field_types: vec![],
        },
    ];
    let enum_ty = pool.enum_type(enum_name, &variants);

    let dag = ProvenanceDag::walk(&pool, enum_ty, MAX_DEPTH);
    assert_eq!(
        dag.edges.len(),
        1,
        "only the payload-bearing variant emits an edge"
    );
    assert_eq!(dag.edges[0].field_name, some);
    assert_eq!(dag.edges[0].child_idx, Idx::INT);
}

/// A diamond (two fields pointing at one shared struct) records both parent
/// edges but walks the shared subtree exactly once (visited-set dedup).
#[test]
fn diamond_shared_child_walked_once() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let shared = pool.struct_type(
        interner.intern("Shared"),
        &[(interner.intern("v"), Idx::INT)],
    );
    let root = pool.struct_type(
        interner.intern("Root"),
        &[
            (interner.intern("a"), shared),
            (interner.intern("b"), shared),
        ],
    );

    let dag = ProvenanceDag::walk(&pool, root, MAX_DEPTH);
    // Root.a, Root.b (both recorded) + Shared.v once = 3.
    assert_eq!(dag.edges.len(), 3);
    let shared_v_edges = dag.edges.iter().filter(|e| e.parent_idx == shared).count();
    assert_eq!(shared_v_edges, 1, "shared subtree walked exactly once");
}

/// The renderer reuses `format_type_with_idx` and emits a non-empty body for a
/// non-empty DAG.
#[test]
fn render_emits_header_and_edges() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let inner = pool.struct_type(
        interner.intern("Inner"),
        &[(interner.intern("val"), Idx::INT)],
    );
    let outer = pool.struct_type(
        interner.intern("Outer"),
        &[(interner.intern("inner"), inner)],
    );

    let dag = ProvenanceDag::walk(&pool, outer, MAX_DEPTH);
    let rendered = dag.render(&pool, &interner);
    assert!(rendered.contains("Provenance DAG"));
    assert!(rendered.contains("structure edge"));
    assert!(rendered.contains("-->"));
}

// RESOLUTION edges: `Named`/`Applied -> concrete` read per node via the
// read-only `resolve` accessor.

/// A `Named` resolving to a concrete struct yields one RESOLUTION edge. Pin:
/// fails if the read-only `resolve` per-node collection is dropped.
#[test]
fn resolution_edge_collected_for_resolved_named() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let name = interner.intern("Shape");
    let named = pool.named(name);
    let concrete = pool.struct_type(name, &[(interner.intern("v"), Idx::INT)]);
    pool.set_resolution(named, concrete);

    let dag = ProvenanceDag::walk(&pool, named, MAX_DEPTH);
    assert_eq!(
        dag.resolution_edges.len(),
        1,
        "the resolved Named root yields one resolution edge"
    );
    assert_eq!(dag.resolution_edges[0].applied_idx, named);
    assert_eq!(dag.resolution_edges[0].concrete_idx, concrete);
}

/// A concrete root (no `Named` resolution) yields zero RESOLUTION edges.
/// Negative pin: rejects fabricating a resolution edge on a concrete struct.
#[test]
fn no_resolution_edge_for_concrete_root() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let s = pool.struct_type(interner.intern("S"), &[(interner.intern("v"), Idx::INT)]);
    let dag = ProvenanceDag::walk(&pool, s, MAX_DEPTH);
    assert!(
        dag.resolution_edges.is_empty(),
        "a concrete struct root has no resolution edge"
    );
}

// MONO edges: projected 1:1 off MonoInstance.body_type_map.

/// Helper: a top-level mono instance carrying only a `body_type_map`.
fn mono_instance(interner: &StringInterner, body_type_map: Vec<(Idx, Idx)>) -> MonoInstance {
    MonoInstance::new_top_level(
        interner.intern("f"),
        Vec::new(),
        Vec::new(),
        Idx::INT,
        body_type_map,
    )
}

/// `with_mono_edges` projects each `body_type_map` entry into one `MonoEdge`.
/// Pin: fails if the 1:1 projection drops or duplicates entries.
#[test]
fn mono_edges_projected_from_body_type_map() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let t = pool.named(interner.intern("T"));
    let s = pool.struct_type(interner.intern("Wrap"), &[(interner.intern("inner"), t)]);

    let inst = mono_instance(&interner, vec![(t, Idx::INT)]);
    let dag = ProvenanceDag::walk(&pool, s, MAX_DEPTH).with_mono_edges(&pool, &[inst]);

    assert_eq!(
        dag.mono_edges.len(),
        1,
        "one body_type_map entry -> one mono edge"
    );
    assert_eq!(dag.mono_edges[0].source_idx, t);
    assert_eq!(dag.mono_edges[0].substituted_idx, Idx::INT);
}

// GENERIC-LEAF detector: the `is_generic_leaf` Named-unresolved gotcha —
// `Tag::is_type_variable` alone misses unresolved `Named`.

/// An UNRESOLVED `Named` child (a `T#141`-shape generic-param leaf) with a
/// concrete mono substitution flags one divergence. Pin: fails if the detector
/// reuses `is_type_variable` verbatim (which excludes `Named`) — the dossier's
/// one real implementation gotcha.
#[test]
fn unresolved_named_leaf_with_concrete_mono_flags_divergence() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    // `Wrap { inner: T }` where `T` is an UNRESOLVED Named (no set_resolution).
    let t = pool.named(interner.intern("T"));
    let wrap = pool.struct_type(interner.intern("Wrap"), &[(interner.intern("inner"), t)]);

    let inst = mono_instance(&interner, vec![(t, Idx::INT)]);
    let dag = ProvenanceDag::walk(&pool, wrap, MAX_DEPTH).with_mono_edges(&pool, &[inst]);

    assert_eq!(
        dag.divergences.len(),
        1,
        "unresolved Named leaf + concrete mono substitution is one divergence"
    );
    assert_eq!(dag.divergences[0].generic_idx, t);
    assert_eq!(dag.divergences[0].concrete_idx, Idx::INT);
}

/// A type-variable leaf (`BoundVar`) is also flagged — the `is_type_variable`
/// arm of the detector. Pin: fails if the detector only handles `Named`.
#[test]
fn bound_var_leaf_with_concrete_mono_flags_divergence() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let bv = pool.bound_var(0);
    let wrap = pool.struct_type(interner.intern("Wrap"), &[(interner.intern("inner"), bv)]);

    let inst = mono_instance(&interner, vec![(bv, Idx::INT)]);
    let dag = ProvenanceDag::walk(&pool, wrap, MAX_DEPTH).with_mono_edges(&pool, &[inst]);
    assert_eq!(dag.divergences.len(), 1, "BoundVar leaf is a generic leaf");
    assert_eq!(dag.divergences[0].generic_idx, bv);
}

/// A clean-monomorphic body (concrete leaf) yields ZERO divergences even with a
/// concrete-to-concrete mono edge. Negative pin: the detector must not fire on
/// a fully-concrete instantiation.
#[test]
fn clean_monomorphic_body_no_divergence() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let wrap = pool.struct_type(
        interner.intern("Wrap"),
        &[(interner.intern("inner"), Idx::INT)],
    );

    let inst = mono_instance(&interner, vec![(Idx::INT, Idx::INT)]);
    let dag = ProvenanceDag::walk(&pool, wrap, MAX_DEPTH).with_mono_edges(&pool, &[inst]);
    assert!(
        dag.divergences.is_empty(),
        "a concrete leaf is not a generic-leaf divergence"
    );
}

/// A generic leaf under a CONCRETE root with no concrete mono substitution
/// still flags a STRUCTURAL divergence — the param survived into a concrete
/// instantiation — with `concrete_idx` falling back to the resolved root (no
/// precise `int` counterpart available). Pin: the divergence is the generic
/// leaf under a concrete root, refined by mono when present, never gated on it.
#[test]
fn generic_leaf_no_concrete_mono_falls_back_to_root() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let t = pool.named(interner.intern("T"));
    let u = pool.named(interner.intern("U")); // also unresolved -> generic
    let wrap = pool.struct_type(interner.intern("Wrap"), &[(interner.intern("inner"), t)]);

    // A generic-to-generic mono substitution supplies NO concrete counterpart.
    let inst = mono_instance(&interner, vec![(t, u)]);
    let dag = ProvenanceDag::walk(&pool, wrap, MAX_DEPTH).with_mono_edges(&pool, &[inst]);
    assert_eq!(
        dag.divergences.len(),
        1,
        "a generic leaf under a concrete root is a structural divergence"
    );
    assert_eq!(dag.divergences[0].generic_idx, t);
    assert_eq!(
        dag.divergences[0].concrete_idx, wrap,
        "no concrete mono substitution -> concrete_idx falls back to the resolved root"
    );
}

/// Tracing a bare generic leaf AS the root yields ZERO divergences — there is no
/// concrete instantiation context (the root itself is generic). Negative pin:
/// the structural divergence requires a CONCRETE root, not merely a generic leaf.
#[test]
fn generic_root_yields_no_divergence() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let t = pool.named(interner.intern("T")); // unresolved -> generic root
    let dag = ProvenanceDag::walk(&pool, t, MAX_DEPTH).with_mono_edges(&pool, &[]);
    assert!(
        dag.divergences.is_empty(),
        "a generic root has no concrete instantiation context -> no divergence"
    );
}

/// A mono substitution to a NESTED aggregate that still carries a generic leaf
/// (`Inner { g: U }`, `U` unresolved) is NOT a concrete counterpart: the
/// `body_carries_generic_leaf` walk must descend the substituted STRUCT, find
/// the deeper leaf, and decline to record it — so the divergence falls back to
/// the resolved root. Pin: fails if the struct-descent arm stops recursing and
/// treats the nested-generic body as concrete (refines to it).
#[test]
fn mono_substitution_to_nested_generic_struct_is_not_concrete() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let t = pool.named(interner.intern("T"));
    let wrap = pool.struct_type(interner.intern("Wrap"), &[(interner.intern("inner"), t)]);
    // Substituted body is a struct carrying its OWN unresolved generic leaf.
    let u = pool.named(interner.intern("U"));
    let inner_generic = pool.struct_type(interner.intern("Inner"), &[(interner.intern("g"), u)]);

    let inst = mono_instance(&interner, vec![(t, inner_generic)]);
    let dag = ProvenanceDag::walk(&pool, wrap, MAX_DEPTH).with_mono_edges(&pool, &[inst]);
    assert_eq!(
        dag.divergences.len(),
        1,
        "generic leaf under a concrete root"
    );
    assert_eq!(dag.divergences[0].generic_idx, t);
    assert_eq!(
        dag.divergences[0].concrete_idx, wrap,
        "a substitution to a nested-generic struct is not concrete -> fall back to root"
    );
}

/// Enum-descent twin of the struct case: a mono substitution to an enum whose
/// variant payload carries a generic leaf is NOT concrete. Pin: fails if the
/// enum-descent arm of `body_carries_generic_leaf` stops recursing.
#[test]
fn mono_substitution_to_nested_generic_enum_is_not_concrete() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let t = pool.named(interner.intern("T"));
    let wrap = pool.struct_type(interner.intern("Wrap"), &[(interner.intern("inner"), t)]);
    let u = pool.named(interner.intern("U"));
    let inner_enum = pool.enum_type(
        interner.intern("InnerE"),
        &[EnumVariant {
            name: interner.intern("V"),
            field_types: vec![u],
        }],
    );

    let inst = mono_instance(&interner, vec![(t, inner_enum)]);
    let dag = ProvenanceDag::walk(&pool, wrap, MAX_DEPTH).with_mono_edges(&pool, &[inst]);
    assert_eq!(dag.divergences.len(), 1);
    assert_eq!(
        dag.divergences[0].concrete_idx, wrap,
        "a substitution to a nested-generic enum is not concrete -> fall back to root"
    );
}

/// Positive pin clamping the descent's other arm: a mono substitution to a
/// FULLY-CONCRETE nested aggregate (`Inner { g: int }`) IS a concrete
/// counterpart — the descent recurses the substituted struct, finds NO generic
/// leaf, records it, and refines the divergence to that nested concrete body.
/// Fails if the descent always reports a generic leaf (never records a nested
/// concrete body), the inverse of the two negative descent pins.
#[test]
fn mono_substitution_to_nested_concrete_struct_refines_divergence() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let t = pool.named(interner.intern("T"));
    let wrap = pool.struct_type(interner.intern("Wrap"), &[(interner.intern("inner"), t)]);
    let inner_concrete = pool.struct_type(
        interner.intern("Inner"),
        &[(interner.intern("g"), Idx::INT)],
    );

    let inst = mono_instance(&interner, vec![(t, inner_concrete)]);
    let dag = ProvenanceDag::walk(&pool, wrap, MAX_DEPTH).with_mono_edges(&pool, &[inst]);
    assert_eq!(dag.divergences.len(), 1);
    assert_eq!(dag.divergences[0].generic_idx, t);
    assert_eq!(
        dag.divergences[0].concrete_idx, inner_concrete,
        "a substitution to a fully-concrete nested struct is concrete -> refine to it"
    );
}

// TDD matrix: nesting {struct-in-struct, enum-in-enum, struct-in-enum,
// enum-in-struct} x divergence {clean-monomorphic, generic-leaf-divergent}.
// Per-cell edge-set + divergence-flag assertions, with a count-assert proving
// all 8 cells are visited.

#[derive(Clone, Copy)]
enum Nesting {
    StructInStruct,
    EnumInEnum,
    StructInEnum,
    EnumInStruct,
}

const ALL_NESTINGS: [Nesting; 4] = [
    Nesting::StructInStruct,
    Nesting::EnumInEnum,
    Nesting::StructInEnum,
    Nesting::EnumInStruct,
];

/// Build one matrix cell. `divergent` chooses the inner leaf: an unresolved
/// `Named "T"` (generic-leaf) when true, `int` (concrete) when false. Returns
/// `(pool, root, leaf_idx)`.
fn build_cell(interner: &StringInterner, nesting: Nesting, divergent: bool) -> (Pool, Idx, Idx) {
    let mut pool = Pool::new();
    let leaf = if divergent {
        pool.named(interner.intern("T")) // unresolved Named -> generic leaf
    } else {
        Idx::INT
    };
    let f = interner.intern("inner");
    let v = interner.intern("V");
    let w = interner.intern("W");
    let root = match nesting {
        Nesting::StructInStruct => {
            let inner = pool.struct_type(interner.intern("Inner"), &[(f, leaf)]);
            pool.struct_type(interner.intern("Outer"), &[(f, inner)])
        }
        Nesting::EnumInEnum => {
            let inner = pool.enum_type(
                interner.intern("Inner"),
                &[EnumVariant {
                    name: w,
                    field_types: vec![leaf],
                }],
            );
            pool.enum_type(
                interner.intern("Outer"),
                &[EnumVariant {
                    name: v,
                    field_types: vec![inner],
                }],
            )
        }
        Nesting::StructInEnum => {
            let inner = pool.struct_type(interner.intern("Inner"), &[(f, leaf)]);
            pool.enum_type(
                interner.intern("Outer"),
                &[EnumVariant {
                    name: v,
                    field_types: vec![inner],
                }],
            )
        }
        Nesting::EnumInStruct => {
            let inner = pool.enum_type(
                interner.intern("Inner"),
                &[EnumVariant {
                    name: w,
                    field_types: vec![leaf],
                }],
            );
            pool.struct_type(interner.intern("Outer"), &[(f, inner)])
        }
    };
    (pool, root, leaf)
}

#[test]
fn provenance_edge_matrix_nesting_x_divergence() {
    let interner = StringInterner::new();
    let mut count = 0;
    for nesting in ALL_NESTINGS {
        for divergent in [false, true] {
            let (pool, root, leaf) = build_cell(&interner, nesting, divergent);
            let instances = if divergent {
                vec![mono_instance(&interner, vec![(leaf, Idx::INT)])]
            } else {
                vec![mono_instance(&interner, vec![(Idx::INT, Idx::INT)])]
            };
            let dag =
                ProvenanceDag::walk(&pool, root, MAX_DEPTH).with_mono_edges(&pool, &instances);

            // Every cell has the two-level nesting -> >= 2 structure edges.
            assert!(
                dag.edges.len() >= 2,
                "two-level nesting yields >= 2 structure edges"
            );
            if divergent {
                assert_eq!(
                    dag.divergences.len(),
                    1,
                    "generic-leaf-divergent cell flags exactly one divergence"
                );
                assert_eq!(dag.divergences[0].generic_idx, leaf);
                assert_eq!(dag.divergences[0].concrete_idx, Idx::INT);
            } else {
                assert!(
                    dag.divergences.is_empty(),
                    "clean-monomorphic cell flags no divergence"
                );
            }
            count += 1;
        }
    }
    assert_eq!(count, ALL_NESTINGS.len() * 2, "all 8 matrix cells visited");
}
