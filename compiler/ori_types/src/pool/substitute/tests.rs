//! Tests for `substitute_in_pool`.

use rustc_hash::FxHashMap;

use super::substitute_in_pool;
use crate::{GeneralizedVarState, Idx, Pool, Tag};

/// Helper: build a `var_subst` map from `[(var_id, concrete_idx)]` pairs.
fn subst(pairs: &[(u32, Idx)]) -> FxHashMap<u32, Idx> {
    pairs.iter().copied().collect()
}

// Primitives (fast path)

#[test]
fn primitive_unchanged() {
    let mut pool = Pool::new();
    let map = subst(&[(0, Idx::BOOL)]);

    // Primitives have no HAS_VAR flag, so they're returned as-is.
    assert_eq!(substitute_in_pool(&mut pool, Idx::INT, &map), Idx::INT);
    assert_eq!(substitute_in_pool(&mut pool, Idx::STR, &map), Idx::STR);
    assert_eq!(substitute_in_pool(&mut pool, Idx::UNIT, &map), Idx::UNIT);
}

// Single variable

#[test]
fn var_substituted() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);

    let map = subst(&[(var_id, Idx::INT)]);
    assert_eq!(substitute_in_pool(&mut pool, var, &map), Idx::INT);
}

#[test]
fn var_not_in_map_unchanged() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();

    // Empty map — var stays as-is.
    let map = subst(&[]);
    assert_eq!(substitute_in_pool(&mut pool, var, &map), var);
}

// Single-child containers

#[test]
fn list_of_var_substituted() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    let list_var = pool.list(var);

    let map = subst(&[(var_id, Idx::STR)]);
    let result = substitute_in_pool(&mut pool, list_var, &map);

    assert_eq!(pool.tag(result), Tag::List);
    let child = Idx::from_raw(pool.data(result));
    assert_eq!(child, Idx::STR);
}

#[test]
fn option_of_var_substituted() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    let opt_var = pool.option(var);

    let map = subst(&[(var_id, Idx::INT)]);
    let result = substitute_in_pool(&mut pool, opt_var, &map);

    assert_eq!(pool.tag(result), Tag::Option);
    let inner = Idx::from_raw(pool.data(result));
    assert_eq!(inner, Idx::INT);
}

#[test]
fn list_of_primitive_unchanged() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    let map = subst(&[(99, Idx::BOOL)]);
    // List<int> has no vars — fast path returns it unchanged.
    assert_eq!(substitute_in_pool(&mut pool, list_int, &map), list_int);
}

// Two-child containers

#[test]
fn map_both_vars_substituted() {
    let mut pool = Pool::new();
    let k = pool.fresh_var();
    let v = pool.fresh_var();
    let k_id = pool.data(k);
    let v_id = pool.data(v);
    let map_type = pool.map(k, v);

    let map = subst(&[(k_id, Idx::STR), (v_id, Idx::INT)]);
    let result = substitute_in_pool(&mut pool, map_type, &map);

    assert_eq!(pool.tag(result), Tag::Map);
    assert_eq!(pool.map_key(result), Idx::STR);
    assert_eq!(pool.map_value(result), Idx::INT);
}

#[test]
fn result_ok_only_substituted() {
    let mut pool = Pool::new();
    let ok = pool.fresh_var();
    let ok_id = pool.data(ok);
    let result_type = pool.result(ok, Idx::STR);

    let map = subst(&[(ok_id, Idx::INT)]);
    let result = substitute_in_pool(&mut pool, result_type, &map);

    assert_eq!(pool.tag(result), Tag::Result);
    assert_eq!(pool.result_ok(result), Idx::INT);
    assert_eq!(pool.result_err(result), Idx::STR);
}

// Function type

#[test]
fn function_params_and_return_substituted() {
    let mut pool = Pool::new();
    let var_a = pool.fresh_var();
    let var_b = pool.fresh_var();
    let a_id = pool.data(var_a);
    let b_id = pool.data(var_b);
    let fn_type = pool.function(&[var_a, Idx::INT], var_b);

    let map = subst(&[(a_id, Idx::STR), (b_id, Idx::BOOL)]);
    let result = substitute_in_pool(&mut pool, fn_type, &map);

    assert_eq!(pool.tag(result), Tag::Function);
    let params = pool.function_params(result);
    assert_eq!(params, &[Idx::STR, Idx::INT]);
    assert_eq!(pool.function_return(result), Idx::BOOL);
}

// Tuple

