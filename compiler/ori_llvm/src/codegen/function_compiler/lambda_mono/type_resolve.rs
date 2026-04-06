//! Type resolution helpers for lambda monomorphization.
//!
//! Functions for finding concrete types from parent ARC IR, building `BoundVar`
//! mappings, applying substitutions, and type predicate checks.

use ori_ir::Name;
use ori_types::Idx;
use ori_types::Tag;

use super::type_predicates::{
    contains_bound_var, contains_var, has_concrete_params, map_types_structural,
};

/// Find the `PartialApply` dst variable for a lambda in a function's blocks.
pub(super) fn find_partial_apply_dst(
    func: &ori_arc::ArcFunction,
    lambda_name: Name,
) -> Option<ori_arc::ir::ArcVarId> {
    for block in &func.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == lambda_name {
                    return Some(*dst);
                }
            }
        }
    }
    None
}

/// Check if a resolved type is a Function with all-concrete params and return type.
pub(super) fn is_concrete_function(pool: &ori_types::Pool, resolved: Idx) -> bool {
    if pool.tag(resolved) != Tag::Function {
        return false;
    }
    let params = pool.function_params(resolved);
    let ret = pool.function_return(resolved);
    params.iter().chain(std::iter::once(&ret)).all(|p| {
        let pt = pool.resolve_fully(*p);
        !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
    })
}

/// Check if a lambda has any unresolved polymorphic types (`BoundVar`/`Var` in
/// params, return, or `var_types`).
///
/// Checks both top-level vars AND vars nested inside container types
/// (e.g., `List<Var>`, `Option<Var>`). Generalized vars (`VarState::Generalized`)
/// from let-polymorphism have `Tag::Var`, so we use `contains_var` (which
/// detects `Tag::Var` at any depth) in addition to `contains_bound_var`
/// (which only detects `Tag::BoundVar`/`Tag::Scheme`).
pub(super) fn is_polymorphic_lambda(lambda: &ori_arc::ArcFunction, pool: &ori_types::Pool) -> bool {
    // Params: check both top-level and nested vars (e.g., List<Var>).
    // The nested check is essential for Generalized vars in container types
    // that need resolution via map_types_structural or call-site extraction.
    lambda.params.iter().any(|p| {
        matches!(pool.tag(p.ty), Tag::BoundVar | Tag::Var) || contains_var(pool, p.ty)
    })
    // Return type: only check BoundVar/Scheme (original behavior).
    // Do NOT add contains_var here — iterator callback lambdas (e.g., `s -> s`
    // in `.map(transform: s -> s)`) have Generalized Var return types that are
    // correctly handled by the existing resolve_lambda_return_types +
    // find_apply_indirect_result_type mechanism without entering the mono pipeline.
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
    parent: &ori_arc::ArcFunction,
    lambdas: &[ori_arc::ArcFunction],
    skip_idx: usize,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    let check_concrete =
        |func: &ori_arc::ArcFunction, dst: &ori_arc::ir::ArcVarId| -> Option<Idx> {
            let pa_ty = func.var_type(*dst);
            let resolved = pool.resolve_fully(pa_ty);
            if is_concrete_function(pool, resolved) {
                return Some(resolved);
            }
            None
        };

    // Search parent first.
    if let Some(dst) = find_partial_apply_dst(parent, lambda_name) {
        if let Some(ty) = check_concrete(parent, &dst) {
            return Some(ty);
        }
        if let Some(ty) = find_concrete_copy_of(parent, dst, pool) {
            return Some(ty);
        }
        if let Some(ty) = find_any_concrete_fn_type(parent, pool) {
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
            if let Some(ty) = find_concrete_copy_of(sibling, dst, pool) {
                return Some(ty);
            }
            if let Some(parent_dst) = find_partial_apply_dst(parent, lambda_name) {
                if let Some(ty) = find_concrete_copy_of(parent, parent_dst, pool) {
                    return Some(ty);
                }
            }
            if let Some(ty) = find_any_concrete_fn_type(sibling, pool) {
                return Some(ty);
            }
            if let Some(ty) = find_any_concrete_fn_type(parent, pool) {
                return Some(ty);
            }
        }
    }

    None
}

