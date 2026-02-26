//! Type substitution for monomorphization.
//!
//! Provides [`substitute_in_pool`] which recursively replaces type variables
//! with concrete types. Used during monomorphization to build the
//! `body_type_map` (generic `Idx` → concrete `Idx`) that the ARC lowerer
//! uses to emit type-specific retain/release/drop.
//!
//! Follows the same structural recursion pattern as `UnifyEngine::substitute()`
//! but operates as a standalone function on `&mut Pool`, suitable for use
//! during mono instance recording in the type checker.

use rustc_hash::FxHashMap;

use crate::{Idx, Pool, Tag, TypeFlags, VarState};

/// Recursively substitute type variables in `ty` using `var_subst`.
///
/// The substitution map keys are `var_ids` (matching [`FunctionSig::scheme_var_ids`]).
/// Each mapped value is a concrete `Idx` (e.g., `Idx::INT` for `int`).
///
/// Returns the substituted type. If no variables in `ty` match the map,
/// returns `ty` unchanged (O(1) via the `HAS_VAR` flag fast path).
/// New composite types are interned in `pool` (deduplication is automatic).
#[expect(
    clippy::implicit_hasher,
    reason = "always called with FxHashMap internally"
)]
pub fn substitute_in_pool(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    // Fast path: no variables to substitute
    if !pool.flags(ty).contains(TypeFlags::HAS_VAR) {
        return ty;
    }

    match pool.tag(ty) {
        Tag::Var => substitute_var(pool, ty, var_subst),

        // Single-child containers
        Tag::List => substitute_single_child(pool, ty, var_subst, Pool::list),
        Tag::Option => substitute_single_child(pool, ty, var_subst, Pool::option),
        Tag::Set => substitute_single_child(pool, ty, var_subst, Pool::set),
        Tag::Channel => substitute_single_child(pool, ty, var_subst, Pool::channel),
        Tag::Range => substitute_single_child(pool, ty, var_subst, Pool::range),
        Tag::Iterator => substitute_single_child(pool, ty, var_subst, Pool::iterator),
        Tag::DoubleEndedIterator => {
            substitute_single_child(pool, ty, var_subst, Pool::double_ended_iterator)
        }

        // Two-child containers
        Tag::Map => substitute_map(pool, ty, var_subst),
        Tag::Result => substitute_result(pool, ty, var_subst),

        // Borrowed reference
        Tag::Borrowed => substitute_borrowed(pool, ty, var_subst),

        // Variable-length types
        Tag::Function => substitute_function(pool, ty, var_subst),
        Tag::Tuple => substitute_tuple(pool, ty, var_subst),
        Tag::Applied => substitute_applied(pool, ty, var_subst),
        Tag::Struct => substitute_struct(pool, ty, var_subst),

        // Schemes have their own bound variables; primitives and other tags
        // don't contain variables.
        _ => ty,
    }
}

/// Substitute a type variable: check `var_id`, follow links, check generalized.
fn substitute_var(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let var_id = pool.data(ty);

    // Direct var_id match (scheme variable)
    if let Some(&replacement) = var_subst.get(&var_id) {
        return replacement;
    }

    // Follow link if present
    if let VarState::Link { target } = pool.var_state(var_id) {
        let target = *target;
        return substitute_in_pool(pool, target, var_subst);
    }

    // Check for generalized variable (same id, different state)
    if let VarState::Generalized { id, .. } = pool.var_state(var_id) {
        let id = *id;
        if let Some(&replacement) = var_subst.get(&id) {
            return replacement;
        }
    }

    ty
}

/// Substitute in a single-child container (List, Option, Set, etc.).
fn substitute_single_child(
    pool: &mut Pool,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
    ctor: fn(&mut Pool, Idx) -> Idx,
) -> Idx {
    let child = Idx::from_raw(pool.data(ty));
    let new_child = substitute_in_pool(pool, child, var_subst);
    if new_child == child {
        ty
    } else {
        ctor(pool, new_child)
    }
}

/// Substitute in a Map type (key + value).
fn substitute_map(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let key = pool.map_key(ty);
    let value = pool.map_value(ty);
    let new_key = substitute_in_pool(pool, key, var_subst);
    let new_value = substitute_in_pool(pool, value, var_subst);
    if new_key == key && new_value == value {
        ty
    } else {
        pool.map(new_key, new_value)
    }
}

/// Substitute in a Result type (ok + err).
fn substitute_result(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let ok = pool.result_ok(ty);
    let err = pool.result_err(ty);
    let new_ok = substitute_in_pool(pool, ok, var_subst);
    let new_err = substitute_in_pool(pool, err, var_subst);
    if new_ok == ok && new_err == err {
        ty
    } else {
        pool.result(new_ok, new_err)
    }
}

/// Substitute in a Borrowed reference (inner + lifetime preserved).
fn substitute_borrowed(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let inner = pool.borrowed_inner(ty);
    let lt = pool.borrowed_lifetime(ty);
    let new_inner = substitute_in_pool(pool, inner, var_subst);
    if new_inner == inner {
        ty
    } else {
        pool.borrowed(new_inner, lt)
    }
}

/// Substitute in a Function type (params + return).
fn substitute_function(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let params = pool.function_params(ty);
    let ret = pool.function_return(ty);

    let mut changed = false;
    let new_params: Vec<Idx> = params
        .iter()
        .map(|&p| {
            let new_p = substitute_in_pool(pool, p, var_subst);
            if new_p != p {
                changed = true;
            }
            new_p
        })
        .collect();

    let new_ret = substitute_in_pool(pool, ret, var_subst);
    if new_ret != ret {
        changed = true;
    }

    if changed {
        pool.function(&new_params, new_ret)
    } else {
        ty
    }
}

