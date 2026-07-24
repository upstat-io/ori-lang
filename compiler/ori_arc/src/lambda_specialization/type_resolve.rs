//! Type resolution helpers for lambda monomorphization.
//!
//! Functions for finding concrete types from parent ARC IR, building `BoundVar`
//! mappings, applying substitutions, and type predicate checks.

mod closure_sites;

pub(super) use closure_sites::{
    find_all_instantiation_types, find_apply_indirect_result_type, find_concrete_types_from_calls,
    find_partial_apply_args, find_partial_apply_dst,
};

use ori_ir::Name;
use ori_types::Idx;
use ori_types::Tag;

use super::type_predicates::{
    contains_bound_var, contains_var, has_concrete_params, map_types_structural,
};

/// Read-only type-pool access plus fail-closed materialization evidence for one
/// lambda-specialization run.
pub(super) struct TypeResolution<'pool> {
    pool: &'pool ori_types::Pool,
    missing: Vec<super::MissingTypeMaterialization>,
}

impl<'pool> TypeResolution<'pool> {
    pub(super) fn new(pool: &'pool ori_types::Pool) -> Self {
        Self {
            pool,
            missing: Vec::new(),
        }
    }

    pub(super) const fn pool(&self) -> &'pool ori_types::Pool {
        self.pool
    }

    pub(super) fn has_missing(&self) -> bool {
        !self.missing.is_empty()
    }

    pub(super) fn into_missing(self) -> Vec<super::MissingTypeMaterialization> {
        self.missing
    }

    fn record_missing(&mut self, function: Name, var_id: crate::ArcVarId, source: Idx) {
        let failure = super::MissingTypeMaterialization::new(function, var_id, source);
        if !self.missing.contains(&failure) {
            self.missing.push(failure);
        }
    }
}

/// Check if a resolved type is a Function with all-concrete params and return type.
pub(super) fn is_concrete_function(pool: &ori_types::Pool, resolved: Idx) -> bool {
    if pool.tag(resolved) != Tag::Function {
        return false;
    }
    let params = pool.function_params(resolved);
    let ret = pool.function_return(resolved);
    params
        .iter()
        .chain(std::iter::once(&ret))
        .all(|&component| is_closed_type(pool, component))
}

fn is_closed_type(pool: &ori_types::Pool, ty: Idx) -> bool {
    let resolved = pool.resolve_fully(ty);
    !matches!(pool.tag(resolved), Tag::BoundVar | Tag::Var | Tag::Scheme)
        && !contains_var(pool, resolved)
        && !contains_bound_var(pool, resolved)
}

/// Check if a lambda has any unresolved polymorphic types (`BoundVar`/`Var` in
/// params, return, or `var_types`).
///
/// Checks top-level variables and variables nested inside container types such
/// as `List<Var>` and `Option<Var>`. Let-polymorphic generalized variables use
/// `Tag::Var`; schemes use `Tag::BoundVar` or `Tag::Scheme`.
pub(super) fn is_polymorphic_lambda(lambda: &crate::ArcFunction, pool: &ori_types::Pool) -> bool {
    // INVARIANT: Parameter checks cover both generalized and scheme-variable leaves.
    lambda.params.iter().any(|p| {
        matches!(pool.tag(p.ty), Tag::BoundVar | Tag::Var)
            || contains_var(pool, p.ty)
            || contains_bound_var(pool, p.ty)
    })
    || contains_bound_var(pool, lambda.return_type)
    // var_types: only check for BoundVar (original behavior).
    || lambda
            .var_types
            .iter()
            .any(|ty| contains_bound_var(pool, *ty))
}

