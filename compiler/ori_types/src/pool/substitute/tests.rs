//! Tests for `substitute_in_pool`.

use rustc_hash::FxHashMap;

use super::substitute_in_pool;
use crate::{Idx, Pool, Tag};

/// Helper: build a `var_subst` map from `[(var_id, concrete_idx)]` pairs.
fn subst(pairs: &[(u32, Idx)]) -> FxHashMap<u32, Idx> {
    pairs.iter().copied().collect()
}

// === Primitives (fast path) ===

#[test]
fn primitive_unchanged() {
    let mut pool = Pool::new();
    let map = subst(&[(0, Idx::BOOL)]);

    // Primitives have no HAS_VAR flag, so they're returned as-is.
    assert_eq!(substitute_in_pool(&mut pool, Idx::INT, &map), Idx::INT);
    assert_eq!(substitute_in_pool(&mut pool, Idx::STR, &map), Idx::STR);
    assert_eq!(substitute_in_pool(&mut pool, Idx::UNIT, &map), Idx::UNIT);
}

// === Single variable ===

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

// === Single-child containers ===

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

// === Two-child containers ===

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

// === Function type ===

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

// === Tuple ===

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

// === Applied ===

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

// === Nested types ===

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

// === Interning deduplication ===

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

// === No-op when no vars ===

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

// === Linked variable ===

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

// === Generalized variable ===

#[test]
fn generalized_var_substituted() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);

    // Generalize the var (simulates let-polymorphism).
    *pool.var_state_mut(var_id) = crate::VarState::Generalized {
        id: var_id,
        name: None,
    };

    let map = subst(&[(var_id, Idx::STR)]);
    let result = substitute_in_pool(&mut pool, var, &map);
    assert_eq!(result, Idx::STR);
}

// === Struct type ===

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