#[test]
fn tuple_elements_substituted() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    let tup = pool.tuple(&[var, Idx::BOOL, var]);

    let map = subst(&[(var_id, Idx::FLOAT)]);
    let result = substitute_in_pool(&mut pool, tup, &map);

    assert_eq!(pool.tag(result), Tag::Tuple);
    let elems = pool.tuple_elems(result);
    assert_eq!(elems, &[Idx::FLOAT, Idx::BOOL, Idx::FLOAT]);
}

// Applied

#[test]
fn applied_args_substituted() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);

    let interner = ori_ir::StringInterner::new();
    let name = interner.intern("MyType");
    let applied = pool.applied(name, &[var, Idx::INT]);

    let map = subst(&[(var_id, Idx::CHAR)]);
    let result = substitute_in_pool(&mut pool, applied, &map);

    assert_eq!(pool.tag(result), Tag::Applied);
    assert_eq!(pool.applied_name(result), name);
    let args = pool.applied_args(result);
    assert_eq!(args, &[Idx::CHAR, Idx::INT]);
}

// Nested types

#[test]
fn nested_list_of_option_of_var() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    let opt = pool.option(var);
    let list_opt = pool.list(opt);

    let map = subst(&[(var_id, Idx::INT)]);
    let result = substitute_in_pool(&mut pool, list_opt, &map);

    // Should be List<Option<int>>
    assert_eq!(pool.tag(result), Tag::List);
    let inner = Idx::from_raw(pool.data(result));
    assert_eq!(pool.tag(inner), Tag::Option);
    let innermost = Idx::from_raw(pool.data(inner));
    assert_eq!(innermost, Idx::INT);
}

// Interning deduplication

#[test]
fn substitution_reuses_interned_types() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    let list_var = pool.list(var);

    // Pre-create List<int>
    let list_int = pool.list(Idx::INT);

    let map = subst(&[(var_id, Idx::INT)]);
    let result = substitute_in_pool(&mut pool, list_var, &map);

    // Should return the same Idx as the pre-created List<int>.
    assert_eq!(result, list_int);
}

// No-op when no vars

#[test]
fn compound_type_without_vars_unchanged() {
    let mut pool = Pool::new();
    let fn_type = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let map_type = pool.map(Idx::STR, Idx::INT);

    let map = subst(&[(0, Idx::CHAR)]);
    // These have no HAS_VAR — all returned as-is.
    assert_eq!(substitute_in_pool(&mut pool, fn_type, &map), fn_type);
    assert_eq!(substitute_in_pool(&mut pool, tup, &map), tup);
    assert_eq!(substitute_in_pool(&mut pool, map_type, &map), map_type);
}

// Linked variable

#[test]
fn linked_var_followed() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);

    // Create a second var and link the first to it.
    let var2 = pool.fresh_var();
    let var2_id = pool.data(var2);
    *pool.var_state_mut(var_id) = crate::VarState::Link { target: var2 };

    // Substitute var2's id.
    let map = subst(&[(var2_id, Idx::BOOL)]);
    let result = substitute_in_pool(&mut pool, var, &map);
    assert_eq!(result, Idx::BOOL);
}

// Generalized variable

#[test]
fn generalized_var_substituted() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);

    // Generalize the var (simulates let-polymorphism).
    *pool.var_state_mut(var_id) = crate::VarState::Generalized(GeneralizedVarState {
        id: var_id,
        name: None,
    });

    let map = subst(&[(var_id, Idx::STR)]);
    let result = substitute_in_pool(&mut pool, var, &map);
    assert_eq!(result, Idx::STR);
}

// Struct type

#[test]
fn struct_field_types_substituted() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);

    let field_x = interner.intern("x");
    let field_y = interner.intern("y");
    let name = interner.intern("Point");
    let struct_ty = pool.struct_type(name, &[(field_x, var), (field_y, Idx::INT)]);

    let map = subst(&[(var_id, Idx::FLOAT)]);
    let result = substitute_in_pool(&mut pool, struct_ty, &map);

    assert_eq!(pool.tag(result), Tag::Struct);
    assert_eq!(pool.struct_name(result), name);
    let fields = pool.struct_fields(result);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0], (field_x, Idx::FLOAT));
    assert_eq!(fields[1], (field_y, Idx::INT));
}

#[test]
fn struct_no_vars_unchanged() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let field_a = interner.intern("a");
    let name = interner.intern("Wrapper");
    let struct_ty = pool.struct_type(name, &[(field_a, Idx::BOOL)]);

    let map = subst(&[(99, Idx::STR)]);
    // No HAS_VAR flag — fast path returns unchanged.
    assert_eq!(substitute_in_pool(&mut pool, struct_ty, &map), struct_ty);
}

// extract_var_from_types

