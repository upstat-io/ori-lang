//! RL-31 disjointness-proof matrix.
//!
//! Pins the 8-clause SUFFICIENT-condition rule against the worked
//! examples (`merge` Borrowed+Borrowed, `accumulate` Owned+Borrowed) plus
//! negative cells: same-type pairs (clause 4/7), sum-node closures (clause 5),
//! scalar params (reference-class precondition), opaque/unresolved (clause 3
//! fail-closed). Every positive cell has a negative counterpart; a wrong
//! `noalias` is a miscompile, so the negatives are the load-bearing pins.

use ori_types::{Idx, LifetimeId, Pool};

use super::{builtin_burden_present, prove_param_noalias};

/// `merge(a: &{str:int}, b: &[int])` — Worked Example 1b (Borrowed+Borrowed).
/// Both Borrowed params over disjoint-type pointees → both eligible.
#[test]
fn merge_borrowed_pair_distinct_collections_both_eligible() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(map_str_int, LifetimeId::SCOPED);
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);

    let proof = prove_param_noalias(&[a, b], &pool);

    assert_eq!(
        proof.eligible,
        vec![true, true],
        "both Borrowed params over disjoint pointee closures must be noalias-eligible"
    );
}

/// `accumulate(a: {str:int}, b: [int])` — Worked Example 1 (Owned+Borrowed
/// generalization). Owned `a` + Borrowed `b` over disjoint types → both
/// eligible (the proof rests on `TypeId` disjointness, independent of Access).
#[test]
fn accumulate_owned_borrowed_distinct_collections_both_eligible() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_int = pool.list(Idx::INT);
    // a: {str:int} (Owned — used directly), b: &[int] (Borrowed).
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);

    let proof = prove_param_noalias(&[map_str_int, b], &pool);

    assert_eq!(
        proof.eligible,
        vec![true, true],
        "Owned+Borrowed disjoint-type pair must be noalias-eligible (RL-31 generalization)"
    );
}

/// Negative pin (clause 4 + clause 7): `swap(a: [int], b: [int])` — same type.
/// Closures intersect on `[int]` AND surface types identical → neither
/// eligible. A wrong `noalias` here is a same-binding `f(x, x)` miscompile.
#[test]
fn swap_same_type_lists_neither_eligible() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    // Same Idx for both params (interning dedups identical types).
    let proof = prove_param_noalias(&[list_int, list_int], &pool);

    assert_eq!(
        proof.eligible,
        vec![false, false],
        "same-type list params must NOT be noalias-eligible (could be passed f(x, x))"
    );
}

/// Negative pin (clause 4 element-sharing): `f(a: [str], b: {str:int})`.
/// Both reach `str` in their closures → closures intersect on `str` → neither
/// eligible even though the outer types differ. Element-level reachable-memory
/// overlap blocks the claim.
#[test]
fn shared_element_type_blocks_eligibility() {
    let mut pool = Pool::new();
    let list_str = pool.list(Idx::STR);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let a = pool.borrowed(list_str, LifetimeId::SCOPED);
    let b = pool.borrowed(map_str_int, LifetimeId::SCOPED);

    let proof = prove_param_noalias(&[a, b], &pool);

    assert_eq!(
        proof.eligible,
        vec![false, false],
        "params whose closures both reach `str` share reachable memory — not disjoint"
    );
}

/// Negative pin (clause 5, transitive sum node): `f(a: [Option<str>], b: [int])`.
/// `a`'s closure reaches an `Option` node (`SetTag` payload-invalidation per
/// RL-10/DP-5) → `a` fails closed; `b` has no OTHER reference-class param to be
/// disjoint from, so `b` is eligible while `a` is not.
#[test]
fn transitive_sum_node_fails_closed() {
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);
    let list_opt = pool.list(opt_str);
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(list_opt, LifetimeId::SCOPED);
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);

    let proof = prove_param_noalias(&[a, b], &pool);

    assert!(
        !proof.eligible[0],
        "param reaching a transitive Option/sum node must fail clause 5 (fail closed)"
    );
    assert!(
        proof.eligible[1],
        "the non-sum param has no other reference-class peer — stays eligible"
    );
}

/// Negative pin (reference-class precondition): scalar params carry no
/// reachable heap memory → never eligible. `noalias` on `int` is meaningless.
#[test]
fn scalar_params_never_eligible() {
    let pool = Pool::new();
    let proof = prove_param_noalias(&[Idx::INT, Idx::FLOAT], &pool);

    assert_eq!(
        proof.eligible,
        vec![false, false],
        "scalar params carry no aliasable reachable memory — not noalias-eligible"
    );
}

