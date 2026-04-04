//! Type resolution helpers for lambda monomorphization.
//!
//! Functions for finding concrete types from parent ARC IR, building `BoundVar`
//! mappings, applying substitutions, and type predicate checks.

use ori_ir::Name;
use ori_types::Idx;
use ori_types::Tag;

use super::type_predicates::{contains_bound_var, contains_nested_var, map_types_structural};

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
pub(super) fn is_polymorphic_lambda(lambda: &ori_arc::ArcFunction, pool: &ori_types::Pool) -> bool {
    lambda
        .params
        .iter()
        .any(|p| matches!(pool.tag(p.ty), Tag::BoundVar | Tag::Var))
        || contains_bound_var(pool, lambda.return_type)
        || contains_nested_var(pool, lambda.return_type)
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

/// Fall back: any remaining `BoundVar`s/`Var`s → `Idx::INT`.
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
            if matches!(pool.tag(param_ty), Tag::BoundVar | Tag::Var) {
                let var_id = pool.data(param_ty);
                let resolved_concrete = pool.resolve_fully(*concrete_ty);
                map.insert(var_id, resolved_concrete);
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