use super::extract_var_from_types;

#[test]
fn extract_var_from_applied() {
    // Applied(Pair, [Var(A), Var(B)]) + Applied(Pair, [Int, Bool]) → A=Int, B=Bool
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let pair_name = interner.intern("Pair");

    let var_a = pool.fresh_var();
    let var_b = pool.fresh_var();
    let a_id = pool.data(var_a);
    let b_id = pool.data(var_b);

    let generic = pool.applied(pair_name, &[var_a, var_b]);
    let concrete = pool.applied(pair_name, &[Idx::INT, Idx::BOOL]);

    assert_eq!(
        extract_var_from_types(&pool, generic, concrete, a_id),
        Some(Idx::INT)
    );
    assert_eq!(
        extract_var_from_types(&pool, generic, concrete, b_id),
        Some(Idx::BOOL)
    );
}

#[test]
fn extract_var_from_nested() {
    // List<Applied(Pair, [Var(A), Int])> + List<Applied(Pair, [Bool, Int])> → A=Bool
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let pair_name = interner.intern("Pair");

    let var_a = pool.fresh_var();
    let a_id = pool.data(var_a);

    let generic_inner = pool.applied(pair_name, &[var_a, Idx::INT]);
    let generic = pool.list(generic_inner);

    let concrete_inner = pool.applied(pair_name, &[Idx::BOOL, Idx::INT]);
    let concrete = pool.list(concrete_inner);

    assert_eq!(
        extract_var_from_types(&pool, generic, concrete, a_id),
        Some(Idx::BOOL)
    );
}

#[test]
fn extract_var_from_tuple() {
    // (Var(A), Var(B)) + (Int, Str) → A=Int, B=Str
    let mut pool = Pool::new();
    let var_a = pool.fresh_var();
    let var_b = pool.fresh_var();
    let a_id = pool.data(var_a);
    let b_id = pool.data(var_b);

    let generic = pool.tuple(&[var_a, var_b]);
    let concrete = pool.tuple(&[Idx::INT, Idx::STR]);

    assert_eq!(
        extract_var_from_types(&pool, generic, concrete, a_id),
        Some(Idx::INT)
    );
    assert_eq!(
        extract_var_from_types(&pool, generic, concrete, b_id),
        Some(Idx::STR)
    );
}

#[test]
fn extract_var_not_found() {
    // Var(A) not present in type → None
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let pair_name = interner.intern("Pair");

    let concrete = pool.applied(pair_name, &[Idx::INT, Idx::BOOL]);

    // var_id 999 doesn't exist anywhere in the type
    assert_eq!(extract_var_from_types(&pool, concrete, concrete, 999), None);
}

#[test]
fn extract_var_from_function() {
    // (Var(A)) -> Var(B) + (Int) -> Str → A=Int, B=Str
    let mut pool = Pool::new();
    let var_a = pool.fresh_var();
    let var_b = pool.fresh_var();
    let a_id = pool.data(var_a);
    let b_id = pool.data(var_b);

    let generic = pool.function(&[var_a], var_b);
    let concrete = pool.function(&[Idx::INT], Idx::STR);

    assert_eq!(
        extract_var_from_types(&pool, generic, concrete, a_id),
        Some(Idx::INT)
    );
    assert_eq!(
        extract_var_from_types(&pool, generic, concrete, b_id),
        Some(Idx::STR)
    );
}

#[test]
fn extract_var_from_struct() {
    // Struct { x: Var(A), y: Int } + Struct { x: Bool, y: Int } → A=Bool
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let var_a = pool.fresh_var();
    let a_id = pool.data(var_a);

    let name = interner.intern("MyStruct");
    let fx = interner.intern("x");
    let fy = interner.intern("y");

    let generic = pool.struct_type(name, &[(fx, var_a), (fy, Idx::INT)]);
    let concrete = pool.struct_type(name, &[(fx, Idx::BOOL), (fy, Idx::INT)]);

    assert_eq!(
        extract_var_from_types(&pool, generic, concrete, a_id),
        Some(Idx::BOOL)
    );
}

#[test]
fn extract_var_direct_match() {
    // Var(A) + Int → A=Int (direct variable at top level)
    let mut pool = Pool::new();
    let var_a = pool.fresh_var();
    let a_id = pool.data(var_a);

    assert_eq!(
        extract_var_from_types(&pool, var_a, Idx::INT, a_id),
        Some(Idx::INT)
    );
}

