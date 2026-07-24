//! Generic-type-parameter substitution for pattern and identifier inference.

use ori_ir::Name;

use crate::{Idx, Tag};

use super::super::super::InferEngine;

/// Substitute generic type parameters in a field type with concrete type arguments.
///
/// Given a field type like `Named("T")` and a mapping `[T] -> [int]`, returns `int`.
/// For compound types (lists, tuples, functions, applied types), recurses into children.
/// Non-parameterized types (primitives, error, etc.) are returned unchanged.
pub(crate) fn substitute_type_params(
    engine: &mut InferEngine<'_>,
    field_ty: Idx,
    type_params: &[ori_ir::Name],
    type_args: &[Idx],
) -> Idx {
    let resolved = engine.resolve(field_ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Named => {
            // Check if this named type is one of the type parameters
            let name = engine.pool().named_name(resolved);
            for (i, &param_name) in type_params.iter().enumerate() {
                if name == param_name {
                    return type_args[i];
                }
            }
            // Not a type parameter — return as-is (concrete named type)
            resolved
        }
        Tag::Applied => {
            // Recurse into applied type arguments: e.g., List<T> -> List<int>
            let app_name = engine.pool().applied_name(resolved);
            let args = engine.pool().applied_args(resolved);
            let substituted_args: Vec<Idx> = args
                .iter()
                .map(|&arg| substitute_type_params(engine, arg, type_params, type_args))
                .collect();
            engine.pool_mut().applied(app_name, &substituted_args)
        }
        Tag::List => {
            let elem = engine.pool().list_elem(resolved);
            let sub_elem = substitute_type_params(engine, elem, type_params, type_args);
            engine.pool_mut().list(sub_elem)
        }
        Tag::Tuple => {
            let elems = engine.pool().tuple_elems(resolved);
            let sub_elems: Vec<Idx> = elems
                .iter()
                .map(|&e| substitute_type_params(engine, e, type_params, type_args))
                .collect();
            engine.pool_mut().tuple(&sub_elems)
        }
        Tag::Function => {
            let params = engine.pool().function_params(resolved);
            let ret = engine.pool().function_return(resolved);
            let sub_params: Vec<Idx> = params
                .iter()
                .map(|&p| substitute_type_params(engine, p, type_params, type_args))
                .collect();
            let sub_ret = substitute_type_params(engine, ret, type_params, type_args);
            engine.pool_mut().function(&sub_params, sub_ret)
        }
        Tag::Option => {
            let inner = engine.pool().option_inner(resolved);
            let sub_inner = substitute_type_params(engine, inner, type_params, type_args);
            engine.pool_mut().option(sub_inner)
        }
        Tag::Result => {
            let ok = engine.pool().result_ok(resolved);
            let err = engine.pool().result_err(resolved);
            let sub_ok = substitute_type_params(engine, ok, type_params, type_args);
            let sub_err = substitute_type_params(engine, err, type_params, type_args);
            engine.pool_mut().result(sub_ok, sub_err)
        }
        Tag::Map => {
            let key = engine.pool().map_key(resolved);
            let val = engine.pool().map_value(resolved);
            let sub_key = substitute_type_params(engine, key, type_params, type_args);
            let sub_val = substitute_type_params(engine, val, type_params, type_args);
            engine.pool_mut().map(sub_key, sub_val)
        }
        // Primitives and other leaf types — no substitution needed
        _ => resolved,
    }
}

/// Substitute type parameters using a pre-built map of (Name, Idx) pairs.
///
/// This is a convenience wrapper around `substitute_type_params` that accepts
/// a map representation rather than parallel arrays.
pub(crate) fn substitute_type_params_with_map(
    engine: &mut InferEngine<'_>,
    field_ty: Idx,
    subst_map: &[(Name, Idx)],
) -> Idx {
    if subst_map.is_empty() {
        return field_ty;
    }
    let type_params: Vec<Name> = subst_map.iter().map(|(n, _)| *n).collect();
    let type_args: Vec<Idx> = subst_map.iter().map(|(_, i)| *i).collect();
    substitute_type_params(engine, field_ty, &type_params, &type_args)
}
