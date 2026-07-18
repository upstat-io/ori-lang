//! Unit pins for [`Pool::structural_eq`] — the SSOT both mono-dispatch fallback
//! sites (`ori_llvm` `apply.rs lookup_mono_dispatch`, `nounwind/prepare.rs
//! resolve_mono_target`) call to match a monomorphized call's arg types against
//! a declared specialization's param types across the merged/imported pool
//! boundary.
//!
//! Regression: the fallbacks formerly matched by raw `Idx`-identity (`p == a`),
//! so a param `[int]` and an arg `[int]` interned to distinct `Idx` (a binding
//! `[int]` vs a literal `[int]` in the merged mono pool) failed to match and the
//! specialization was never selected ("missing mono instance"). The cure routes
//! both fallbacks through this predicate's recursive `Tag`+children comparison,
//! which reports equal regardless of `Idx` identity.
//!
//! Distinct-`Idx`-same-structure is a pipeline-internal artifact of the
//! merged-mono pool (the host-resolved arg type and the re-interned param type
//! land at distinct `Idx` for one specialization). It is NOT reproducible
//! through the public pool API — content-addressed interning (TI-1) collapses
//! every var-free structural type to a single `Idx`, even across a cross-pool
//! re-intern. So these pins exercise the recursive branches directly (which the
//! distinct-`Idx` case routes through once the fast path misses); the
//! distinct-`Idx` integration behavior is pinned end-to-end by the spec corpus
//! and the AOT generic fixtures.

use crate::{EnumVariant, Idx, Pool, Tag};

/// The load-bearing cure pin: two `[int]` interned to DISTINCT `Idx` (the
/// merged-mono-pool artifact, reproduced via `intern_distinct_for_test`) MUST
/// compare structurally-equal. This is the exact `Idx(222)` vs `Idx(232)` shape
/// the fallback sites face. Under the reverted raw-`Idx`-identity predicate
/// (`a == b` only) this FAILS — `Idx`-identity is `false` for distinct `Idx` —
/// which is precisely the "missing mono instance" regression.
#[test]
fn distinctly_interned_list_int_pair_is_structurally_equal() {
    let mut p = Pool::new();
    let list_int_a = p.list(Idx::INT);
    // A second `[int]` at a distinct `Idx`, bypassing dedup — the merged-pool shape.
    let list_int_b = p.intern_distinct_for_test(Tag::List, Idx::INT.raw());

    assert_ne!(
        list_int_a, list_int_b,
        "test setup: the two [int] must be distinctly interned (={list_int_a:?} vs {list_int_b:?})"
    );
    assert!(
        p.structural_eq(list_int_a, list_int_b),
        "distinctly-interned [int] (={list_int_a:?}) and [int] (={list_int_b:?}) must be structurally equal"
    );
}

/// Negative companion to the load-bearing pin: a distinctly-interned `[str]`
/// must NOT match `[int]` — the recursion's child comparison must reject, so the
/// cure cannot over-match and dispatch to the wrong specialization.
#[test]
fn distinctly_interned_list_str_does_not_match_list_int() {
    let mut p = Pool::new();
    let list_int = p.list(Idx::INT);
    let list_str = p.intern_distinct_for_test(Tag::List, Idx::STR.raw());

    assert_ne!(list_int, list_str, "test setup: distinct Idx");
    assert!(
        !p.structural_eq(list_int, list_str),
        "distinctly-interned [str] must NOT match [int]"
    );
}