// extend_var_subst_with_roots
//
// Regression pins for the rank-weighted union-find fix:
//  can make a fresh instantiation var the root of a
// scheme var's equivalence class. `substitute_var` finds the scheme var's
// direct entry in `var_subst` but a pool-walk that lands on the ROOT's
// `Tag::Var(root_var_id)` leaf falls through unchanged — the helper extends
// the map with `{root_var_id → concrete}` so the root is also substituted.

use super::extend_var_subst_with_roots;

#[test]
fn extend_var_subst_with_roots_via_pool_adds_root_when_different() {
    // Scheme var's equivalence-class root is a DIFFERENT fresh instantiation
    // var (simulating the 3-hop deferred-mono scenario). Helper should insert
    // {root_var_id → concrete}.
    let mut pool = Pool::new();

    // Allocate the scheme var (what the callee's signature holds).
    let scheme_var = pool.fresh_var();
    let scheme_var_id = pool.data(scheme_var);

    // Allocate a fresh instantiation var and make it the root by linking
    // the scheme var to it. Union-find: scheme_var → inst_var.
    let inst_var = pool.fresh_var();
    let inst_var_id = pool.data(inst_var);
    *pool.var_state_mut(scheme_var_id) = crate::VarState::Link { target: inst_var };

    // Seed the map with the authoritative {scheme_var_id → concrete} entry
    // (this is what the caller's var_subst build produces).
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    var_subst.insert(scheme_var_id, Idx::INT);

    // Sanity: before the helper runs, the root var_id is NOT in the map.
    assert!(!var_subst.contains_key(&inst_var_id));

    // Helper adds the root's var_id → concrete.
    extend_var_subst_with_roots(&pool, &[scheme_var_id], &mut var_subst);

    assert_eq!(
        var_subst.get(&inst_var_id),
        Some(&Idx::INT),
        "root var_id {inst_var_id} should be mapped to Idx::INT after extension"
    );
    // Original entry preserved.
    assert_eq!(var_subst.get(&scheme_var_id), Some(&Idx::INT));
    assert_eq!(var_subst.len(), 2);
}

#[test]
fn extend_var_subst_with_roots_via_pool_noop_when_root_equals_scheme_var() {
    // Scheme var IS its own root (no Link) — the helper MUST NOT add any
    // new entry. This is the monomorphic-signature / non-forwarded path.
    let mut pool = Pool::new();

    let scheme_var = pool.fresh_var();
    let scheme_var_id = pool.data(scheme_var);
    // Keep the scheme var Unbound (its default state on fresh_var): it's
    // its own root.

    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    var_subst.insert(scheme_var_id, Idx::STR);
    let before_len = var_subst.len();

    extend_var_subst_with_roots(&pool, &[scheme_var_id], &mut var_subst);

    assert_eq!(
        var_subst.len(),
        before_len,
        "helper must not add entries when scheme var IS its own root"
    );
    assert_eq!(var_subst.get(&scheme_var_id), Some(&Idx::STR));
}

#[test]
fn extend_var_subst_with_roots_via_pool_empty_scheme_vars() {
    // Monomorphic function (empty scheme_var_ids) — map unchanged.
    let pool = Pool::new();
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    // Pre-seed an unrelated entry to prove the helper doesn't stomp it.
    var_subst.insert(99, Idx::BOOL);
    let before = var_subst.clone();

    extend_var_subst_with_roots(&pool, &[], &mut var_subst);

    assert_eq!(
        var_subst, before,
        "empty scheme_var_ids must leave var_subst unchanged"
    );
}

#[test]
fn extend_var_subst_with_roots_via_pool_preserves_existing_entries() {
    // Root already keyed in the map BEFORE the helper runs. The helper's
    // preserve-existing semantics (.entry().or_insert()) MUST NOT overwrite
    // that entry — mirrors the idempotent `build_exempt_var_ids` pattern.
    let mut pool = Pool::new();

    // scheme_var linked to inst_var (union-find shape as in the "adds root"
    // test), but pre-seed var_subst with a DIFFERENT concrete for the root
    // to detect any overwrite.
    let scheme_var = pool.fresh_var();
    let scheme_var_id = pool.data(scheme_var);
    let inst_var = pool.fresh_var();
    let inst_var_id = pool.data(inst_var);
    *pool.var_state_mut(scheme_var_id) = crate::VarState::Link { target: inst_var };

    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    var_subst.insert(scheme_var_id, Idx::INT);
    // Pre-populate the root's var_id with a DIFFERENT concrete (Str). The
    // helper must NOT clobber this authoritative caller-supplied entry.
    var_subst.insert(inst_var_id, Idx::STR);

    extend_var_subst_with_roots(&pool, &[scheme_var_id], &mut var_subst);

    // Existing entry preserved — Str, not Int.
    assert_eq!(
        var_subst.get(&inst_var_id),
        Some(&Idx::STR),
        "pre-existing root-var entry must NOT be overwritten by the helper"
    );
    // Original scheme var entry also preserved.
    assert_eq!(var_subst.get(&scheme_var_id), Some(&Idx::INT));
    assert_eq!(var_subst.len(), 2);
}