/// Search parent + all sibling lambdas for a `PartialApply` that references the
/// given lambda, and return the concrete instantiated function type.
pub(super) fn find_partial_apply_concrete_type(
    parent: &crate::ArcFunction,
    lambdas: &[crate::ArcFunction],
    skip_idx: usize,
    lambda_name: Name,
    lambda_param_count: usize,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    let check_concrete = |func: &crate::ArcFunction, dst: &crate::ir::ArcVarId| -> Option<Idx> {
        let pa_ty = func.var_type(*dst);
        let resolved = pool.resolve_fully(pa_ty);
        if is_concrete_function(pool, resolved)
            && arity_compatible(pool, resolved, lambda_param_count)
        {
            return Some(resolved);
        }
        None
    };

    // Search parent first.
    if let Some(dst) = find_partial_apply_dst(parent, lambda_name) {
        if let Some(ty) = check_concrete(parent, &dst) {
            return Some(ty);
        }
        if let Some(ty) = find_concrete_copy_of(parent, dst, pool, lambda_param_count) {
            return Some(ty);
        }
    }

    // Search sibling lambdas (skip self).
    for (j, sibling) in lambdas.iter().enumerate() {
        if j == skip_idx {
            continue;
        }
        if let Some(dst) = find_partial_apply_dst(sibling, lambda_name) {
            if let Some(ty) = check_concrete(sibling, &dst) {
                return Some(ty);
            }
            if let Some(ty) = find_concrete_copy_of(sibling, dst, pool, lambda_param_count) {
                return Some(ty);
            }
            if let Some(parent_dst) = find_partial_apply_dst(parent, lambda_name) {
                if let Some(ty) =
                    find_concrete_copy_of(parent, parent_dst, pool, lambda_param_count)
                {
                    return Some(ty);
                }
            }
        }
    }

    None
}

/// Apply a `BoundVar` → concrete mapping to a lambda's types.
pub(super) fn apply_bound_var_map(
    lambda: &mut crate::ArcFunction,
    map: &rustc_hash::FxHashMap<u32, Idx>,
    resolution: &mut TypeResolution<'_>,
) {
    resolve_type_sites(lambda, map, resolution);
}

/// Resolve every ARC type site through the type phase's existing canonical
/// identities. Schemes are instantiated at this executable-lambda seam; the
/// general pool helper deliberately leaves them opaque because it cannot know
/// whether a caller owns their binders. Missing identities fail closed instead
/// of granting ARC authority to extend the type pool.
pub(super) fn resolve_type_sites(
    function: &mut crate::ArcFunction,
    substitutions: &rustc_hash::FxHashMap<u32, Idx>,
    resolution: &mut TypeResolution<'_>,
) {
    let function_name = function.name;
    crate::ir::validate::rewrite_type_sites(function, |ty, var_id| {
        let pool = resolution.pool();
        let materialized = if pool.tag(ty) == Tag::Scheme {
            pool.scheme_body(ty)
        } else {
            ty
        };
        match ori_types::substitute_in_existing_pool(pool, materialized, substitutions) {
            Ok(resolved) => resolved,
            Err(error) => {
                resolution.record_missing(function_name, var_id, error.source());
                ty
            }
        }
    });
}

/// Apply exact schema-to-concrete substitutions at every ARC type position.
fn apply_exact_type_map(
    function: &mut crate::ArcFunction,
    substitutions: &rustc_hash::FxHashMap<Idx, Idx>,
    resolution: &mut TypeResolution<'_>,
) {
    if substitutions.is_empty() {
        return;
    }
    let function_name = function.name;
    let empty = rustc_hash::FxHashMap::default();
    crate::ir::validate::rewrite_type_sites(function, |ty, var_id| {
        let pool = resolution.pool();
        let replacement = substitutions
            .get(&ty)
            .or_else(|| substitutions.get(&pool.resolve_fully(ty)))
            .copied()
            .unwrap_or(ty);
        let replacement = if pool.tag(replacement) == Tag::Scheme {
            pool.scheme_body(replacement)
        } else {
            replacement
        };
        match ori_types::substitute_in_existing_pool(pool, replacement, &empty) {
            Ok(resolved) => resolved,
            Err(error) => {
                resolution.record_missing(function_name, var_id, error.source());
                ty
            }
        }
    });
}

