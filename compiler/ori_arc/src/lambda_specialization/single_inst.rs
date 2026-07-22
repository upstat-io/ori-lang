//! Single-instantiation lambda monomorphization.
//!
//! Builds the global `BoundVar → concrete` map for lambdas used at exactly
//! one concrete instantiation (the common case). Returns the map, per-lambda
//! return-type resolutions, and per-lambda concrete function types for
//! downstream direct param substitution.

use rustc_hash::FxHashMap;

use ori_types::Idx;

use super::type_predicates::{contains_bound_var, contains_var};
use super::type_resolve::{
    build_bound_var_map, find_apply_indirect_result_type, find_partial_apply_concrete_type,
    is_polymorphic_lambda,
};

/// Result of `build_single_inst_mappings`: global `BoundVar` map, per-lambda
/// return type resolutions, and per-lambda concrete function types.
pub(super) type SingleInstMappings = (
    FxHashMap<u32, Idx>,
    FxHashMap<usize, (Idx, Idx)>,
    FxHashMap<usize, Idx>,
);

/// Build concrete type mappings for singly instantiated lambdas.
///
/// Returns the global `BoundVar` map, per-lambda return resolutions, and
/// concrete function types used to substitute container parameters.
pub(super) fn build_single_inst_mappings(
    parent: &crate::ArcFunction,
    lambdas: &[crate::ArcFunction],
    orig_len: usize,
    multi_inst_lambdas: &rustc_hash::FxHashSet<usize>,
    pool: &ori_types::Pool,
) -> SingleInstMappings {
    let mut global_map: FxHashMap<u32, Idx> = FxHashMap::default();
    let mut ret_type_resolutions: FxHashMap<usize, (Idx, Idx)> = FxHashMap::default();
    let mut concrete_fn_types: FxHashMap<usize, Idx> = FxHashMap::default();

    for i in 0..orig_len {
        if multi_inst_lambdas.contains(&i) {
            continue;
        }
        if !is_polymorphic_lambda(&lambdas[i], pool) {
            continue;
        }

        let lambda_name = lambdas[i].name;
        let lambda_param_count = lambdas[i].params.len();
        let concrete_fn_ty = find_partial_apply_concrete_type(
            parent,
            lambdas,
            i,
            lambda_name,
            lambda_param_count,
            pool,
        );

        if let Some(concrete_ty) = concrete_fn_ty {
            concrete_fn_types.insert(i, concrete_ty);

            build_bound_var_map(
                pool,
                concrete_ty,
                &lambdas[i].params,
                lambdas[i].return_type,
                &mut global_map,
            );

            // Track return type resolution from ApplyIndirect results (not from
            // the function type, which may still contain unresolved Vars inside
            // containers like Option<Var>, Result<Var>).
            //
            // the gate widened from `contains_var` to also detect
            // `Tag::BoundVar` at any nesting depth. Post-normalization
            //  scheme-var leaves in curried closure return
            // types are `Tag::BoundVar`, not `Tag::Var`; without this widening
            // `find_apply_indirect_result_type` never runs and the container
            // return type (e.g., `Function($b17) -> $b16`) reaches LLVM
            // declaration with unresolved leaves, triggering a
            // declared/emitted return type mismatch at verification.
            let schema_ret = lambdas[i].return_type;
            if contains_var(pool, schema_ret) || contains_bound_var(pool, schema_ret) {
                if let Some(concrete_ret) =
                    find_apply_indirect_result_type(parent, lambdas[i].name, pool)
                {
                    if concrete_ret != schema_ret {
                        ret_type_resolutions.insert(i, (schema_ret, concrete_ret));
                    }
                }
            }
        }
    }

    (global_map, ret_type_resolutions, concrete_fn_types)
}