// materialize_applied_body

use super::materialize_applied_body;
use ori_ir::Name;
use rustc_hash::FxHashSet;

/// Build a pool carrying a generic struct `S<A, B> = { a: A, b: B }`: interns the
/// generic body (fields `Named(A)`/`Named(B)`), records `Named(S) → generic
/// struct`, and returns `(struct_name, type_params_map, generic_struct_idx)`.
fn generic_struct_fixture(
    pool: &mut Pool,
    interner: &ori_ir::StringInterner,
) -> (Name, FxHashMap<Name, Vec<Name>>, Idx) {
    let s = interner.intern("S");
    let a = interner.intern("A");
    let b = interner.intern("B");
    let fa = interner.intern("a");
    let fb = interner.intern("b");
    let named_a = pool.named(a);
    let named_b = pool.named(b);
    let generic = pool.struct_type(s, &[(fa, named_a), (fb, named_b)]);
    let named_s = pool.named(s);
    pool.set_resolution(named_s, generic);
    let mut type_params: FxHashMap<Name, Vec<Name>> = FxHashMap::default();
    type_params.insert(s, vec![a, b]);
    (s, type_params, generic)
}

#[test]
fn finalized_body_map_registration_materializes_concrete_applied_body() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (s, type_params, generic) = generic_struct_fixture(&mut pool, &interner);
    let concrete = pool.applied(s, &[Idx::INT, Idx::STR]);

    super::register_concrete_applied_resolutions(&mut pool, &[(generic, concrete)], &type_params);

    let body = pool
        .resolve(concrete)
        .unwrap_or_else(|| panic!("finalized mono body map must register the concrete layout"));
    assert_eq!(
        pool.struct_fields(body),
        vec![
            (interner.intern("a"), Idx::INT),
            (interner.intern("b"), Idx::STR),
        ]
    );
}

// Multi-param distinct interning. `S<int, str>` and `S<str, int>` must
// materialize to DISTINCT concrete struct Idxs with distinct concrete field
// bodies (positive pin: distinct args intern distinctly).
#[test]
fn materialize_interns_distinct_bodies_for_distinct_args() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (s, type_params, _) = generic_struct_fixture(&mut pool, &interner);

    let applied_is = pool.applied(s, &[Idx::INT, Idx::STR]);
    let applied_si = pool.applied(s, &[Idx::STR, Idx::INT]);

    let mut in_progress = FxHashSet::default();
    materialize_applied_body(&mut pool, applied_is, &type_params, &mut in_progress);
    materialize_applied_body(&mut pool, applied_si, &type_params, &mut in_progress);

    let body_is = pool
        .resolve(applied_is)
        .unwrap_or_else(|| panic!("S<int,str> must materialize"));
    let body_si = pool
        .resolve(applied_si)
        .unwrap_or_else(|| panic!("S<str,int> must materialize"));
    assert_ne!(
        body_is, body_si,
        "distinct generic args must intern distinct concrete bodies"
    );
    assert_eq!(
        pool.struct_fields(body_is),
        vec![
            (interner.intern("a"), Idx::INT),
            (interner.intern("b"), Idx::STR)
        ],
    );
    assert_eq!(
        pool.struct_fields(body_si),
        vec![
            (interner.intern("a"), Idx::STR),
            (interner.intern("b"), Idx::INT)
        ],
    );
}

// Merkle interning distinctness. The materialized concrete struct Idx must
// differ from the generic `StructDef` Idx (distinct field hashes → distinct
// `Idx` per TI-1/TI-3), so `S<int,str>` interns as a SEPARATE pool entry.
#[test]
fn materialized_concrete_struct_distinct_from_generic_definition() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (s, type_params, generic) = generic_struct_fixture(&mut pool, &interner);

    let applied = pool.applied(s, &[Idx::INT, Idx::STR]);
    let mut in_progress = FxHashSet::default();
    materialize_applied_body(&mut pool, applied, &type_params, &mut in_progress);

    let concrete = pool
        .resolve(applied)
        .unwrap_or_else(|| panic!("must materialize"));
    assert_ne!(
        concrete, generic,
        "the materialized concrete struct must be a distinct pool entry from the generic definition"
    );
    assert_ne!(
        pool.hash(concrete),
        pool.hash(generic),
        "concrete + generic struct bodies must have distinct Merkle hashes"
    );
}

