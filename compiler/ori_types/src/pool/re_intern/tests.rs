//! Tests for cross-pool type re-interning.

use rustc_hash::FxHashMap;

use crate::pool::re_intern::{
    re_intern_sig, re_intern_sig_with_var_remap, re_intern_type, re_intern_type_with_var_remap,
};
use crate::{FunctionSig, Idx, Pool, Tag, VarState};

// `re_intern_type_with_var_remap` and `re_intern_sig_with_var_remap` in
// `pool/re_intern/mod.rs` are the remap-aware re-intern entry points; the
// positive pins below pin their behavior, and the legacy-path negative pins
// document the var-id collision the remap exists to prevent.

// === Primitive Types ===

#[test]
fn primitives_are_identity() {
    let source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // All primitives should map to themselves (fixed indices)
    for idx in [
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::STR,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
        Idx::NEVER,
        Idx::DURATION,
        Idx::SIZE,
        Idx::ORDERING,
    ] {
        let result = re_intern_type(&source, idx, &mut target, &mut cache);
        assert_eq!(result, idx, "Primitive {idx:?} should map to itself");
    }
}

// === Simple Container Types ===

#[test]
fn list_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let list_int = source.list(Idx::INT);
    let result = re_intern_type(&source, list_int, &mut target, &mut cache);

    // Verify structural equality via Merkle hash
    assert_eq!(target.hash(result), source.hash(list_int));
    assert_eq!(target.tag(result), crate::Tag::List);
}

#[test]
fn option_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let opt_str = source.option(Idx::STR);
    let result = re_intern_type(&source, opt_str, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(opt_str));
}

// === Two-Child Container Types ===

#[test]
fn map_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let map_str_int = source.map(Idx::STR, Idx::INT);
    let result = re_intern_type(&source, map_str_int, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(map_str_int));
    assert_eq!(target.map_key(result), Idx::STR);
    assert_eq!(target.map_value(result), Idx::INT);
}

#[test]
fn result_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let res = source.result(Idx::INT, Idx::STR);
    let result = re_intern_type(&source, res, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(res));
    assert_eq!(target.result_ok(result), Idx::INT);
    assert_eq!(target.result_err(result), Idx::STR);
}

// === Complex Types ===

#[test]
fn function_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let func = source.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    let result = re_intern_type(&source, func, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(func));
    assert_eq!(target.function_params(result), vec![Idx::INT, Idx::STR]);
    assert_eq!(target.function_return(result), Idx::BOOL);
}

#[test]
fn tuple_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let tup = source.tuple(&[Idx::INT, Idx::BOOL, Idx::STR]);
    let result = re_intern_type(&source, tup, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(tup));
    assert_eq!(
        target.tuple_elems(result),
        vec![Idx::INT, Idx::BOOL, Idx::STR]
    );
}

// === Nested Types ===

#[test]
fn nested_list_of_tuples_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // List<(int, str)>
    let tup = source.tuple(&[Idx::INT, Idx::STR]);
    let list_tup = source.list(tup);
    let result = re_intern_type(&source, list_tup, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(list_tup));
}

#[test]
fn deeply_nested_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // Option<Map<str, List<int>>>
    let list_int = source.list(Idx::INT);
    let map_type = source.map(Idx::STR, list_int);
    let opt = source.option(map_type);
    let result = re_intern_type(&source, opt, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(opt));
}

// === Cache Behavior ===

#[test]
fn cache_prevents_redundant_interning() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let list_int = source.list(Idx::INT);

    // Re-intern twice with the same cache
    let result1 = re_intern_type(&source, list_int, &mut target, &mut cache);
    let result2 = re_intern_type(&source, list_int, &mut target, &mut cache);

    // Should return the same Idx (cache hit)
    assert_eq!(result1, result2);

    // Cache should contain the mapping
    assert_eq!(cache.get(&list_int), Some(&result1));
}

// === Cross-Pool Stability ===

#[test]
fn re_interned_types_have_stable_hashes() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // Build a complex type in source
    let func = source.function(&[Idx::INT], Idx::STR);
    let list_func = source.list(func);

    // Also build the same type directly in target
    let target_func = target.function(&[Idx::INT], Idx::STR);
    let target_list_func = target.list(target_func);

    // Re-intern from source into target
    let result = re_intern_type(&source, list_func, &mut target, &mut cache);

    // Should resolve to the same Idx (deduplication via Merkle hash)
    assert_eq!(result, target_list_func);
    assert_eq!(target.hash(result), target.hash(target_list_func));
}