/// Apply a `BoundVar` → concrete mapping to a lambda's types.
pub(super) fn apply_bound_var_map(
    lambda: &mut ori_arc::ArcFunction,
    map: &rustc_hash::FxHashMap<u32, Idx>,
    pool: &ori_types::Pool,
) {
    if map.is_empty() {
        return;
    }

    for param in &mut lambda.params {
        if matches!(pool.tag(param.ty), Tag::BoundVar | Tag::Var) {
            let var_id = pool.data(param.ty);
            if let Some(&concrete) = map.get(&var_id) {
                param.ty = concrete;
            }
        }
    }

    for ty in &mut lambda.var_types {
        if matches!(pool.tag(*ty), Tag::BoundVar | Tag::Var) {
            let var_id = pool.data(*ty);
            if let Some(&concrete) = map.get(&var_id) {
                *ty = concrete;
            }
        }
    }

    if matches!(pool.tag(lambda.return_type), Tag::BoundVar | Tag::Var) {
        let var_id = pool.data(lambda.return_type);
        if let Some(&concrete) = map.get(&var_id) {
            lambda.return_type = concrete;
        }
    }
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
    lambda: &mut ori_arc::ArcFunction,
    concrete_fn_ty: Idx,
    pool: &ori_types::Pool,
) {
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
            if contains_var(pool, schema_ty) && !contains_var(pool, resolved) {
                idx_subst.insert(schema_ty, resolved);
                lambda.params[li].ty = resolved;
            }
        }
    }

    if idx_subst.is_empty() {
        return;
    }

    // Apply the substitution to var_types.
    for ty in &mut lambda.var_types {
        if let Some(&concrete) = idx_subst.get(ty) {
            *ty = concrete;
        }
    }

    // Apply to Construct instruction types.
    for block in &mut lambda.blocks {
        for instr in &mut block.body {
            if let ori_arc::ir::ArcInstr::Construct { ty, .. } = instr {
                if let Some(&concrete) = idx_subst.get(ty) {
                    *ty = concrete;
                }
            }
        }
    }
}

/// Extract concrete param types from `ApplyIndirect` call sites in the parent.
///
/// For let-polymorphic lambdas, the call chain is:
///   `PartialApply` → Let copy (narrows type) → `ApplyIndirect` (uses copy)
/// This function follows that chain: first collects all variables that copy
/// the `PartialApply` result, then finds `ApplyIndirect` calls using those copies.
///
/// Returns `Some((arg_types, result_type))` if a concrete call site is found.
pub(super) fn find_concrete_types_from_calls(
    parent: &ori_arc::ArcFunction,
    pa_dst: ori_arc::ir::ArcVarId,
    pool: &ori_types::Pool,
) -> Option<(Vec<Idx>, Idx)> {
    // Collect all variables that are copies of the PartialApply result.
    // Pattern: `Let { dst, value: Var(pa_dst), .. }`
    let mut closure_vars: Vec<ori_arc::ir::ArcVarId> = vec![pa_dst];
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src == pa_dst {
                    closure_vars.push(*dst);
                }
            }
        }
    }

    // Find the first ApplyIndirect that uses any closure variable.
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::ApplyIndirect {
                dst, closure, args, ..
            } = instr
            {
                if closure_vars.contains(closure) {
                    let arg_types: Vec<Idx> = args
                        .iter()
                        .map(|a| {
                            let ty = parent.var_type(*a);
                            pool.resolve_fully(ty)
                        })
                        .collect();
                    // For the result type, prefer the resolved var_type of the
                    // ApplyIndirect result. If it still contains vars (common for
                    // Scheme return types), try to find a concrete downstream use.
                    let raw_result_ty = pool.resolve_fully(parent.var_type(*dst));
                    let result_ty = if contains_var(pool, raw_result_ty)
                        || matches!(pool.tag(raw_result_ty), Tag::Scheme)
                    {
                        // Look for a Let copy that narrows the result type.
                        find_narrowed_result_type(parent, *dst, pool).unwrap_or(raw_result_ty)
                    } else {
                        raw_result_ty
                    };

                    // Only use this call site if ALL arg types are concrete.
                    if arg_types.iter().all(|t| !contains_var(pool, *t)) {
                        return Some((arg_types, result_ty));
                    }
                }
            }
        }
    }
    None
}

/// Find a narrowed (concrete) type for a variable by looking at Let copies.
fn find_narrowed_result_type(
    func: &ori_arc::ArcFunction,
    var: ori_arc::ir::ArcVarId,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    for block in &func.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                value: ori_arc::ir::ArcValue::Var(src),
                ty,
                ..
            } = instr
            {
                if *src == var {
                    let resolved = pool.resolve_fully(*ty);
                    if !contains_var(pool, resolved) && !matches!(pool.tag(resolved), Tag::Scheme) {
                        return Some(resolved);
                    }
                }
            }
        }
    }
    None
}

