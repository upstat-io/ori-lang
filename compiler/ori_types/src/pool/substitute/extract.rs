//! Extract the concrete type corresponding to a target `var_id` by walking
//! `generic_ty` and `concrete_ty` in parallel.
//!
//! Used during monomorphization to resolve type params that don't appear
//! directly as function parameters (e.g., `T` in `f(x: Pair<T, int>)`).
//! The entry point is [`extract_var_from_types`]; per-tag-category helpers
//! share the recursive walk skeleton.

use crate::{Idx, Pool, Tag};

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
        // Match both `Tag::Var` (pre-normalization) and `Tag::BoundVar`.
        Tag::Var | Tag::BoundVar => extract_var_leaf(pool, generic_ty, concrete_ty, target_var_id),

        // Single-child containers
        Tag::List
        | Tag::Option
        | Tag::Set
        | Tag::Channel
        | Tag::Range
        | Tag::Iterator
        | Tag::DoubleEndedIterator => {
            extract_var_simple_container(pool, generic_ty, concrete_ty, target_var_id)
        }

        // Two-child containers
        Tag::Map | Tag::Result => {
            extract_var_two_child(pool, generic_ty, concrete_ty, target_var_id)
        }

        // Applied type (e.g., Pair<T, int>)
        Tag::Applied => extract_var_applied(pool, generic_ty, concrete_ty, target_var_id),

        // Tuple
        Tag::Tuple => extract_var_tuple(pool, generic_ty, concrete_ty, target_var_id),

        // Function type
        Tag::Function => extract_var_function(pool, generic_ty, concrete_ty, target_var_id),

        // Struct type
        Tag::Struct => extract_var_struct(pool, generic_ty, concrete_ty, target_var_id),

        // Primitives and other non-compound types — no vars to find
        _ => None,
    }
}

/// Leaf case: `Tag::Var` / `Tag::BoundVar`. Matches when `var_id` equals target.
fn extract_var_leaf(
    pool: &Pool,
    generic_ty: Idx,
    concrete_ty: Idx,
    target_var_id: u32,
) -> Option<Idx> {
    if pool.data(generic_ty) == target_var_id {
        Some(concrete_ty)
    } else {
        None
    }
}

/// Single-child container: recurse into the child.
fn extract_var_simple_container(
    pool: &Pool,
    generic_ty: Idx,
    concrete_ty: Idx,
    target_var_id: u32,
) -> Option<Idx> {
    let g_child = Idx::from_raw(pool.data(generic_ty));
    let c_child = Idx::from_raw(pool.data(concrete_ty));
    extract_var_from_types(pool, g_child, c_child, target_var_id)
}

/// Two-child container: `Map` or `Result`.
fn extract_var_two_child(
    pool: &Pool,
    generic_ty: Idx,
    concrete_ty: Idx,
    target_var_id: u32,
) -> Option<Idx> {
    match pool.tag(generic_ty) {
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
        _ => None,
    }
}

/// Applied generic (e.g. `Pair<T, int>`): recurse through arg pairs.
fn extract_var_applied(
    pool: &Pool,
    generic_ty: Idx,
    concrete_ty: Idx,
    target_var_id: u32,
) -> Option<Idx> {
    let g_args = pool.applied_args(generic_ty);
    let c_args = pool.applied_args(concrete_ty);
    for (g, c) in g_args.iter().zip(c_args.iter()) {
        if let Some(found) = extract_var_from_types(pool, *g, *c, target_var_id) {
            return Some(found);
        }
    }
    None
}

/// Tuple: recurse through element pairs.
fn extract_var_tuple(
    pool: &Pool,
    generic_ty: Idx,
    concrete_ty: Idx,
    target_var_id: u32,
) -> Option<Idx> {
    let g_elems = pool.tuple_elems(generic_ty);
    let c_elems = pool.tuple_elems(concrete_ty);
    for (g, c) in g_elems.iter().zip(c_elems.iter()) {
        if let Some(found) = extract_var_from_types(pool, *g, *c, target_var_id) {
            return Some(found);
        }
    }
    None
}

/// Function: recurse through parameters, then the return type.
fn extract_var_function(
    pool: &Pool,
    generic_ty: Idx,
    concrete_ty: Idx,
    target_var_id: u32,
) -> Option<Idx> {
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

/// Struct: recurse through field-type pairs (field names are not relevant).
fn extract_var_struct(
    pool: &Pool,
    generic_ty: Idx,
    concrete_ty: Idx,
    target_var_id: u32,
) -> Option<Idx> {
    let g_fields = pool.struct_fields(generic_ty);
    let c_fields = pool.struct_fields(concrete_ty);
    for ((_, g_ty), (_, c_ty)) in g_fields.iter().zip(c_fields.iter()) {
        if let Some(found) = extract_var_from_types(pool, *g_ty, *c_ty, target_var_id) {
            return Some(found);
        }
    }
    None
}
