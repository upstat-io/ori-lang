//! Core function call inference (positional and named arguments).

use ori_ir::{ExprArena, ExprId, ExprKind, Span};

use super::super::super::InferEngine;
use super::super::infer_expr;
use super::constraints::{check_call_capabilities, check_where_clauses};
use super::monomorphization::maybe_record_mono_instance;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError};

/// Infer the type of a function call expression.
///
/// `call_expr_id` is the AST `ExprId` of the call expression itself (the
/// parent of `func`); used by `maybe_record_mono_instance` to publish a
/// dispatch entry into `TypedModule.mono_dispatch_map`.
pub(crate) fn infer_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    call_expr_id: ExprId,
    func: ExprId,
    args: ori_ir::ExprRange,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let arg_ids = arena.get_expr_list(args);

    // Extract function name for signature lookup
    let func_name_id = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(name) | ExprKind::Ident(name) => Some(*name),
        _ => None,
    };

    // Look up required_params from function signature if available
    let required_params = func_name_id
        .and_then(|n| engine.get_signature(n))
        .map_or(params.len(), |sig| sig.required_params);

    // Check arity: allow fewer args if defaults fill the gap
    if arg_ids.len() < required_params || arg_ids.len() > params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            params.len(),
            arg_ids.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Idx::ERROR;
    }

    // Validate capability requirements
    check_call_capabilities(engine, func_name_id, span);

    // Check each provided argument
    for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
                kind: ContextKind::FunctionArgument {
                    func_name: None,
                    arg_index: i,
                    param_name: None,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg_id);
        let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg_id).span);
    }

    // Record monomorphization instance for generic function calls.
    // At this point type variables have been unified with concrete types.
    maybe_record_mono_instance(engine, call_expr_id, func_name_id, &params);

    resolve_return_projection(engine, func_name_id, &params, ret)
}

/// Infer the type of a named-argument function call.
///
/// `call_expr_id` is the AST `ExprId` of the call expression itself (the
/// parent of `func`); used by `maybe_record_mono_instance` to publish a
/// dispatch entry into `TypedModule.mono_dispatch_map`.
pub(crate) fn infer_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    call_expr_id: ExprId,
    func: ExprId,
    args: ori_ir::CallArgRange,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let call_args = arena.get_call_args(args);

    // Extract function name for error messages and signature lookup
    let func_name_id = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(name) | ExprKind::Ident(name) => Some(*name),
        _ => None,
    };

    // Look up required_params from function signature if available
    let required_params = func_name_id
        .and_then(|n| engine.get_signature(n))
        .map_or(params.len(), |sig| sig.required_params);

    // Check arity: allow fewer args if defaults fill the gap
    if call_args.len() < required_params || call_args.len() > params.len() {
        // Allocate func name string only on the error path
        let func_name = func_name_id.and_then(|n| engine.lookup_name(n).map(String::from));
        if let Some(name) = func_name {
            engine.push_error(TypeCheckError::arity_mismatch_named(
                span,
                name,
                params.len(),
                call_args.len(),
            ));
        } else {
            engine.push_error(TypeCheckError::arity_mismatch(
                span,
                params.len(),
                call_args.len(),
                crate::ArityMismatchKind::Function,
            ));
        }
        return Idx::ERROR;
    }

    // Validate capability requirements
    check_call_capabilities(engine, func_name_id, span);

    // Check each argument type by position
    for (i, (arg, &param_ty)) in call_args.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
                kind: ContextKind::FunctionArgument {
                    func_name: func_name_id,
                    arg_index: i,
                    param_name: arg.name,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg.value);
        let _ = engine.check_type(arg_ty, &expected, arg.span);
    }

    // Record monomorphization instance for generic function calls.
    maybe_record_mono_instance(engine, call_expr_id, func_name_id, &params);

    // Validate where-clause constraints after argument type-checking.
    // At this point, generic type variables have been unified with concrete types.
    if let Some(func_name) = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(n) | ExprKind::Ident(n) => Some(*n),
        _ => None,
    } {
        check_where_clauses(engine, func_name, &params, span);
    }

    resolve_return_projection(engine, func_name_id, &params, ret)
}

/// Project a generic function's associated-type return (`-> C.Item`) to the
/// concrete result type once the call's arguments have bound the base
/// type-param to a concrete receiver.
///
/// When the function's signature carries a `return_projection: (base_param,
/// assoc_name)` and the declared `ret` is `Idx::ERROR` (symbolic poison at
/// signature time), resolve the concrete type the base type-param is bound to
/// at this call site (via the param it directly types) and project the impl's
/// `type <assoc_name> = …` binding. Falls back to `ret` (poison) for a symbolic
/// receiver or a missing binding — the symbolic-poison guard for a generic
/// receiver that cannot resolve.
///
/// Shared SSOT for return-projection resolution: the call-site inference path
/// uses it to type the call expression, and the monomorphization-recording path
/// (`monomorphization::maybe_record_mono_instance`) uses it to hoist the
/// concrete return type BEFORE recording the `MonoInstance`, so the recorded
/// mono return is the projected concrete type, not the poison `Idx::ERROR`.
pub(super) fn resolve_return_projection(
    engine: &mut InferEngine<'_>,
    func_name_id: Option<ori_ir::Name>,
    instantiated_params: &[Idx],
    ret: Idx,
) -> Idx {
    let Some(name) = func_name_id else {
        return ret;
    };
    // Snapshot the projection + base-param index + the base-param's first trait
    // bound from the signature (immutable borrow) before resolving against the
    // pool / registry (mutable borrow). The bound trait (`C: Container`)
    // disambiguates the projection by `(trait_idx, base_ty, assoc_name)` when
    // the concrete receiver implements two traits with a same-named associated
    // type.
    let Some((assoc_name, base_param_index, bound_trait)) =
        engine.get_signature(name).and_then(|sig| {
            let (base_param, assoc_name) = sig.return_projection?;
            let tp_index = sig.type_params.iter().position(|&n| n == base_param)?;
            let param_index = (*sig.generic_param_mapping.get(tp_index)?)?;
            let bound_trait = sig
                .type_param_bounds
                .get(tp_index)
                .and_then(|bounds| bounds.first().copied());
            Some((assoc_name, param_index, bound_trait))
        })
    else {
        return ret;
    };

    // Resolve the concrete type the base type-param is bound to at this call.
    let Some(&param_ty) = instantiated_params.get(base_param_index) else {
        return ret;
    };
    let base_ty = engine.resolve(param_ty);
    if base_ty == Idx::ERROR || engine.pool().tag(base_ty).is_type_variable() {
        return ret;
    }

    let projected = engine.trait_registry().and_then(|reg| {
        // Resolve the bound trait Name to its pool Idx (when present) so the
        // lookup keys on `(trait_idx, base_ty, assoc_name)`; an absent or
        // unknown bound keeps the trait-blind first-match fallback.
        let trait_idx = bound_trait.and_then(|t| reg.get_trait_by_name(t).map(|e| e.idx));
        reg.find_impl_assoc_binding(trait_idx, base_ty, assoc_name)
    });
    projected.unwrap_or(ret)
}