/// Positive matrix: every container/aggregate tag the fallback can carry must
/// report structural equality for its own structure. `Idx`-identity is the fast
/// path; these assert the recursive branches the cure depends on.
#[test]
fn structurally_equal_types_match_across_tag_families() {
    let mut p = Pool::new();

    // Simple container (List) — the distinct-interned-list shape.
    let list_int = p.list(Idx::INT);
    assert!(p.structural_eq(list_int, list_int), "list<int> ≡ list<int>");

    // Two-child container (Map).
    let map_int_str = p.map(Idx::INT, Idx::STR);
    assert!(
        p.structural_eq(map_int_str, map_int_str),
        "map<int,str> ≡ map<int,str>"
    );

    // Two-child container (Result).
    let result_int_str = p.result(Idx::INT, Idx::STR);
    assert!(
        p.structural_eq(result_int_str, result_int_str),
        "result<int,str> ≡ result<int,str>"
    );

    // Nested container — exercises the recursion.
    let list_list_int = p.list(list_int);
    assert!(
        p.structural_eq(list_list_int, list_list_int),
        "list<list<int>> ≡ list<list<int>>"
    );

    // Tuple — slice recursion.
    let tuple = p.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
    assert!(
        p.structural_eq(tuple, tuple),
        "(int, str, bool) ≡ (int, str, bool)"
    );

    // Struct — named + field-slice recursion.
    let sname = ori_ir::Name::from_raw(101);
    let fx = ori_ir::Name::from_raw(102);
    let fy = ori_ir::Name::from_raw(103);
    let point = p.struct_type(sname, &[(fx, Idx::INT), (fy, Idx::INT)]);
    assert!(
        p.structural_eq(point, point),
        "Point {{ x: int, y: int }} ≡ itself"
    );

    // Primitives — leaf branch.
    assert!(p.structural_eq(Idx::INT, Idx::INT), "int ≡ int");
}

/// Negative matrix: structurally-distinct types must NOT match. Pairs each
/// positive cell so the fallback cannot drift toward over-matching (which would
/// dispatch a call to the WRONG specialization — silent miscompilation).
#[test]
fn structurally_distinct_types_do_not_match() {
    let mut p = Pool::new();

    // Negative pin: [str] must NOT match [int].
    let list_int = p.list(Idx::INT);
    let list_str = p.list(Idx::STR);
    assert!(
        !p.structural_eq(list_int, list_str),
        "list<int> must NOT match list<str>"
    );

    // Different tag, same child kind: list<int> vs set<int>.
    let set_int = p.set(Idx::INT);
    assert!(
        !p.structural_eq(list_int, set_int),
        "list<int> must NOT match set<int> (different tag)"
    );

    // Two-child: map<int,str> vs map<str,int> (key/value swapped).
    let map_int_str = p.map(Idx::INT, Idx::STR);
    let map_str_int = p.map(Idx::STR, Idx::INT);
    assert!(
        !p.structural_eq(map_int_str, map_str_int),
        "map<int,str> must NOT match map<str,int>"
    );

    // Different arity: (int) vs (int, str).
    let tuple1 = p.tuple(&[Idx::INT]);
    let tuple2 = p.tuple(&[Idx::INT, Idx::STR]);
    assert!(
        !p.structural_eq(tuple1, tuple2),
        "(int) must NOT match (int, str)"
    );

    // Different struct field type: same name + field names, different field
    // type — the field-recursion branch must reject.
    let sname = ori_ir::Name::from_raw(101);
    let fx = ori_ir::Name::from_raw(102);
    let fy = ori_ir::Name::from_raw(103);
    let point_int = p.struct_type(sname, &[(fx, Idx::INT), (fy, Idx::INT)]);
    let point_str = p.struct_type(sname, &[(fx, Idx::STR), (fy, Idx::STR)]);
    assert!(
        !p.structural_eq(point_int, point_str),
        "Point {{ x: int }} must NOT match Point {{ x: str }}"
    );

    // Primitive vs container.
    assert!(
        !p.structural_eq(Idx::INT, list_int),
        "int must NOT match list<int>"
    );
}

#[test]
fn representation_equality_distinguishes_enum_payloads() {
    let mut pool = Pool::new();
    let enum_name = ori_ir::Name::from_raw(201);
    let variant_name = ori_ir::Name::from_raw(202);
    let enum_int = pool.enum_type(
        enum_name,
        &[EnumVariant {
            name: variant_name,
            field_types: vec![Idx::INT],
        }],
    );
    let enum_string = pool.enum_type(
        enum_name,
        &[EnumVariant {
            name: variant_name,
            field_types: vec![Idx::STR],
        }],
    );

    assert!(pool.structural_eq(enum_int, enum_string));
    assert!(pool.representation_eq(enum_int, enum_int));
    assert!(!pool.representation_eq(enum_int, enum_string));
}