#[test]
fn fast_path_hits_for_existing_types() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // Pre-populate target with some types to shift its next Idx
    let _ = target.list(Idx::FLOAT);
    let _ = target.option(Idx::BOOL);
    let target_list = target.list(Idx::INT);

    // Source has the same type at a different Idx (no padding types)
    let source_list = source.list(Idx::INT);
    assert_ne!(
        source_list, target_list,
        "Source and target should have different Idx for List<int>"
    );

    // Re-interning should find the existing type via hash
    let result = re_intern_type(&source, source_list, &mut target, &mut cache);
    assert_eq!(result, target_list, "Should reuse existing Idx via hash");
}

// === FunctionSig Re-interning ===

#[test]
fn sig_primitive_params_re_interned() {
    let source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let mut sig = FunctionSig::simple(
        ori_ir::Name::from_raw(1),
        vec![Idx::INT, Idx::STR],
        Idx::BOOL,
    );
    sig.populate_hashes(&source);

    let result = re_intern_sig(&sig, &source, &mut target, &mut cache);

    assert_eq!(result.param_types, vec![Idx::INT, Idx::STR]);
    assert_eq!(result.return_type, Idx::BOOL);
    // Hashes should be valid in target pool
    assert_eq!(result.param_hashes[0], target.hash(Idx::INT));
    assert_eq!(result.param_hashes[1], target.hash(Idx::STR));
    assert_eq!(result.return_hash, target.hash(Idx::BOOL));
}

#[test]
fn sig_compound_params_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let list_int = source.list(Idx::INT);
    let mut sig = FunctionSig::simple(ori_ir::Name::from_raw(1), vec![list_int], Idx::UNIT);
    sig.populate_hashes(&source);

    let result = re_intern_sig(&sig, &source, &mut target, &mut cache);

    // Param type should be re-interned (valid in target)
    let target_list_int = target.list(Idx::INT);
    assert_eq!(result.param_types[0], target_list_int);
    assert_eq!(result.param_hashes[0], target.hash(target_list_int));
}

// === Struct/Enum Types ===

#[test]
fn struct_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let interner = ori_ir::StringInterner::new();
    let point_name = interner.intern("Point");
    let x_name = interner.intern("x");
    let y_name = interner.intern("y");

    let point = source.struct_type(point_name, &[(x_name, Idx::INT), (y_name, Idx::INT)]);
    let result = re_intern_type(&source, point, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(point));
    assert_eq!(target.struct_name(result), point_name);

    let fields = target.struct_fields(result);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0], (x_name, Idx::INT));
    assert_eq!(fields[1], (y_name, Idx::INT));
}

#[test]
fn enum_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let interner = ori_ir::StringInterner::new();
    let shape_name = interner.intern("Shape");
    let circle_name = interner.intern("Circle");
    let rect_name = interner.intern("Rect");

    let shape = source.enum_type(
        shape_name,
        &[
            crate::pool::construct::EnumVariant {
                name: circle_name,
                field_types: vec![Idx::FLOAT],
            },
            crate::pool::construct::EnumVariant {
                name: rect_name,
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );
    let result = re_intern_type(&source, shape, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(shape));
    assert_eq!(target.enum_name(result), shape_name);

    let variants = target.enum_variants(result);
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].0, circle_name);
    assert_eq!(variants[0].1, vec![Idx::FLOAT]);
    assert_eq!(variants[1].0, rect_name);
    assert_eq!(variants[1].1, vec![Idx::FLOAT, Idx::FLOAT]);
}

// === Named/Applied Types ===

#[test]
fn applied_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let interner = ori_ir::StringInterner::new();
    let name = interner.intern("Container");

    let applied = source.applied(name, &[Idx::INT, Idx::STR]);
    let result = re_intern_type(&source, applied, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(applied));
    assert_eq!(target.applied_name(result), name);
    assert_eq!(target.applied_args(result), vec![Idx::INT, Idx::STR]);
}

// === Scheme Types ===

#[test]
fn scheme_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let body = source.function(&[Idx::INT], Idx::STR);
    let scheme = source.scheme(&[0, 1], body);
    let result = re_intern_type(&source, scheme, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(scheme));
}