/// Negative pin (clause 3 fail-closed): an unresolved/named type whose
/// structure is not reachable at this phase fails closed — never eligible.
#[test]
fn unresolved_named_type_fails_closed() {
    let pool = Pool::new();
    // Idx::ERROR is poison/unresolvable; a real Tag::Named without registry
    // resolution is the production fail-closed case. ERROR is scalar-filtered
    // (no closure) so it is non-eligible by the reference-class precondition,
    // which is itself the conservative-safe outcome.
    let proof = prove_param_noalias(&[Idx::ERROR, Idx::STR], &pool);

    assert!(
        !proof.eligible[0],
        "unresolvable/poison param must never be noalias-eligible (fail closed)"
    );
}

/// Positive: distinct nominal struct types with disjoint field closures are
/// eligible — the struct node itself enters the closure so two structs with
/// shared scalar fields do not falsely intersect.
#[test]
fn distinct_struct_types_with_disjoint_heap_eligible() {
    let mut pool = Pool::new();
    let s_name = ori_ir::Name::from_raw(100);
    let t_name = ori_ir::Name::from_raw(101);
    let f_name = ori_ir::Name::from_raw(102);
    // S { f: str }, T { f: [int] } — distinct struct Idxs, distinct heap closures.
    let list_int = pool.list(Idx::INT);
    let s = pool.struct_type(s_name, &[(f_name, Idx::STR)]);
    let t = pool.struct_type(t_name, &[(f_name, list_int)]);
    let a = pool.borrowed(s, LifetimeId::SCOPED);
    let b = pool.borrowed(t, LifetimeId::SCOPED);

    let proof = prove_param_noalias(&[a, b], &pool);

    assert_eq!(
        proof.eligible,
        vec![true, true],
        "distinct struct types with disjoint heap-field closures are noalias-eligible"
    );
}

/// Three-param self-verifying matrix: `f(a: &{str:int}, b: &[int], c: int)`.
/// a/b disjoint reference-class → eligible; c scalar → not. Count assertion
/// proves every cell visited.
#[test]
fn three_param_mixed_matrix_visits_every_cell() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(map_str_int, LifetimeId::SCOPED);
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);
    let params = [a, b, Idx::INT];

    let proof = prove_param_noalias(&params, &pool);

    assert_eq!(proof.eligible.len(), 3, "one verdict per parameter");
    assert_eq!(
        proof.eligible,
        vec![true, true, false],
        "disjoint reference-class a/b eligible; scalar c not"
    );
}

/// Single reference-class param has no peer to alias → eligible (vacuously
/// disjoint from the empty set of other reference-class params).
#[test]
fn single_reference_param_is_eligible() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(list_int, LifetimeId::SCOPED);

    let proof = prove_param_noalias(&[a], &pool);

    assert_eq!(proof.eligible, vec![true]);
}

/// Negative pin (RL-31 (P2) dual-facet): the type-level facet (b) proves
/// disjoint reachable closures, but the per-call-site provenance facet (a) is
/// NOT shipped. `prove_param_noalias` MUST report `call_site_provenance_proven
/// == false` — the signal codegen reads to OMIT the function-attribute
/// `noalias`. Emitting on facet (b) alone is the (P2)-rejected unsound verdict
/// (`RL31_type_facet_alone_insufficient`): a function-attribute `noalias`
/// asserts NO caller can EVER pass aliasing args, which facet (b) cannot
/// establish (distinct reachable types CAN alias the same runtime memory). If
/// the per-call-site provenance facet (a) ships, this pin flips to assert
/// `true`; until then it guards against re-introducing the facet-(b)-alone
/// over-emission (the UAF the cure closed).
#[test]
fn call_site_provenance_facet_unproven_blocks_function_attribute_noalias() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let list_int = pool.list(Idx::INT);
    let a = pool.borrowed(map_str_int, LifetimeId::SCOPED);
    let b = pool.borrowed(list_int, LifetimeId::SCOPED);

    let proof = prove_param_noalias(&[a, b], &pool);

    // Facet (b) still proves type-level disjointness (the precise half).
    assert_eq!(
        proof.eligible,
        vec![true, true],
        "type-level facet (b) must still prove disjoint distinct-type closures"
    );
    // Facet (a) is unshipped → the function-attribute noalias must be omitted.
    assert!(
        !proof.call_site_provenance_proven,
        "per-call-site provenance facet (a) is unshipped — must be false so codegen omits noalias (RL-31 (P2))"
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