/// Substitute in a Tuple type (element list).
fn substitute_tuple(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let elems = pool.tuple_elems(ty);

    let mut changed = false;
    let new_elems: Vec<Idx> = elems
        .iter()
        .map(|&e| {
            let new_e = substitute_in_pool(pool, e, var_subst);
            if new_e != e {
                changed = true;
            }
            new_e
        })
        .collect();

    if changed {
        pool.tuple(&new_elems)
    } else {
        ty
    }
}

/// Substitute in an Applied type (name + type args).
fn substitute_applied(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let name = pool.applied_name(ty);
    let args = pool.applied_args(ty);

    let mut changed = false;
    let new_args: Vec<Idx> = args
        .iter()
        .map(|&a| {
            let new_a = substitute_in_pool(pool, a, var_subst);
            if new_a != a {
                changed = true;
            }
            new_a
        })
        .collect();

    if changed {
        pool.applied(name, &new_args)
    } else {
        ty
    }
}

/// Extract the concrete type corresponding to a type variable from parallel type trees.
///
/// Walks `generic_ty` and `concrete_ty` in parallel. When a `Var` node in
/// `generic_ty` has `data == target_var_id`, returns the corresponding node
/// from `concrete_ty`. Returns `None` if the variable isn't found.
///
/// Used during monomorphization to resolve type params that don't appear
/// directly as function parameters (e.g., `T` in `f(x: Pair<T, int>)`).
pub fn extract_var_from_types(
    pool: &Pool,
    generic_ty: Idx,
    concrete_ty: Idx,
    target_var_id: u32,
) -> Option<Idx> {
    match pool.tag(generic_ty) {
        Tag::Var => {
            if pool.data(generic_ty) == target_var_id {
                return Some(concrete_ty);
            }
            None
        }

        // Single-child containers
        Tag::List
        | Tag::Option
        | Tag::Set
        | Tag::Channel
        | Tag::Range
        | Tag::Iterator
        | Tag::DoubleEndedIterator => {
            let g_child = Idx::from_raw(pool.data(generic_ty));
            let c_child = Idx::from_raw(pool.data(concrete_ty));
            extract_var_from_types(pool, g_child, c_child, target_var_id)
        }

        // Two-child containers
        Tag::Map => {
            let g_key = pool.map_key(generic_ty);
            let c_key = pool.map_key(concrete_ty);
            if let Some(found) = extract_var_from_types(pool, g_key, c_key, target_var_id) {
                return Some(found);
            }
            let g_val = pool.map_value(generic_ty);
            let c_val = pool.map_value(concrete_ty);
            extract_var_from_types(pool, g_val, c_val, target_var_id)
        }
        Tag::Result => {
            let g_ok = pool.result_ok(generic_ty);
            let c_ok = pool.result_ok(concrete_ty);
            if let Some(found) = extract_var_from_types(pool, g_ok, c_ok, target_var_id) {
                return Some(found);
            }
            let g_err = pool.result_err(generic_ty);
            let c_err = pool.result_err(concrete_ty);
            extract_var_from_types(pool, g_err, c_err, target_var_id)
        }

        // Applied type (e.g., Pair<T, int>)
        Tag::Applied => {
            let g_args = pool.applied_args(generic_ty);
            let c_args = pool.applied_args(concrete_ty);
            for (g, c) in g_args.iter().zip(c_args.iter()) {
                if let Some(found) = extract_var_from_types(pool, *g, *c, target_var_id) {
                    return Some(found);
                }
            }
            None
        }

        // Tuple
        Tag::Tuple => {
            let g_elems = pool.tuple_elems(generic_ty);
            let c_elems = pool.tuple_elems(concrete_ty);
            for (g, c) in g_elems.iter().zip(c_elems.iter()) {
                if let Some(found) = extract_var_from_types(pool, *g, *c, target_var_id) {
                    return Some(found);
                }
            }
            None
        }

        // Function type
        Tag::Function => {
            let g_params = pool.function_params(generic_ty);
            let c_params = pool.function_params(concrete_ty);
            for (g, c) in g_params.iter().zip(c_params.iter()) {
                if let Some(found) = extract_var_from_types(pool, *g, *c, target_var_id) {
                    return Some(found);
                }
            }
            let g_ret = pool.function_return(generic_ty);
            let c_ret = pool.function_return(concrete_ty);
            extract_var_from_types(pool, g_ret, c_ret, target_var_id)
        }

        // Struct type
        Tag::Struct => {
            let g_fields = pool.struct_fields(generic_ty);
            let c_fields = pool.struct_fields(concrete_ty);
            for ((_, g_ty), (_, c_ty)) in g_fields.iter().zip(c_fields.iter()) {
                if let Some(found) = extract_var_from_types(pool, *g_ty, *c_ty, target_var_id) {
                    return Some(found);
                }
            }
            None
        }

        // Primitives and other non-compound types — no vars to find
        _ => None,
    }
}

/// Substitute in a Struct type (field types, preserving field names).
fn substitute_struct(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let name = pool.struct_name(ty);
    let fields = pool.struct_fields(ty);

    let mut changed = false;
    let new_fields: Vec<(ori_ir::Name, Idx)> = fields
        .iter()
        .map(|&(field_name, field_ty)| {
            let new_ty = substitute_in_pool(pool, field_ty, var_subst);
            if new_ty != field_ty {
                changed = true;
            }
            (field_name, new_ty)
        })
        .collect();

    if changed {
        pool.struct_type(name, &new_fields)
    } else {
        ty
    }
}

#[cfg(test)]
mod tests;