// Matrix extension (e1–e5) — cross-module pool-merge var_id remap
//
// Pins the cross-module pool-merge var_id collision.
//
// - Negative pins (no `_with_var_remap` suffix): assert the collision is
//   present on the legacy `re_intern_type` / `re_intern_sig` path — regression
//   pins that document WHY the remap-aware variant is required. They
//   remain GREEN because the legacy path is preserved.
// - Positive pins (`_with_var_remap` suffix): pin the required behavior
//   of the remap-aware path.

// --- (e1) Leaf var remap across pools — Tag::Var / BoundVar / RigidVar ------

/// Helper: construct a target pool whose `var_state(var_id)` is
/// `Generalized { id: var_id, name: None }` — the "host-file poly-lambda
/// residue" shape from the cross-module pool-merge collision.
fn target_with_generalized_slot(var_id: u32) -> Pool {
    let mut target = Pool::new();
    target.ensure_var_capacity(var_id + 1);
    *target.var_state_mut(var_id) = VarState::Generalized {
        id: var_id,
        name: None,
    };
    target
}

/// Regression guard — backward-compat `re_intern_type` delegates to the
/// remap-aware entry point.
///
/// The host-file poly-lambda residue at `target.var_states[0]` (a
/// `VarState::Generalized` slot the imported source module never touched)
/// MUST NOT be aliased by the imported `Tag::Var(0)` leaf. The backward-compat
/// API must allocate a fresh `dst_id` via `target.allocate_var_id`, leaving
/// slot 0 untouched.
#[test]
fn re_intern_type_var_leaf_via_legacy_api_allocates_fresh_dst_id_and_does_not_alias_host_slot() {
    let mut source = Pool::new();
    let source_var = source.intern(Tag::Var, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();

    let result = re_intern_type(&source, source_var, &mut target, &mut cache);

    assert_eq!(target.tag(result), Tag::Var);
    let dst_id = target.data(result);
    assert_ne!(
        dst_id, 0,
        "legacy `re_intern_type` must delegate to remap-aware path — \
         fresh dst_id must not alias host slot 0"
    );
    assert!(
        matches!(target.var_state(0), VarState::Generalized { .. }),
        "host-owned slot 0 must be untouched by the imported leaf"
    );
}

/// Regression guard — backward-compat `re_intern_type` delegation for `Tag::BoundVar`.
#[test]
fn re_intern_type_bound_var_leaf_via_legacy_api_allocates_fresh_dst_id() {
    let mut source = Pool::new();
    let source_var = source.intern(Tag::BoundVar, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();

    let result = re_intern_type(&source, source_var, &mut target, &mut cache);

    assert_eq!(target.tag(result), Tag::BoundVar);
    assert_ne!(
        target.data(result),
        0,
        "fresh dst_id must not alias host slot 0"
    );
    assert!(matches!(target.var_state(0), VarState::Generalized { .. }));
}

/// Regression guard — backward-compat `re_intern_type` delegation for `Tag::RigidVar`.
#[test]
fn re_intern_type_rigid_var_leaf_via_legacy_api_allocates_fresh_dst_id() {
    let mut source = Pool::new();
    let source_var = source.intern(Tag::RigidVar, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();

    let result = re_intern_type(&source, source_var, &mut target, &mut cache);

    assert_eq!(target.tag(result), Tag::RigidVar);
    assert_ne!(
        target.data(result),
        0,
        "fresh dst_id must not alias host slot 0"
    );
    assert!(matches!(target.var_state(0), VarState::Generalized { .. }));
}

/// Matrix cell (e1, positive pin) — `Tag::Var` under remap-aware re-intern.
///
/// Re-interning a source `Tag::Var(0)` into a target
/// holding an unrelated `Generalized(0)` slot MUST: (a) allocate a fresh
/// `dst_id` via `target.next_var_id`, (b) record `0 → dst_id` in `var_remap`,
/// (c) rebuild `target.var_state(dst_id)` as `Unbound { id: dst_id, .. }`
/// (pool-local id — NOT source's `0`), preserving `rank` and `name` verbatim
/// from the source's shipped `VarState::Unbound` variant.
#[test]
fn remap_aware_re_intern_var_leaf_allocates_fresh_dst_id_and_rebuilds_var_state() {
    let mut source = Pool::new();
    source.fresh_var(); // source.var_states[0] = Unbound { id: 0, .. }
    let source_var = source.intern(Tag::Var, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, source_var, &mut target, &mut cache, &mut var_remap);

    assert_eq!(target.tag(result), Tag::Var);
    let dst_id = target.data(result);
    assert_ne!(dst_id, 0, "fresh dst_id must not collide with host slot 0");
    assert_eq!(var_remap.get(&0), Some(&dst_id));

    match target.var_state(dst_id) {
        VarState::Unbound { id, .. } => {
            assert_eq!(*id, dst_id, "VarState.id must be pool-local dst_id");
        }
        other => panic!("expected variant-aware Unbound rebuild, got {other:?}"),
    }
    assert!(
        matches!(target.var_state(0), VarState::Generalized { .. }),
        "host-owned slot 0 must be untouched"
    );
}

/// Matrix cell (e1, positive pin) — `Tag::BoundVar`.
#[test]
fn remap_aware_re_intern_bound_var_leaf_allocates_fresh_dst_id() {
    let mut source = Pool::new();
    source.fresh_var();
    let source_var = source.intern(Tag::BoundVar, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, source_var, &mut target, &mut cache, &mut var_remap);

    assert_eq!(target.tag(result), Tag::BoundVar);
    let dst_id = target.data(result);
    assert_ne!(dst_id, 0);
    assert_eq!(var_remap.get(&0), Some(&dst_id));
}

/// Matrix cell (e1, positive pin) — `Tag::RigidVar`.
#[test]
fn remap_aware_re_intern_rigid_var_leaf_allocates_fresh_dst_id_and_preserves_rigid_name() {
    let mut source = Pool::new();
    let rigid_name = ori_ir::Name::from_raw(42);
    source.ensure_var_capacity(1);
    *source.var_state_mut(0) = VarState::Rigid { name: rigid_name };
    let source_var = source.intern(Tag::RigidVar, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, source_var, &mut target, &mut cache, &mut var_remap);

    assert_eq!(target.tag(result), Tag::RigidVar);
    let dst_id = target.data(result);
    assert_ne!(dst_id, 0);
    assert_eq!(var_remap.get(&0), Some(&dst_id));
    match target.var_state(dst_id) {
        VarState::Rigid { name } => {
            assert_eq!(
                *name, rigid_name,
                "Rigid.name is a global Name intern — clone verbatim"
            );
        }
        other => panic!("expected variant-aware Rigid rebuild, got {other:?}"),
    }
}

// --- (e2) Scheme binder list remaps together with body leaves ---------------

/// Regression guard — backward-compat `re_intern_type` on `Tag::Scheme`
/// delegates to the remap-aware path.
///
/// Binders `[7, 9]` and body-leaf `Tag::Var(7)` must remap coherently through
/// a single `var_remap` map; the binder list in the target MUST NOT preserve
/// the source ids verbatim (which was the pre-fix collision symptom).
#[test]
fn re_intern_type_scheme_via_legacy_api_remaps_binders_and_body_coherently() {
    let mut source = Pool::new();
    // Scheme binders reference var_ids [7, 9]; body references Tag::Var(7).
    let body_leaf = source.intern(Tag::Var, 7);
    let scheme = source.scheme(&[7, 9], body_leaf);

    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let result = re_intern_type(&source, scheme, &mut target, &mut cache);

    assert_eq!(target.tag(result), Tag::Scheme);
    let dst_binders = target.scheme_vars(result).to_vec();
    assert_ne!(
        dst_binders,
        vec![7, 9],
        "legacy `re_intern_type` must delegate to remap-aware path — \
         source binder ids must not leak through"
    );
    // Body leaf's var_id must match the remapped binder (internal consistency).
    let dst_body = target.scheme_body(result);
    assert_eq!(
        target.data(dst_body),
        dst_binders[0],
        "body leaf Tag::Var uses the SAME remapped id as the first binder"
    );
}

/// Matrix cell (e2, positive pin).
///
/// Re-interning `Scheme([7, 9], body{Tag::Var(7), ...})`
/// across pools MUST yield a scheme whose binder list is `[remap[7], remap[9]]`
/// AND whose body-leaf `Tag::Var` uses the SAME `remap[7]`. A remap that
/// touches body leaves but clones binders (or vice versa) produces an
/// internally-inconsistent scheme.
#[test]
fn remap_aware_re_intern_scheme_remaps_binders_and_body_leaves_coherently() {
    let mut source = Pool::new();
    let body_leaf_7 = source.intern(Tag::Var, 7);
    let body_fn = source.function(&[body_leaf_7], body_leaf_7);
    let scheme = source.scheme(&[7, 9], body_fn);

    let mut target = Pool::new();
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, scheme, &mut target, &mut cache, &mut var_remap);

    assert_eq!(target.tag(result), Tag::Scheme);
    let dst_binders = target.scheme_vars(result).to_vec();
    assert_eq!(dst_binders.len(), 2);
    assert_eq!(
        var_remap.get(&7).copied(),
        Some(dst_binders[0]),
        "binder[0] must use remap[7]"
    );
    assert_eq!(
        var_remap.get(&9).copied(),
        Some(dst_binders[1]),
        "binder[1] must use remap[9]"
    );

    // Body's Tag::Var leaves must carry the SAME remapped id as the binder —
    // internal consistency: a scheme whose body references a var_id not in
    // its binder list is malformed.
    let dst_body = target.scheme_body(result);
    let dst_params = target.function_params(dst_body);
    assert_eq!(target.data(dst_params[0]), dst_binders[0]);
    assert_eq!(
        target.data(target.function_return(dst_body)),
        dst_binders[0]
    );
}

// --- (e3) FunctionSig.scheme_var_ids coherence with remapped type tree ------

/// Regression guard — backward-compat `re_intern_sig` delegates to the
/// remap-aware path.
///
/// Both `scheme_var_ids` and the leaf `Tag::Var` ids in `param_types` /
/// `return_type` must remap through a single shared `var_remap`, so the
/// monomorphizer's `var_subst = HashMap::from([(scheme_var_ids[i], concrete_arg[i])])`
/// at call sites resolves every leaf `Tag::Var` in the remapped type tree.
#[test]
fn re_intern_sig_via_legacy_api_remaps_scheme_var_ids_coherently_with_leaves() {
    let mut source = Pool::new();
    let leaf_7 = source.intern(Tag::Var, 7);
    let mut sig = FunctionSig::simple(ori_ir::Name::from_raw(1), vec![leaf_7], leaf_7);
    sig.scheme_var_ids = vec![7];
    sig.populate_hashes(&source);

    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let result = re_intern_sig(&sig, &source, &mut target, &mut cache);

    assert_ne!(
        result.scheme_var_ids,
        vec![7],
        "legacy `re_intern_sig` must delegate to remap-aware path — \
         source scheme_var_ids must not leak through"
    );
    // Internal coherence: the remapped leaf in param_types uses the SAME
    // dst_id as scheme_var_ids[0].
    assert_eq!(
        target.data(result.param_types[0]),
        result.scheme_var_ids[0],
        "remapped leaf in param_types must match remapped scheme_var_ids[0]"
    );
    assert_eq!(
        target.data(result.return_type),
        result.scheme_var_ids[0],
        "remapped leaf in return_type must match remapped scheme_var_ids[0]"
    );
}

/// Matrix cell (e3, positive pin).
///
/// `re_intern_sig_with_var_remap` MUST rewrite
/// `scheme_var_ids` via the same `var_remap` that rewrites leaf `Tag::Var`
/// ids, so the monomorphizer's `var_subst = HashMap::from([(scheme_var_ids[0],
/// concrete)])` resolves every leaf in `param_types` / `return_type`.
#[test]
fn remap_aware_re_intern_sig_remaps_scheme_var_ids_coherently_with_leaves() {
    let mut source = Pool::new();
    let leaf_7 = source.intern(Tag::Var, 7);
    let mut sig = FunctionSig::simple(ori_ir::Name::from_raw(1), vec![leaf_7], leaf_7);
    sig.scheme_var_ids = vec![7];
    sig.populate_hashes(&source);

    let mut target = Pool::new();
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_sig_with_var_remap(&sig, &source, &mut target, &mut cache, &mut var_remap);

    assert_eq!(result.scheme_var_ids.len(), 1);
    let dst_id = result.scheme_var_ids[0];
    assert_eq!(
        var_remap.get(&7).copied(),
        Some(dst_id),
        "scheme_var_ids must use var_remap"
    );
    assert_eq!(
        target.data(result.param_types[0]),
        dst_id,
        "leaves in param_types must match remapped scheme_var_ids"
    );
    assert_eq!(
        target.data(result.return_type),
        dst_id,
        "leaf in return_type must match remapped scheme_var_ids"
    );
}

// --- (e4) VarState variant-aware rebuild ------------------------------------

/// Matrix cell (e4, positive pin) — `Unbound` variant.
///
/// Rebuild preserves `rank` and `name` verbatim; `id` is
/// the fresh `dst_id` (NOT source's `id`). A literal-byte clone that kept
/// source's `id` would reintroduce the aliasing the remap exists to eliminate.
#[test]
fn remap_aware_re_intern_rebuilds_unbound_with_fresh_id_and_preserved_rank_name() {
    let mut source = Pool::new();
    let src_name = ori_ir::Name::from_raw(7);
    source.fresh_named_var(src_name); // Unbound { id: 0, rank: DEFAULT, name: Some(src_name) }
    let source_var = source.intern(Tag::Var, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, source_var, &mut target, &mut cache, &mut var_remap);

    let dst_id = target.data(result);
    assert_ne!(dst_id, 0);
    match target.var_state(dst_id) {
        VarState::Unbound { id, name, .. } => {
            assert_eq!(*id, dst_id, "pool-local id must be dst_id, NOT source's 0");
            assert_eq!(*name, Some(src_name), "name clones verbatim");
        }
        other => panic!("expected Unbound rebuild, got {other:?}"),
    }
}

/// Matrix cell (e4, positive pin) — `Generalized` variant.
#[test]
fn remap_aware_re_intern_rebuilds_generalized_with_fresh_id_and_preserved_name() {
    let mut source = Pool::new();
    source.ensure_var_capacity(1);
    let src_name = ori_ir::Name::from_raw(11);
    *source.var_state_mut(0) = VarState::Generalized {
        id: 0,
        name: Some(src_name),
    };
    let source_var = source.intern(Tag::Var, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, source_var, &mut target, &mut cache, &mut var_remap);

    let dst_id = target.data(result);
    assert_ne!(dst_id, 0);
    match target.var_state(dst_id) {
        VarState::Generalized { id, name } => {
            assert_eq!(*id, dst_id, "pool-local id must be dst_id");
            assert_eq!(*name, Some(src_name));
        }
        other => panic!(
            "expected Generalized rebuild (NOT Unbound — blank-init would flip the \
             substitute_in_pool branch), got {other:?}"
        ),
    }
}

/// Matrix cell (e4, positive pin) — `Rigid` variant.
///
/// `Rigid.name` is a global `Name` intern (pool-independent); there is no `id`
/// field on `Rigid`. Clone verbatim.
#[test]
fn remap_aware_re_intern_rebuilds_rigid_with_preserved_name() {
    let mut source = Pool::new();
    let rigid_name = ori_ir::Name::from_raw(13);
    source.ensure_var_capacity(1);
    *source.var_state_mut(0) = VarState::Rigid { name: rigid_name };
    let source_var = source.intern(Tag::RigidVar, 0);

    let mut target = target_with_generalized_slot(0);
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, source_var, &mut target, &mut cache, &mut var_remap);

    let dst_id = target.data(result);
    assert_ne!(dst_id, 0);
    match target.var_state(dst_id) {
        VarState::Rigid { name } => assert_eq!(*name, rigid_name),
        other => panic!("expected Rigid rebuild, got {other:?}"),
    }
}

/// Matrix cell (e4, positive pin) — `Link` variant.
///
/// `Link.target: Idx` is source-pool-local and MUST be
/// rewritten via recursive `re_intern_type` — NOT preserved verbatim (would
/// leak source-pool identity), NOT resolved via `cache.get(&source.target)
/// .expect(...)` (panics on Link targets reachable ONLY through this Link).
#[test]
fn remap_aware_re_intern_rebuilds_link_with_recursively_reinterned_target() {
    let mut source = Pool::new();
    // Source has a list<int> reachable ONLY via VarState::Link (not traversed
    // by any other branch of the type being re-interned), exercising the
    // "cache is not yet populated with source.target" case from the Round 2
    // F1 fix.
    let source_list = source.list(Idx::INT);
    source.ensure_var_capacity(1);
    *source.var_state_mut(0) = VarState::Link {
        target: source_list,
    };
    let source_var = source.intern(Tag::Var, 0);

    let mut target = Pool::new();
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, source_var, &mut target, &mut cache, &mut var_remap);

    let dst_id = target.data(result);
    let dst_link_target = match target.var_state(dst_id) {
        VarState::Link { target: t } => *t,
        other => panic!("expected Link rebuild, got {other:?}"),
    };
    let expected = target.list(Idx::INT);
    assert_eq!(
        dst_link_target, expected,
        "Link.target must be recursively re-interned, NOT preserved verbatim"
    );
}