/// Apply concrete types extracted from a call site directly to a lambda's
/// params, `var_types`, and return type.
pub(super) fn apply_call_site_types(
    lambda: &mut ori_arc::ArcFunction,
    arg_types: &[Idx],
    result_ty: Idx,
    pool: &ori_types::Pool,
) {
    let num_captures = lambda.params.len().saturating_sub(arg_types.len());
    let mut idx_subst: rustc_hash::FxHashMap<Idx, Idx> = rustc_hash::FxHashMap::default();

    for (i, &concrete_ty) in arg_types.iter().enumerate() {
        let li = num_captures + i;
        if li < lambda.params.len() {
            let schema_ty = lambda.params[li].ty;
            // Only substitute if the schema type actually contains unresolved vars.
            // Don't replace concrete types that happen to differ (e.g., capture types).
            if schema_ty != concrete_ty && contains_var(pool, schema_ty) {
                idx_subst.insert(schema_ty, concrete_ty);
                lambda.params[li].ty = concrete_ty;
            }
        }
    }

    // Substitute return type if it contains vars or is a Scheme.
    let schema_ret = lambda.return_type;
    let ret_is_generic = contains_var(pool, schema_ret)
        || matches!(pool.tag(schema_ret), Tag::Scheme | Tag::Var | Tag::BoundVar);
    if ret_is_generic && !contains_var(pool, result_ty) {
        idx_subst.insert(schema_ret, result_ty);
        lambda.return_type = result_ty;
    }

    if idx_subst.is_empty() {
        return;
    }

    // Apply to var_types.
    for ty in &mut lambda.var_types {
        if let Some(&concrete) = idx_subst.get(ty) {
            *ty = concrete;
        }
    }

    // Apply to Construct instruction types.
    for block in &mut lambda.blocks {
        for instr in &mut block.body {
            if let ori_arc::ir::ArcInstr::Construct { ty, .. } = instr {
                if let Some(&concrete) = idx_subst.get(ty) {
                    *ty = concrete;
                }
            }
        }
    }
}

/// Fall back: any remaining `BoundVar`s/`Var`s → `Idx::INT`.
///
/// Only replaces TOP-LEVEL `BoundVar`/`Var` types. Container types with nested
/// vars (e.g., `List<Var>`) are left as-is — they should have been resolved by
/// `apply_concrete_param_types` or `apply_call_site_types` before this runs.
/// Replacing a container type with INT would cause ABI mismatches.
pub(super) fn fallback_bound_vars_to_int(
    lambda: &mut ori_arc::ArcFunction,
    pool: &ori_types::Pool,
) {
    for param in &mut lambda.params {
        if matches!(pool.tag(param.ty), Tag::BoundVar | Tag::Var) {
            param.ty = Idx::INT;
        }
    }
    for ty in &mut lambda.var_types {
        if matches!(pool.tag(*ty), Tag::BoundVar | Tag::Var) {
            *ty = Idx::INT;
        }
    }
    if contains_bound_var(pool, lambda.return_type) {
        lambda.return_type = Idx::INT;
    }
}

/// Resolve a lambda's return type, `var_types`, and `Construct` instruction types
/// from a schema->concrete mapping.
pub(super) fn resolve_lambda_return_types(
    lambda: &mut ori_arc::ArcFunction,
    schema_ret: Idx,
    concrete_ret: Idx,
) {
    lambda.return_type = concrete_ret;
    for ty in &mut lambda.var_types {
        if *ty == schema_ret {
            *ty = concrete_ret;
        }
    }
    for block in &mut lambda.blocks {
        for instr in &mut block.body {
            if let ori_arc::ir::ArcInstr::Construct { ty, .. } = instr {
                if *ty == schema_ret {
                    *ty = concrete_ret;
                }
            }
        }
    }
}