/// Directly substitute lambda params and `var_types` using the concrete function
/// type's parameter types. Handles container types with nested vars that
/// `apply_bound_var_map` cannot resolve (it only handles top-level `Var`/`BoundVar`).
///
/// For example, if a lambda has param `xs: List<Var(X)>` and the concrete
/// function type has param `List<int>`, this directly sets `xs.ty = List<int>`.
/// Also builds an `Idx → Idx` map from schema→concrete param types and applies
/// it to `var_types` and `Construct` instruction types.
pub(super) fn apply_concrete_param_types(
    lambda: &mut crate::ArcFunction,
    concrete_fn_ty: Idx,
    resolution: &mut TypeResolution<'_>,
) {
    let pool = resolution.pool();
    if pool.tag(concrete_fn_ty) != Tag::Function {
        return;
    }
    let concrete_params = pool.function_params(concrete_fn_ty);
    let num_captures = lambda.params.len().saturating_sub(concrete_params.len());

    // Build Idx→Idx substitution map from schema params to concrete params.
    let mut idx_subst: rustc_hash::FxHashMap<Idx, Idx> = rustc_hash::FxHashMap::default();

    for (i, &cp) in concrete_params.iter().enumerate() {
        let li = num_captures + i;
        if li < lambda.params.len() {
            let schema_ty = lambda.params[li].ty;
            let resolved = pool.resolve_fully(cp);
            let schema_is_generic =
                contains_var(pool, schema_ty) || contains_bound_var(pool, schema_ty);
            let concrete_is_generic =
                contains_var(pool, resolved) || contains_bound_var(pool, resolved);
            if schema_is_generic && !concrete_is_generic {
                idx_subst.insert(schema_ty, resolved);
            }
        }
    }

    apply_exact_type_map(lambda, &idx_subst, resolution);
}

/// Apply concrete types extracted from a call site directly to a lambda's
/// params, `var_types`, and return type.
pub(super) fn apply_call_site_types(
    lambda: &mut crate::ArcFunction,
    arg_types: &[Idx],
    result_ty: Idx,
    resolution: &mut TypeResolution<'_>,
) {
    let pool = resolution.pool();
    let num_captures = lambda.params.len().saturating_sub(arg_types.len());
    let mut idx_subst: rustc_hash::FxHashMap<Idx, Idx> = rustc_hash::FxHashMap::default();

    for (i, &concrete_ty) in arg_types.iter().enumerate() {
        let li = num_captures + i;
        if li < lambda.params.len() {
            let schema_ty = lambda.params[li].ty;
            // Only substitute if the schema type actually contains unresolved vars.
            // Don't replace concrete types that happen to differ (e.g., capture types).
            if schema_ty != concrete_ty
                && (contains_var(pool, schema_ty) || contains_bound_var(pool, schema_ty))
                && !contains_var(pool, concrete_ty)
                && !contains_bound_var(pool, concrete_ty)
            {
                idx_subst.insert(schema_ty, concrete_ty);
            }
        }
    }

    // Substitute return type if it contains vars or is a Scheme.
    let schema_ret = lambda.return_type;
    let ret_is_generic = contains_var(pool, schema_ret)
        || matches!(pool.tag(schema_ret), Tag::Scheme | Tag::Var | Tag::BoundVar);
    if ret_is_generic && !contains_var(pool, result_ty) && !contains_bound_var(pool, result_ty) {
        idx_subst.insert(schema_ret, result_ty);
    }

    apply_exact_type_map(lambda, &idx_subst, resolution);
}

/// Resolve a lambda's return type at every shared ARC type position.
pub(super) fn resolve_lambda_return_types(
    lambda: &mut crate::ArcFunction,
    schema_ret: Idx,
    concrete_ret: Idx,
) {
    let substitutions = rustc_hash::FxHashMap::from_iter([(schema_ret, concrete_ret)]);
    crate::ir::validate::rewrite_type_sites(lambda, |ty, _| {
        substitutions.get(&ty).copied().unwrap_or(ty)
    });
}

