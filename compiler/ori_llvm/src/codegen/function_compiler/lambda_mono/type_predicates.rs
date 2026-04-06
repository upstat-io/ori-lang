//! Type predicate and structural traversal helpers for lambda monomorphization.
//!
//! Recursive predicates for checking whether types contain `Var` or `BoundVar`
//! at any nesting level, and parallel structural walks for building type mappings.

use ori_types::Idx;
use ori_types::Tag;

/// Check if a type contains a `Var` at any nesting level.
pub(super) fn contains_var(pool: &ori_types::Pool, ty: Idx) -> bool {
    match pool.tag(ty) {
        Tag::Var => true,
        Tag::Option => contains_var(pool, pool.option_inner(ty)),
        Tag::Result => {
            contains_var(pool, pool.result_ok(ty)) || contains_var(pool, pool.result_err(ty))
        }
        Tag::List => contains_var(pool, pool.list_elem(ty)),
        Tag::Tuple => pool.tuple_elems(ty).iter().any(|e| contains_var(pool, *e)),
        Tag::Map => contains_var(pool, pool.map_key(ty)) || contains_var(pool, pool.map_value(ty)),
        Tag::Set => contains_var(pool, pool.set_elem(ty)),
        Tag::Function => {
            pool.function_params(ty)
                .iter()
                .any(|p| contains_var(pool, *p))
                || contains_var(pool, pool.function_return(ty))
        }
        _ => false,
    }
}

/// Check if a type contains any unresolvable `BoundVar`.
pub(super) fn contains_bound_var(pool: &ori_types::Pool, ty: Idx) -> bool {
    let resolved = pool.resolve_fully(ty);
    match pool.tag(resolved) {
        Tag::BoundVar | Tag::Scheme => true,
        Tag::Option => contains_bound_var(pool, pool.option_inner(resolved)),
        Tag::Result => {
            contains_bound_var(pool, pool.result_ok(resolved))
                || contains_bound_var(pool, pool.result_err(resolved))
        }
        Tag::List => contains_bound_var(pool, pool.list_elem(resolved)),
        Tag::Tuple => pool
            .tuple_elems(resolved)
            .iter()
            .any(|e| contains_bound_var(pool, *e)),
        Tag::Map => {
            contains_bound_var(pool, pool.map_key(resolved))
                || contains_bound_var(pool, pool.map_value(resolved))
        }
        Tag::Set => contains_bound_var(pool, pool.set_elem(resolved)),
        Tag::Function => {
            pool.function_params(resolved)
                .iter()
                .any(|p| contains_bound_var(pool, *p))
                || contains_bound_var(pool, pool.function_return(resolved))
        }
        _ => false,
    }
}

/// Walk `schema_ty` and `concrete_ty` in parallel to build `BoundVar` mappings.
pub(super) fn map_types_structural(
    pool: &ori_types::Pool,
    schema_ty: Idx,
    concrete_ty: Idx,
    map: &mut rustc_hash::FxHashMap<u32, Idx>,
) {
    let schema_tag = pool.tag(schema_ty);

    if matches!(schema_tag, Tag::BoundVar | Tag::Var) {
        let var_id = pool.data(schema_ty);
        map.insert(var_id, concrete_ty);
        return;
    }

    let concrete_tag = pool.tag(concrete_ty);
    if schema_tag != concrete_tag {
        return;
    }

    match schema_tag {
        Tag::Option => {
            map_types_structural(
                pool,
                pool.option_inner(schema_ty),
                pool.option_inner(concrete_ty),
                map,
            );
        }
        Tag::Result => {
            map_types_structural(
                pool,
                pool.result_ok(schema_ty),
                pool.result_ok(concrete_ty),
                map,
            );
            map_types_structural(
                pool,
                pool.result_err(schema_ty),
                pool.result_err(concrete_ty),
                map,
            );
        }
        Tag::List => {
            map_types_structural(
                pool,
                pool.list_elem(schema_ty),
                pool.list_elem(concrete_ty),
                map,
            );
        }
        Tag::Tuple => {
            let s_elems = pool.tuple_elems(schema_ty);
            let c_elems = pool.tuple_elems(concrete_ty);
            for (se, ce) in s_elems.iter().zip(c_elems.iter()) {
                map_types_structural(pool, *se, *ce, map);
            }
        }
        Tag::Map => {
            map_types_structural(
                pool,
                pool.map_key(schema_ty),
                pool.map_key(concrete_ty),
                map,
            );
            map_types_structural(
                pool,
                pool.map_value(schema_ty),
                pool.map_value(concrete_ty),
                map,
            );
        }
        Tag::Set => {
            map_types_structural(
                pool,
                pool.set_elem(schema_ty),
                pool.set_elem(concrete_ty),
                map,
            );
        }
        Tag::Function => {
            let s_params = pool.function_params(schema_ty);
            let c_params = pool.function_params(concrete_ty);
            for (sp, cp) in s_params.iter().zip(c_params.iter()) {
                map_types_structural(pool, *sp, *cp, map);
            }
            map_types_structural(
                pool,
                pool.function_return(schema_ty),
                pool.function_return(concrete_ty),
                map,
            );
        }
        _ => {}
    }
}