// --- (e5) Scheme with var-bearing binders AND var-free body -----------------
//
// Per, scheme flags propagate from the BODY
// only — a scheme's raw binder list in `extra` is NOT a propagation source.
// So `Scheme([7], body: Tag::Int)` carries flags with `HAS_VAR | HAS_BOUND_VAR
// | HAS_RIGID_VAR` all clear even though binder `var_id=7` is pool-local and
// MUST remap.

/// Matrix cell (e5, negative pin i).
///
/// Confirms that a `HAS_VAR`-gated fast-path guard on `Tag::Scheme` would
/// miss this case: `source.flags(scheme)` has no var-bit set, so a guard
/// checking only the flag bits would take the hash fast-path and hand the
/// source scheme through unchanged.
#[test]
fn scheme_with_var_bearing_binders_and_var_free_body_has_no_propagated_var_flags() {
    use crate::TypeFlags;

    let mut source = Pool::new();
    let scheme = source.scheme(&[7], Idx::INT);

    let flags = source.flags(scheme);
    assert!(
        !flags.intersects(TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR | TypeFlags::HAS_RIGID_VAR),
        ", binder var-ids do not propagate as parent var-bearing flags — \
         a fast-path guard keyed only on these flags would MISS this scheme"
    );
}

/// Regression guard — backward-compat `re_intern_type` on `Tag::Scheme` with a
/// var-free body still remaps binders.
///
/// This is the edge case `PROPAGATE_MASK` (TF-3) does NOT flag on
/// the scheme parent: the body has no var-bit, so a fast-path guard keyed only
/// on `HAS_VAR | HAS_BOUND_VAR | HAS_RIGID_VAR` would MISS this scheme. The
/// legacy entry point must still route through the remap-aware path AND the
/// `Tag::Scheme` fast-path guard must fire (step 7 — unconditional Scheme skip)
/// so the binder walks step 5 and allocates a fresh `dst_id` for binder `7`.
#[test]
fn re_intern_type_scheme_with_var_free_body_via_legacy_api_remaps_binder() {
    let mut source = Pool::new();
    let scheme = source.scheme(&[7], Idx::INT);

    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let result = re_intern_type(&source, scheme, &mut target, &mut cache);

    assert_eq!(target.tag(result), Tag::Scheme);
    let dst_binders = target.scheme_vars(result).to_vec();
    assert_eq!(dst_binders.len(), 1);
    assert_ne!(
        dst_binders[0], 7,
        "legacy `re_intern_type` must delegate to remap-aware path — \
         source binder id 7 must not leak through even when body is var-free"
    );
    assert_eq!(target.scheme_body(result), Idx::INT);
}