/// Rewrite the exact parent closure site selected for a single instantiation.
///
/// This is deliberately keyed by callee identity rather than by type index:
/// distinct polymorphic lambdas may share one schema index but select different
/// concrete instantiations.
pub(super) fn apply_parent_partial_apply_type(
    parent: &mut crate::ArcFunction,
    lambda_name: Name,
    concrete_fn_ty: Idx,
) {
    let (blocks, var_types) = (&mut parent.blocks, &mut parent.var_types);
    for block in blocks {
        for instruction in &mut block.body {
            if let crate::ir::ArcInstr::PartialApply { dst, ty, func, .. } = instruction {
                if *func == lambda_name {
                    *ty = concrete_fn_ty;
                    var_types[dst.index()] = concrete_fn_ty;
                }
            }
        }
    }
}

/// Build a `BoundVar` → concrete type mapping.
pub(super) fn build_bound_var_map(
    pool: &ori_types::Pool,
    concrete_fn_ty: Idx,
    lambda_params: &[crate::ir::ArcParam],
    lambda_return_type: Idx,
    map: &mut rustc_hash::FxHashMap<u32, Idx>,
) {
    if pool.tag(concrete_fn_ty) != Tag::Function {
        return;
    }

    let concrete_params = pool.function_params(concrete_fn_ty);
    let concrete_ret = pool.function_return(concrete_fn_ty);

    let num_captures = lambda_params.len().saturating_sub(concrete_params.len());

    for (i, concrete_ty) in concrete_params.iter().enumerate() {
        let lambda_idx = num_captures + i;
        if lambda_idx < lambda_params.len() {
            let param_ty = lambda_params[lambda_idx].ty;
            if matches!(pool.tag(param_ty), Tag::BoundVar | Tag::Var) {
                let var_id = pool.data(param_ty);
                // The concrete function type is the callable-signature owner.
                // Preserve its exact component identity: resolving a nominal
                // type here replaces that identity with its layout body even
                // though both denote the same structural type.
                map.insert(var_id, *concrete_ty);
            } else if contains_var(pool, param_ty) || contains_bound_var(pool, param_ty) {
                // Container type with nested vars (e.g., List<Var>, Option<Var>).
                // Walk schema and concrete types in parallel to extract var mappings.
                map_types_structural(pool, param_ty, *concrete_ty, map);
            }
        }
    }

    let schema_ret = if pool.tag(lambda_return_type) == Tag::Scheme {
        pool.scheme_body(lambda_return_type)
    } else {
        lambda_return_type
    };
    if contains_bound_var(pool, schema_ret) {
        map_types_structural(pool, schema_ret, concrete_ret, map);
    }
}

/// Check if a concrete function type's param count is compatible with a lambda.
///
/// The lambda's `params` includes captures (env pointer, captured values) followed
/// by the user-visible params. A concrete function type only has user-visible params.
/// So `concrete_param_count <= lambda_param_count` must hold.
fn arity_compatible(pool: &ori_types::Pool, fn_ty: Idx, lambda_param_count: usize) -> bool {
    pool.function_params(fn_ty).len() <= lambda_param_count
}

/// Find a concrete Function type from a Let copy of a specific dst, with arity check.
fn find_concrete_copy_of(
    func: &crate::ArcFunction,
    pa_dst: crate::ir::ArcVarId,
    pool: &ori_types::Pool,
    lambda_param_count: usize,
) -> Option<Idx> {
    for block in &func.blocks {
        for instr in &block.body {
            if let crate::ir::ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src == pa_dst {
                    let ty = func.var_type(*dst);
                    let resolved = pool.resolve_fully(ty);
                    if is_concrete_function(pool, resolved)
                        && arity_compatible(pool, resolved, lambda_param_count)
                    {
                        return Some(resolved);
                    }
                }
            }
        }
    }
    None
}