/// Regression: a generic shell such as `S<A, str>` is not a concrete
/// instantiation merely because `Named(A)` has no cached variable flag.
#[test]
fn materialize_unresolved_named_argument_does_not_register_generic_body() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (s, type_params, generic) = generic_struct_fixture(&mut pool, &interner);
    let unresolved_a = pool.named(interner.intern("A"));
    let applied = pool.applied(s, &[unresolved_a, Idx::STR]);

    let mut in_progress = FxHashSet::default();
    materialize_applied_body(&mut pool, applied, &type_params, &mut in_progress);

    assert_eq!(
        pool.resolve(applied),
        None,
        "an unresolved named binder must not register the generic body as concrete"
    );
    assert_eq!(
        pool.struct_fields(generic)[0].1,
        unresolved_a,
        "the generic definition remains the sole body carrying the binder"
    );
}

/// A type parameter may shadow an outer nominal type with the same name. The
/// generic application must remain unresolved even though the shared `Named`
/// pool handle carries the outer nominal type's resolution.
#[test]
fn materialize_shadowing_named_binder_keeps_application_unresolved() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (s, type_params, _) = generic_struct_fixture(&mut pool, &interner);
    let a = interner.intern("A");
    let value = interner.intern("value");
    let shadowing_binder = pool.named(a);
    let generic_shell = pool.applied(s, &[shadowing_binder, Idx::STR]);
    let outer_nominal_body = pool.struct_type(a, &[(value, Idx::INT)]);
    pool.set_resolution(shadowing_binder, outer_nominal_body);
    let nominal_a = pool.named(a);
    let concrete_same_spelling = pool.applied(s, &[nominal_a, Idx::STR]);

    assert_eq!(
        generic_shell, concrete_same_spelling,
        "the pool currently erases the lexical origin that distinguishes the generic shell from S<global A>"
    );

    let mut in_progress = FxHashSet::default();
    materialize_applied_body(&mut pool, generic_shell, &type_params, &mut in_progress);

    assert_eq!(
        pool.resolve(shadowing_binder),
        Some(outer_nominal_body),
        "the outer nominal type must keep its concrete resolution"
    );
    assert_eq!(
        pool.resolve(generic_shell),
        None,
        "the shadowing binder must not turn the generic application concrete"
    );
}

/// Regression: a named binder nested under another application remains
/// generic; neither the inner nor outer application may gain a resolution.
#[test]
fn materialize_nested_unresolved_named_argument_registers_no_resolution() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (s, type_params, _) = generic_struct_fixture(&mut pool, &interner);
    let wrapper = interner.intern("Wrapper");
    let unresolved_a = pool.named(interner.intern("A"));
    let inner = pool.applied(wrapper, &[unresolved_a]);
    let outer = pool.applied(s, &[inner, Idx::STR]);

    let mut in_progress = FxHashSet::default();
    materialize_applied_body(&mut pool, outer, &type_params, &mut in_progress);

    assert_eq!(
        pool.resolve(outer),
        None,
        "an outer application containing a generic inner argument must remain unresolved"
    );
    assert_eq!(
        pool.resolve(inner),
        None,
        "the bottom-up walk must not materialize a generic inner application"
    );
}

/// A resolved nominal argument is concrete and must not be rejected merely
/// because its surface node is `Named` or an unrelated generic declaration
/// happens to reuse the nominal's spelling as one of its binders.
#[test]
fn materialize_resolved_named_argument_registers_concrete_body() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (s, mut type_params, _) = generic_struct_fixture(&mut pool, &interner);
    let user = interner.intern("User");
    let unrelated = interner.intern("Unrelated");
    let value = interner.intern("value");
    type_params.insert(unrelated, vec![user]);
    let named_user = pool.named(user);
    let user_body = pool.struct_type(user, &[(value, Idx::INT)]);
    pool.set_resolution(named_user, user_body);
    let applied = pool.applied(s, &[named_user, Idx::STR]);

    let mut in_progress = FxHashSet::default();
    materialize_applied_body(&mut pool, applied, &type_params, &mut in_progress);

    let concrete = pool
        .resolve(applied)
        .unwrap_or_else(|| panic!("a resolved nominal argument must materialize"));
    assert_eq!(
        pool.resolve_fully(pool.struct_fields(concrete)[0].1),
        user_body,
        "the materialized field must retain the concrete nominal argument"
    );
}

