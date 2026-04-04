//! Type resolution helpers for lambda monomorphization.
//!
//! Functions for finding concrete types from parent ARC IR, building `BoundVar`
//! mappings, applying substitutions, and type predicate checks.

use ori_ir::Name;
use ori_types::Idx;

/// Search parent + all sibling lambdas for a `PartialApply` that references the
/// given lambda, and return the concrete instantiated function type.
pub(super) fn find_partial_apply_concrete_type(
    parent: &ori_arc::ArcFunction,
    lambdas: &[ori_arc::ArcFunction],
    skip_idx: usize,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    use ori_types::Tag;

    let check_concrete =
        |func: &ori_arc::ArcFunction, dst: &ori_arc::ir::ArcVarId| -> Option<Idx> {
            let pa_ty = func.var_type(*dst);
            let resolved = pool.resolve_fully(pa_ty);
            if pool.tag(resolved) == Tag::Function {
                let params = pool.function_params(resolved);
                let all_concrete = params.iter().all(|p| {
                    let pt = pool.resolve_fully(*p);
                    !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
                });
                if all_concrete {
                    return Some(resolved);
                }
            }
            None
        };

    let find_pa = |func: &ori_arc::ArcFunction| -> Option<ori_arc::ir::ArcVarId> {
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
    };

    // Search parent first.
    if let Some(dst) = find_pa(parent) {
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
        if let Some(dst) = find_pa(sibling) {
            if let Some(ty) = check_concrete(sibling, &dst) {
                return Some(ty);
            }
            if let Some(ty) = find_concrete_copy_of(sibling, dst, pool) {
                return Some(ty);
            }
            if let Some(parent_dst) = find_pa(parent) {
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
    use ori_types::Tag;

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
    use ori_types::Tag;

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
    use ori_types::Tag;

    let mut pa_dst = None;
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == lambda_name {
                    pa_dst = Some(*dst);
                    break;
                }
            }
        }
        if pa_dst.is_some() {
            break;
        }
    }

    let Some(pa_dst) = pa_dst else {
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
                    if pool.tag(resolved) == Tag::Function {
                        let params = pool.function_params(resolved);
                        let ret = pool.function_return(resolved);
                        let all_concrete = params.iter().chain(std::iter::once(&ret)).all(|p| {
                            let pt = pool.resolve_fully(*p);
                            !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
                        });
                        if all_concrete {
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
    use ori_types::Tag;

    let mut pa_dst = None;
    for block in &parent.blocks {
        for instr in &block.body {
            if let ori_arc::ir::ArcInstr::PartialApply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == lambda_name {
                    pa_dst = Some(*dst);
                    break;
                }
            }
        }
        if pa_dst.is_some() {
            break;
        }
    }
    let pa_dst = pa_dst?;

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
    use ori_types::Tag;

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

/// Check if a type contains a `Var` INSIDE a container (not at the top level).
pub(super) fn contains_nested_var(pool: &ori_types::Pool, ty: Idx) -> bool {
    use ori_types::Tag;
    match pool.tag(ty) {
        Tag::Option => contains_var(pool, pool.option_inner(ty)),
        Tag::Result => {
            contains_var(pool, pool.result_ok(ty)) || contains_var(pool, pool.result_err(ty))
        }
        Tag::List => contains_var(pool, pool.list_elem(ty)),
        _ => false,
    }
}

/// Check if a type contains a `Var` at any nesting level.
pub(super) fn contains_var(pool: &ori_types::Pool, ty: Idx) -> bool {
    use ori_types::Tag;
    match pool.tag(ty) {
        Tag::Var => true,
        Tag::Option => contains_var(pool, pool.option_inner(ty)),
        Tag::Result => {
            contains_var(pool, pool.result_ok(ty)) || contains_var(pool, pool.result_err(ty))
        }
        Tag::List => contains_var(pool, pool.list_elem(ty)),
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
    use ori_types::Tag;

    let resolved = pool.resolve_fully(ty);
    match pool.tag(resolved) {
        Tag::BoundVar | Tag::Scheme => true,
        Tag::Option => contains_bound_var(pool, pool.option_inner(resolved)),
        Tag::Result => {
            contains_bound_var(pool, pool.result_ok(resolved))
                || contains_bound_var(pool, pool.result_err(resolved))
        }
        Tag::List => contains_bound_var(pool, pool.list_elem(resolved)),
        Tag::Function => {
            pool.function_params(resolved)
                .iter()
                .any(|p| contains_bound_var(pool, *p))
                || contains_bound_var(pool, pool.function_return(resolved))
        }
        _ => false,
    }
}

// Internal helpers

/// Scan all `var_types` for the first concrete Function type.
fn find_any_concrete_fn_type(func: &ori_arc::ArcFunction, pool: &ori_types::Pool) -> Option<Idx> {
    use ori_types::Tag;
    for ty in &func.var_types {
        let resolved = pool.resolve_fully(*ty);
        if pool.tag(resolved) == Tag::Function {
            let params = pool.function_params(resolved);
            let all_concrete = params.iter().all(|p| {
                let pt = pool.resolve_fully(*p);
                !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
            });
            if all_concrete {
                return Some(resolved);
            }
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
    use ori_types::Tag;
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
                    if pool.tag(resolved) == Tag::Function {
                        let params = pool.function_params(resolved);
                        let all_concrete = params.iter().all(|p| {
                            let pt = pool.resolve_fully(*p);
                            !matches!(pool.tag(pt), Tag::BoundVar | Tag::Var | Tag::Scheme)
                        });
                        if all_concrete {
                            return Some(resolved);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Walk `schema_ty` and `concrete_ty` in parallel to build `BoundVar` mappings.
fn map_types_structural(
    pool: &ori_types::Pool,
    schema_ty: Idx,
    concrete_ty: Idx,
    map: &mut rustc_hash::FxHashMap<u32, Idx>,
) {
    use ori_types::Tag;

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
