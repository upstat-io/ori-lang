//! Lambda monomorphization: resolve polymorphic lambda types to concrete types.
//!
//! Handles `BoundVar`/`Var` resolution in lambda ARC functions before LLVM emission.
//! Two strategies:
//! - **Single-instantiation**: lambda used at one concrete type → resolve directly
//! - **Multi-instantiation**: lambda used at multiple types → clone per instantiation,
//!   rewrite parent ARC IR to use specialized clones
//!
//! # Submodules
//!
//! - `single_inst` — global `BoundVar → concrete` mapping for single-inst
//!   lambdas (the common case).
//! - `multi_inst` — clone-per-instantiation + parent rewrite.
//! - `call_site` — post-fixup of `var_types` / `var_reprs` / RC ops after
//!   either strategy runs.
//! - `type_predicates` — shared predicates (`is_polymorphic_lambda`,
//!   `contains_var`, `has_concrete_params`).
//! - `type_resolve` — `BoundVar` map construction + apply helpers.

mod call_site;
mod multi_inst;
mod single_inst;
mod type_predicates;
mod type_resolve;

#[cfg(test)]
mod tests;

use ori_types::Tag;

use call_site::fixup_parent_var_reprs_and_rc_ops;
use multi_inst::{detect_and_clone_multi_inst, remove_multi_inst_originals};
use single_inst::build_single_inst_mappings;
use type_predicates::contains_var;
use type_resolve::{
    apply_bound_var_map, apply_call_site_types, apply_concrete_param_types,
    fallback_bound_vars_to_int, find_all_instantiation_types, find_concrete_types_from_calls,
    find_partial_apply_dst, is_polymorphic_lambda, resolve_lambda_return_types,
};

/// Resolve `BoundVar` types in ALL lambdas to concrete types.
///
/// Handles two cases:
/// 1. **Single-instantiation**: a lambda used at one concrete type → resolve directly
/// 2. **Multi-instantiation**: a lambda used at multiple concrete types (e.g.,
///    `let $id = x -> x; id("hello"); id(42)`) → clone per instantiation and
///    rewrite the parent's ARC IR so each use gets the correct specialization
pub(super) fn resolve_all_lambda_bound_vars(
    parent: &mut ori_arc::ArcFunction,
    lambdas: &mut Vec<ori_arc::ArcFunction>,
    pool: &ori_types::Pool,
    interner: &ori_ir::StringInterner,
    classifier: &dyn ori_arc::ArcClassification,
) {
    // Check if ANY lambda has polymorphic types or multi-instantiation.
    let any_polymorphic = lambdas.iter().any(|l| is_polymorphic_lambda(l, pool));
    let any_multi_inst = !any_polymorphic
        && lambdas
            .iter()
            .any(|l| find_all_instantiation_types(parent, l.name, pool).len() > 1);
    if !any_polymorphic && !any_multi_inst {
        return;
    }

    let orig_len = lambdas.len();
    let multi_inst_lambdas = detect_and_clone_multi_inst(parent, lambdas, pool, interner);
    let (global_map, ret_type_resolutions, concrete_fn_types) =
        build_single_inst_mappings(parent, lambdas, orig_len, &multi_inst_lambdas, pool);

    // Apply the global map + return type resolutions to ALL non-multi-inst lambdas.
    for (i, lambda) in lambdas.iter_mut().enumerate() {
        if i < orig_len && multi_inst_lambdas.contains(&i) {
            continue;
        }
        apply_bound_var_map(lambda, &global_map, pool);

        if let Some(&(schema_ret, concrete_ret)) = ret_type_resolutions.get(&i) {
            resolve_lambda_return_types(lambda, schema_ret, concrete_ret);
        }

        // Direct param substitution for container types with nested vars.
        // apply_bound_var_map only resolves top-level Var/BoundVar; this handles
        // containers like List<Var>, Option<Var> by directly substituting from
        // the concrete function type's param types.
        if let Some(&concrete_fn_ty) = concrete_fn_types.get(&i) {
            apply_concrete_param_types(lambda, concrete_fn_ty, pool);
        }
    }

    // For lambdas where no concrete function type was found via PartialApply
    // narrowing, try extracting concrete types from ApplyIndirect call sites.
    // This handles let-polymorphic lambdas (e.g., `let head = xs -> xs[0]`)
    // where type narrowing happens at the call site, not via Let copies.
    //
    // Safety: only process ORIGINAL lambdas (not multi-inst clones which are
    // appended after orig_len), and only when the lambda's params still contain
    // unresolved Generalized vars after all other resolution attempts.
    #[expect(
        clippy::needless_range_loop,
        reason = "need index for multi_inst_lambdas/concrete_fn_types lookup and mutable lambdas[i] access"
    )]
    for i in 0..orig_len {
        if multi_inst_lambdas.contains(&i) {
            continue;
        }
        if concrete_fn_types.contains_key(&i) {
            continue; // Already resolved via concrete function type.
        }
        let lambda = &lambdas[i];
        // Only try call-site extraction if params still contain nested vars.
        let has_unresolved_container_params = lambda.params.iter().any(|p| {
            !matches!(pool.tag(p.ty), Tag::BoundVar | Tag::Var) && contains_var(pool, p.ty)
        });
        if !has_unresolved_container_params {
            continue;
        }
        // Find the PartialApply dst for this lambda in the parent.
        if let Some(pa_dst) = find_partial_apply_dst(parent, lambda.name) {
            if let Some((arg_types, result_ty)) =
                find_concrete_types_from_calls(parent, pa_dst, pool)
            {
                tracing::debug!(
                    name = ?lambdas[i].name,
                    "lambda mono: resolved from ApplyIndirect call site"
                );
                apply_call_site_types(&mut lambdas[i], &arg_types, result_ty, pool);
            }
        }
    }

    // Final fallback: any remaining BoundVars/Vars → Idx::INT.
    for (i, lambda) in lambdas.iter_mut().enumerate() {
        if i < orig_len && multi_inst_lambdas.contains(&i) {
            continue;
        }
        fallback_bound_vars_to_int(lambda, pool);
    }

    remove_multi_inst_originals(lambdas, multi_inst_lambdas);

    // Recompute parent var_reprs and fix RC ops after type fixups.
    // fixup_call_result_types may have changed var_types (e.g., Var→int),
    // making existing RcInc/RcDec ops invalid (wrong strategy or RC on Scalar).
    fixup_parent_var_reprs_and_rc_ops(parent, classifier, pool);
}