// Self-referential composite termination. `List<T> = Nil | Cons(T,
// List<T>)` materialized at `List<int>` must TERMINATE (no stack overflow /
// infinite intern) under the in-progress recursion guard, and the recursive
// `tail` payload must resolve to the in-progress concrete enum.
#[test]
fn materialize_self_referential_enum_terminates() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let list = interner.intern("List");
    let t = interner.intern("T");
    let nil = interner.intern("Nil");
    let cons = interner.intern("Cons");

    // Generic body: Nil (unit) | Cons(Named(T), Applied(List, [Named(T)]))
    let named_t = pool.named(t);
    let tail_generic = pool.applied(list, &[named_t]);
    let generic = pool.enum_type(
        list,
        &[
            crate::pool::EnumVariant {
                name: nil,
                field_types: vec![],
            },
            crate::pool::EnumVariant {
                name: cons,
                field_types: vec![named_t, tail_generic],
            },
        ],
    );
    let named_list = pool.named(list);
    pool.set_resolution(named_list, generic);
    let mut type_params: FxHashMap<Name, Vec<Name>> = FxHashMap::default();
    type_params.insert(list, vec![t]);

    let applied = pool.applied(list, &[Idx::INT]);
    let mut in_progress = FxHashSet::default();
    // The assertion is reaching this line at all — non-termination would stack
    // overflow inside the call.
    materialize_applied_body(&mut pool, applied, &type_params, &mut in_progress);

    let concrete = pool
        .resolve(applied)
        .unwrap_or_else(|| panic!("List<int> must materialize"));
    assert_eq!(pool.tag(concrete), Tag::Enum);
    let variants = pool.enum_variants(concrete);
    let (_, cons_payloads) = &variants[1];
    assert_eq!(cons_payloads[0], Idx::INT, "Cons head must be concrete int");
    // The recursive tail `Applied(List,[int])` must resolve to the SAME concrete
    // enum (the in-progress entry), proving termination via the guard.
    let tail = cons_payloads[1];
    assert_eq!(pool.tag(tail), Tag::Applied);
    assert_eq!(
        pool.resolve(tail),
        Some(concrete),
        "the recursive tail must resolve to the materialized concrete enum"
    );
    // The in-progress guard must be empty after completion (no leaked entries).
    assert!(
        in_progress.is_empty(),
        "in_progress guard must drain on completion"
    );
}

#[test]
fn materialize_rebuilds_stale_applied_resolution_from_generic_declaration() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let holder = interner.intern("Holder");
    let parameter = interner.intern("T");
    let empty = interner.intern("Empty");
    let full = interner.intern("Full");
    let named_parameter = pool.named(parameter);
    let generic = pool.enum_type(
        holder,
        &[
            crate::pool::EnumVariant {
                name: empty,
                field_types: Vec::new(),
            },
            crate::pool::EnumVariant {
                name: full,
                field_types: vec![named_parameter],
            },
        ],
    );
    let named_holder = pool.named(holder);
    pool.set_resolution(named_holder, generic);
    let mut type_params = FxHashMap::default();
    type_params.insert(holder, vec![parameter]);

    let holder_string = pool.applied(holder, &[Idx::STR]);
    let stale_outer = pool.enum_type(
        holder,
        &[
            crate::pool::EnumVariant {
                name: empty,
                field_types: Vec::new(),
            },
            crate::pool::EnumVariant {
                name: full,
                field_types: vec![holder_string],
            },
        ],
    );
    pool.set_resolution(holder_string, stale_outer);

    let mut in_progress = FxHashSet::default();
    materialize_applied_body(&mut pool, holder_string, &type_params, &mut in_progress);

    let concrete = pool
        .resolve(holder_string)
        .unwrap_or_else(|| panic!("Holder<str> must retain a concrete body"));
    assert_eq!(
        pool.enum_variants(concrete)[1].1,
        vec![Idx::STR],
        "re-materialization must not reuse a stale nested specialization as the generic template"
    );
}

// build_finalized_body_type_map
//
// The build->(extend named)->finalize bookend has one home shared by the three
// Vec-sink sites (build_mono_instance, refresh_method_mono_body_type_maps,
// build_and_register_body_type_map). These pins fix the helper's contract: build
// into a fresh Vec, extend with the caller's already-resolved (key, val) pairs,
// finalize (sort+dedup), return.

use super::build_finalized_body_type_map;

