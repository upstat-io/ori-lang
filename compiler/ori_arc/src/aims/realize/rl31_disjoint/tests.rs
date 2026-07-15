//! RL-31 disjointness-proof matrix.
//!
//! Pins the 8-clause SUFFICIENT-condition rule against the worked
//! examples (`merge` Borrowed+Borrowed, `accumulate` Owned+Borrowed) plus
//! negative cells: same-type pairs (clause 4/7), sum-node closures (clause 5),
//! scalar params (reference-class precondition), opaque/unresolved (clause 3
//! fail-closed). Every positive cell has a negative counterpart; a false
//! disjointness fact can miscompile any consuming backend.

use ori_types::{Idx, LifetimeId, Pool};

use super::{builtin_burden_present, prove_param_disjointness};

/// `merge(a: &{str:int}, b: &[int])` — Worked Example 1b (Borrowed+Borrowed).
/// Both Borrowed params over disjoint-type pointees satisfy the type facet.
#[test]
fn merge_borrowed_pair_distinct_collections_type_disjoint() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(map_str_int, LifetimeId::SCOPED);
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);

    let facts = prove_param_disjointness(&[a, b], &pool);

    assert_eq!(
        facts.type_disjointness(),
        [true, true],
        "both Borrowed params over disjoint pointee closures satisfy the type facet"
    );
}

/// `accumulate(a: {str:int}, b: [int])` — Worked Example 1 (Owned+Borrowed
/// generalization). Owned `a` + Borrowed `b` over disjoint types → both
/// satisfy the type facet, independent of Access.
#[test]
fn accumulate_owned_borrowed_distinct_collections_type_disjoint() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_int = pool.list(Idx::INT);
    // a: {str:int} (Owned — used directly), b: &[int] (Borrowed).
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);

    let facts = prove_param_disjointness(&[map_str_int, b], &pool);

    assert_eq!(
        facts.type_disjointness(),
        [true, true],
        "Owned+Borrowed disjoint-type pair must satisfy the RL-31 type facet"
    );
}

/// Negative pin (clause 4 + clause 7): `swap(a: [int], b: [int])` — same type.
/// Closures intersect on `[int]` AND surface types identical → neither
/// type-disjoint. A wrong fact here permits a same-binding `f(x, x)` miscompile.
#[test]
fn swap_same_type_lists_neither_type_disjoint() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    // Same Idx for both params (interning dedups identical types).
    let facts = prove_param_disjointness(&[list_int, list_int], &pool);

    assert_eq!(
        facts.type_disjointness(),
        [false, false],
        "same-type list params are not type-disjoint because callers may pass f(x, x)"
    );
}

/// Negative pin (clause 4 element-sharing): `f(a: [str], b: {str:int})`.
/// Both reach `str` in their closures → closures intersect on `str` → neither
/// type-disjoint even though the outer types differ. Element-level reachable-memory
/// overlap blocks the claim.
#[test]
fn shared_element_type_blocks_type_disjointness() {
    let mut pool = Pool::new();
    let list_str = pool.list(Idx::STR);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let a = pool.borrowed(list_str, LifetimeId::SCOPED);
    let b = pool.borrowed(map_str_int, LifetimeId::SCOPED);

    let facts = prove_param_disjointness(&[a, b], &pool);

    assert_eq!(
        facts.type_disjointness(),
        [false, false],
        "params whose closures both reach `str` share reachable memory — not disjoint"
    );
}

/// Negative pin (clause 5, transitive sum node): `f(a: [Option<str>], b: [int])`.
/// `a`'s closure reaches an `Option` node (`SetTag` payload-invalidation per
/// RL-10/DP-5) → `a` fails closed; `b` has no OTHER reference-class param to be
/// disjoint from, so `b` satisfies the type facet while `a` does not.
#[test]
fn transitive_sum_node_fails_closed() {
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);
    let list_opt = pool.list(opt_str);
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(list_opt, LifetimeId::SCOPED);
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);

    let facts = prove_param_disjointness(&[a, b], &pool);

    assert!(
        !facts.type_disjointness()[0],
        "param reaching a transitive Option/sum node must fail clause 5 (fail closed)"
    );
    assert!(
        facts.type_disjointness()[1],
        "the non-sum param has no other reference-class peer"
    );
}

/// Negative pin (reference-class precondition): scalar params carry no
/// reachable heap memory, so they never satisfy the type facet.
#[test]
fn scalar_params_never_type_disjoint() {
    let pool = Pool::new();
    let facts = prove_param_disjointness(&[Idx::INT, Idx::FLOAT], &pool);

    assert_eq!(
        facts.type_disjointness(),
        [false, false],
        "scalar params carry no aliasable reachable memory represented by RL-31"
    );
}