/// Matrix cell (e5, positive pin).
///
/// The unconditional `Tag::Scheme` skip of the fast-path
/// MUST fire even when the scheme's parent flags have no
/// var-bit set, so binders are walked by step 5 regardless of body flags. The
/// resulting scheme's binder list is `[remap[7]]` and — because scheme hashing
/// is extra-backed — its hash in `target` differs from the
/// source's hash even though the body re-intern is a no-op.
#[test]
fn remap_aware_re_intern_scheme_with_var_free_body_remaps_binder_and_changes_hash() {
    let mut source = Pool::new();
    let scheme = source.scheme(&[7], Idx::INT);
    let source_hash = source.hash(scheme);

    let mut target = Pool::new();
    let mut cache = FxHashMap::default();
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();

    let result =
        re_intern_type_with_var_remap(&source, scheme, &mut target, &mut cache, &mut var_remap);

    assert_eq!(target.tag(result), Tag::Scheme);
    let dst_binders = target.scheme_vars(result).to_vec();
    assert_eq!(dst_binders.len(), 1);
    assert_eq!(var_remap.get(&7).copied(), Some(dst_binders[0]));
    assert_eq!(target.scheme_body(result), Idx::INT);
    assert_ne!(
        target.hash(result),
        source_hash,
        "extra-backed scheme hash must differ when binder list is remapped"
    );
}
