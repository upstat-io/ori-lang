//! Closure-site queries used by lambda type resolution.

use ori_ir::Name;
use ori_types::{Idx, Tag};

use super::{contains_var, is_concrete_function};

/// Find the `PartialApply` destination for a lambda.
pub(in crate::lambda_specialization) fn find_partial_apply_dst(
    function: &crate::ArcFunction,
    lambda_name: Name,
) -> Option<crate::ir::ArcVarId> {
    for block in &function.blocks {
        for instruction in &block.body {
            if let crate::ir::ArcInstr::PartialApply {
                dst, func: callee, ..
            } = instruction
            {
                if *callee == lambda_name {
                    return Some(*dst);
                }
            }
        }
    }
    None
}

/// Extract concrete parameter and result types from an indirect call site.
pub(in crate::lambda_specialization) fn find_concrete_types_from_calls(
    parent: &crate::ArcFunction,
    partial_apply_dst: crate::ir::ArcVarId,
    pool: &ori_types::Pool,
) -> Option<(Vec<Idx>, Idx)> {
    let mut closure_vars = vec![partial_apply_dst];
    for block in &parent.blocks {
        for instruction in &block.body {
            if let crate::ir::ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::Var(source),
                ..
            } = instruction
            {
                if *source == partial_apply_dst {
                    closure_vars.push(*dst);
                }
            }
        }
    }

    for block in &parent.blocks {
        for instruction in &block.body {
            if let crate::ir::ArcInstr::ApplyIndirect {
                dst, closure, args, ..
            } = instruction
            {
                if closure_vars.contains(closure) {
                    let arg_types: Vec<Idx> = args
                        .iter()
                        .map(|arg| pool.resolve_fully(parent.var_type(*arg)))
                        .collect();
                    let raw_result_ty = pool.resolve_fully(parent.var_type(*dst));
                    let result_ty = if contains_var(pool, raw_result_ty)
                        || matches!(pool.tag(raw_result_ty), Tag::Scheme)
                    {
                        find_narrowed_result_type(parent, *dst, pool).unwrap_or(raw_result_ty)
                    } else {
                        raw_result_ty
                    };

                    if arg_types.iter().all(|ty| !contains_var(pool, *ty)) {
                        return Some((arg_types, result_ty));
                    }
                }
            }
        }
    }
    None
}

fn find_narrowed_result_type(
    function: &crate::ArcFunction,
    var: crate::ir::ArcVarId,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    for block in &function.blocks {
        for instruction in &block.body {
            if let crate::ir::ArcInstr::Let {
                value: crate::ir::ArcValue::Var(source),
                ty,
                ..
            } = instruction
            {
                if *source == var {
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

/// Find every distinct concrete function type selected for a lambda.
pub(in crate::lambda_specialization) fn find_all_instantiation_types(
    parent: &crate::ArcFunction,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Vec<Idx> {
    let Some(partial_apply_dst) = find_partial_apply_dst(parent, lambda_name) else {
        return Vec::new();
    };

    let mut instantiations = Vec::new();
    let mut seen = rustc_hash::FxHashSet::<Vec<Idx>>::default();

    for block in &parent.blocks {
        for instruction in &block.body {
            if let crate::ir::ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::Var(source),
                ..
            } = instruction
            {
                if *source == partial_apply_dst {
                    let resolved = pool.resolve_fully(parent.var_type(*dst));
                    if is_concrete_function(pool, resolved) {
                        let params = pool.function_params(resolved);
                        let ret = pool.function_return(resolved);
                        let key: Vec<Idx> = params
                            .iter()
                            .chain(std::iter::once(&ret))
                            .map(|component| pool.resolve_fully(*component))
                            .collect();
                        if seen.insert(key) {
                            instantiations.push(resolved);
                        }
                    } else if super::has_concrete_params(pool, resolved) {
                        let params = pool.function_params(resolved);
                        let key: Vec<Idx> = params
                            .iter()
                            .map(|component| pool.resolve_fully(*component))
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

/// Find the capture arguments from a lambda's `PartialApply` instruction.
pub(in crate::lambda_specialization) fn find_partial_apply_args(
    parent: &crate::ArcFunction,
    lambda_name: Name,
) -> Vec<crate::ir::ArcVarId> {
    for block in &parent.blocks {
        for instruction in &block.body {
            if let crate::ir::ArcInstr::PartialApply {
                func: callee, args, ..
            } = instruction
            {
                if *callee == lambda_name {
                    return args.clone();
                }
            }
        }
    }
    Vec::new()
}

/// Find the concrete return type selected by an indirect lambda call.
pub(in crate::lambda_specialization) fn find_apply_indirect_result_type(
    parent: &crate::ArcFunction,
    lambda_name: Name,
    pool: &ori_types::Pool,
) -> Option<Idx> {
    let partial_apply_dst = find_partial_apply_dst(parent, lambda_name)?;

    let mut narrowing_vars = Vec::new();
    for block in &parent.blocks {
        for instruction in &block.body {
            if let crate::ir::ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::Var(source),
                ..
            } = instruction
            {
                if *source == partial_apply_dst {
                    narrowing_vars.push(*dst);
                }
            }
        }
    }

    for block in &parent.blocks {
        for instruction in &block.body {
            if let crate::ir::ArcInstr::ApplyIndirect { dst, closure, .. } = instruction {
                if narrowing_vars.contains(closure) {
                    let resolved = pool.resolve_fully(parent.var_type(*dst));
                    if !matches!(pool.tag(resolved), Tag::BoundVar | Tag::Var | Tag::Scheme) {
                        return Some(resolved);
                    }
                }
            }
        }
    }

    None
}