// Two-fresh-pool setup: build_mono_body_type_map interns BoundVars that then
// carry HAS_BOUND_VAR, so a second call on the SAME pool re-scans them — it is
// NOT idempotent on one pool. fresh_var allocates var_ids sequentially, so two
// fresh pools built identically produce identical maps.
fn two_var_subst(pool: &mut Pool) -> FxHashMap<u32, Idx> {
    let v1 = pool.fresh_var();
    let id1 = pool.data(v1);
    let v2 = pool.fresh_var();
    let id2 = pool.data(v2);
    let mut vs: FxHashMap<u32, Idx> = FxHashMap::default();
    vs.insert(id1, Idx::INT);
    vs.insert(id2, Idx::STR);
    vs
}

#[test]
fn build_finalized_empty_extra_equals_manual_build_then_finalize() {
    // SSOT-collapse pin: the helper with no extra_named is EXACTLY the
    // build_mono_body_type_map + finalize_body_type_map bookend the three sites
    // duplicated. Build the manual baseline and the helper on two fresh pools.
    let mut pool_manual = Pool::new();
    let vs_manual = two_var_subst(&mut pool_manual);
    let mut manual: Vec<(Idx, Idx)> = Vec::new();
    super::body_type_map::build_mono_body_type_map(&mut pool_manual, &vs_manual, &mut manual);
    super::body_type_map::finalize_body_type_map(&mut manual);

    let mut pool_helper = Pool::new();
    let vs_helper = two_var_subst(&mut pool_helper);
    let helper = build_finalized_body_type_map(&mut pool_helper, &vs_helper, &[]);

    assert_eq!(
        helper, manual,
        "helper with empty extra_named must equal the manual build+finalize bookend"
    );
    assert!(
        !helper.is_empty(),
        "two-var subst must produce a non-empty map"
    );
}

#[test]
fn build_finalized_sorts_extra_named_by_key() {
    // Extra_named entries are passed through finalize (sort by key.raw()), NOT
    // appended raw. Empty var_subst isolates the extra_named path.
    let mut pool = Pool::new();
    let empty: FxHashMap<u32, Idx> = FxHashMap::default();
    // Two distinct keys, deliberately out of raw() order.
    let mut keys = [Idx::BOOL, Idx::INT, Idx::STR];
    keys.sort_by_key(|k| k.raw());
    let extra = [(keys[2], Idx::INT), (keys[0], Idx::STR)];

    let result = build_finalized_body_type_map(&mut pool, &empty, &extra);

    assert_eq!(result.len(), 2, "two distinct extra keys survive finalize");
    assert!(
        result.windows(2).all(|w| w[0].0.raw() < w[1].0.raw()),
        "finalize must sort extra_named by key.raw() ascending"
    );
}

#[test]
fn build_finalized_dedups_duplicate_extra_named_key() {
    // Clamp / negative pin: a duplicate-key extra entry is deduped by finalize.
    // This ONLY holds if extend happens BEFORE finalize — appending extra AFTER
    // finalize would leave the duplicate, yielding len 2. Pins the ordering.
    let mut pool = Pool::new();
    let empty: FxHashMap<u32, Idx> = FxHashMap::default();
    let extra = [(Idx::INT, Idx::STR), (Idx::INT, Idx::BOOL)];

    let result = build_finalized_body_type_map(&mut pool, &empty, &extra);

    assert_eq!(
        result.len(),
        1,
        "duplicate extra_named key must be deduped by finalize (extend-before-finalize)"
    );
    assert_eq!(result[0].0, Idx::INT, "the surviving entry keys on the dup");
}

// (negative / no-register clamp) — the bookend helper is PURE build+extend+finalize:
// it NEVER registers Applied->concrete resolutions. register_concrete_applied_resolutions
// stays site-local; build_mono_instance must NOT register. Pins the no-register
// invariant the other pins miss.
#[test]
fn build_finalized_does_not_register_applied_resolutions() {
    let mut pool = Pool::new();
    // An Applied(List, [int]) the caller has NOT registered to any concrete enum.
    let interner = ori_ir::StringInterner::new();
    let list = interner.intern("List");
    let applied = pool.applied(list, &[Idx::INT]);
    assert!(
        pool.resolve(applied).is_none(),
        "precondition: the Applied type starts unregistered"
    );
    // Drive the bookend over a non-empty var_subst (so it does real build work).
    let v1 = pool.fresh_var();
    let id1 = pool.data(v1);
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    var_subst.insert(id1, Idx::INT);

    let _map = build_finalized_body_type_map(&mut pool, &var_subst, &[]);

    // The helper builds the body-type-map ONLY; it MUST NOT have registered the
    // Applied type to a concrete resolution (that is register_concrete_applied_resolutions'
    // job, kept site-local at the two registering call sites — NEVER in the bookend).
    assert!(
        pool.resolve(applied).is_none(),
        "build_finalized_body_type_map must NOT register Applied->concrete resolutions"
    );
}