/// Negative pin (clause 3 fail-closed): an unresolved/named type whose
/// structure is not reachable at this phase fails closed.
#[test]
fn unresolved_named_type_fails_closed() {
    let pool = Pool::new();
    // Idx::ERROR is poison/unresolvable; a real Tag::Named without registry
    // resolution is the production fail-closed case. ERROR is scalar-filtered
    // (no closure) so the type facet is false by the reference-class precondition,
    // which is itself the conservative-safe outcome.
    let facts = prove_param_disjointness(&[Idx::ERROR, Idx::STR], &pool);

    assert!(
        !facts.type_disjointness()[0],
        "unresolvable/poison param must fail closed"
    );
}

/// Positive: distinct nominal struct types with disjoint field closures are
/// type-disjoint — the struct node itself enters the closure so two structs with
/// shared scalar fields do not falsely intersect.
#[test]
fn distinct_struct_types_with_disjoint_heap_type_disjoint() {
    let mut pool = Pool::new();
    let s_name = ori_ir::Name::from_raw(100);
    let t_name = ori_ir::Name::from_raw(101);
    let f_name = ori_ir::Name::from_raw(102);
    // S { f: str }, T { f: [int] } — distinct struct Idxs and reachable graphs.
    let list_int = pool.list(Idx::INT);
    let s = pool.struct_type(s_name, &[(f_name, Idx::STR)]);
    let t = pool.struct_type(t_name, &[(f_name, list_int)]);
    let a = pool.borrowed(s, LifetimeId::SCOPED);
    let b = pool.borrowed(t, LifetimeId::SCOPED);

    let facts = prove_param_disjointness(&[a, b], &pool);

    assert_eq!(
        facts.type_disjointness(),
        [true, true],
        "distinct struct types with disjoint heap-field closures satisfy the type facet"
    );
}

/// Three-param self-verifying matrix: `f(a: &{str:int}, b: &[int], c: int)`.
/// The a/b type facets hold; the scalar c facet does not. Count assertion
/// proves every cell visited.
#[test]
fn three_param_mixed_matrix_visits_every_cell() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(map_str_int, LifetimeId::SCOPED);
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);
    let params = [a, b, Idx::INT];

    let facts = prove_param_disjointness(&params, &pool);

    assert_eq!(facts.type_disjointness().len(), 3, "one fact per parameter");
    assert_eq!(
        facts.type_disjointness(),
        [true, true, false],
        "disjoint reference-class a/b satisfy the type facet; scalar c does not"
    );
}

/// A single reference-class param has no peer to alias, so it is vacuously
/// disjoint from the empty set of other reference-class parameters.
#[test]
fn single_reference_param_is_type_disjoint() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(list_int, LifetimeId::SCOPED);

    let facts = prove_param_disjointness(&[a], &pool);

    assert_eq!(facts.type_disjointness(), [true]);
}

/// Negative pin (RL-31 (P2) dual-facet): the type-level facet (b) proves
/// disjoint reachable closures, but the per-call-site provenance facet (a) is
/// not shipped. The combined fact must remain false even while the type facet
/// is true. Distinct reachable types can alias the same runtime memory, so a
/// consumer may never project facet (b) alone. If the provenance facet ships,
/// this pin flips to assert true.
#[test]
fn call_site_provenance_facet_unproven_blocks_combined_fact() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(map_str_int, LifetimeId::SCOPED);
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);

    let facts = prove_param_disjointness(&[a, b], &pool);

    // Facet (b) still proves type-level disjointness (the precise half).
    assert_eq!(
        facts.type_disjointness(),
        [true, true],
        "type-level facet (b) must still prove disjoint distinct-type closures"
    );
    assert!(
        !facts.call_site_provenance_disjoint(),
        "per-call-site provenance facet (a) is unshipped"
    );
    assert!(
        !facts.proves_disjoint(0) && !facts.proves_disjoint(1),
        "the combined fact must fail closed when either facet is unproven"
    );
    assert!(
        !facts.proves_disjoint(usize::MAX),
        "an invalid parameter index must fail closed"
    );
}

/// Builtin-burden bridge (clause 8 builtin side): str/list/map resolve to a
/// `BURDEN_TABLE` entry without a `TypeRegistry`; a scalar does not.
#[test]
fn builtin_burden_present_for_collections_not_scalars() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let map_str_int = pool.map(Idx::STR, Idx::INT);

    assert!(
        builtin_burden_present(Idx::STR, &pool),
        "str has a builtin burden entry"
    );
    assert!(
        builtin_burden_present(list_int, &pool),
        "list has a builtin burden entry"
    );
    assert!(
        builtin_burden_present(map_str_int, &pool),
        "map has a builtin burden entry"
    );
    assert!(
        !builtin_burden_present(Idx::INT, &pool),
        "scalar int has no burden entry"
    );
}