/// Find all distinct concrete Function types that a polymorphic lambda is
/// narrowed to in the parent function's `var_types` (via Let copies).
pub(super) fn find_all_instantiation_types(
    parent: &ori_arc::ArcFunction,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Vec<Idx> {
    let Some(pa_dst) = find_partial_apply_dst(parent, lambda_name) else {
        return Vec::new();
    };

    let mut instantiations: Vec<Idx> = Vec::new();
    let mut seen = rustc_hash::FxHashSet::<Vec<Idx>>::default();

    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src == pa_dst {
                    let ty = parent.var_type(*dst);
                    let resolved = pool.resolve_fully(ty);
                    if is_concrete_function(pool, resolved) {
                        // Fully concrete — dedup by params + return.
                        let params = pool.function_params(resolved);
                        let ret = pool.function_return(resolved);
                        let key: Vec<Idx> = params
                            .iter()
                            .chain(std::iter::once(&ret))
                            .map(|p| pool.resolve_fully(*p))
                            .collect();
                        if seen.insert(key) {
                            instantiations.push(resolved);
                        }
                    } else if has_concrete_params(pool, resolved) {
                        // Params concrete but return is Scheme/Var (let-polymorphic
                        // lambda). Dedup by params only since the return type is the
                        // same Scheme across all copies — params distinguish instances.
                        let params = pool.function_params(resolved);
                        let key: Vec<Idx> = params.iter().map(|p| pool.resolve_fully(*p)).collect();
                        if seen.insert(key) {
                            instantiations.push(resolved);
                        }
                    }
                }
            }
        }
    }

    instantiations
}

/// Find the capture arguments from a `PartialApply` instruction for a lambda.
pub(super) fn find_partial_apply_args(
    parent: &ori_arc::ArcFunction,
    lambda_name: Name,
) -> Vec<ori_arc::ir::ArcVarId> {
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply {
                func: callee, args, ..
            } = instr
            {
                if *callee == lambda_name {
                    return args.clone();
                }
            }
        }
    }
    Vec::new()
}

/// Find the concrete return type by looking at `ApplyIndirect` results.
pub(super) fn find_apply_indirect_result_type(
    parent: &ori_arc::ArcFunction,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    let pa_dst = find_partial_apply_dst(parent, lambda_name)?;

    let mut narrowing_vars = Vec::new();
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src == pa_dst {
                    narrowing_vars.push(*dst);
                }
            }
        }
    }

    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::ApplyIndirect { dst, closure, .. } = instr {
                if narrowing_vars.contains(closure) {
                    let result_ty = parent.var_type(*dst);
                    let resolved = pool.resolve_fully(result_ty);
                    if !matches!(pool.tag(resolved), Tag::BoundVar | Tag::Var | Tag::Scheme) {
                        return Some(resolved);
                    }
                }
            }
        }
    }

    None
}

/// Build a `BoundVar` → concrete type mapping.
pub(super) fn build_bound_var_map(
    pool: &ori_types::Pool,
    concrete_fn_ty: Idx,
    lambda_params: &[ori_arc::ir::ArcParam],
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
            let resolved_concrete = pool.resolve_fully(*concrete_ty);
            if matches!(pool.tag(param_ty), Tag::BoundVar | Tag::Var) {
                let var_id = pool.data(param_ty);
                map.insert(var_id, resolved_concrete);
            } else if contains_var(pool, param_ty) {
                // Container type with nested vars (e.g., List<Var>, Option<Var>).
                // Walk schema and concrete types in parallel to extract var mappings.
                map_types_structural(pool, param_ty, resolved_concrete, map);
            }
        }
    }

    let schema_ret = if pool.tag(lambda_return_type) == Tag::Scheme {
        pool.scheme_body(lambda_return_type)
    } else {
        lambda_return_type
    };
    if contains_bound_var(pool, schema_ret) {
        map_types_structural(pool, schema_ret, pool.resolve_fully(concrete_ret), map);
    }
}
// Internal helpers

/// Scan all `var_types` for the first concrete Function type.
fn find_any_concrete_fn_type(func: &ori_arc::ArcFunction, pool: &ori_types::Pool) -> Option<Idx> {
    for ty in &func.var_types {
        let resolved = pool.resolve_fully(*ty);
        if is_concrete_function(pool, resolved) {
            return Some(resolved);
        }
    }
    None
}

/// Find the first concrete Function type from a Let copy of a specific dst.
fn find_concrete_copy_of(
    func: &ori_arc::ArcFunction,
    pa_dst: ori_arc::ir::ArcVarId,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    for block in &func.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::Let {
                dst,
                value: ori_arc::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                if *src == pa_dst {
                    let ty = func.var_type(*dst);
                    let resolved = pool.resolve_fully(ty);
                    if is_concrete_function(pool, resolved) {
                        return Some(resolved);
                    }
                }
            }
        }
    }
    None
}
