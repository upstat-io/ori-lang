//! Core function call inference (positional and named arguments).

use ori_ir::{ExprArena, ExprId, ExprKind, Span};

use super::super::super::InferEngine;
use super::super::infer_expr;
use super::constraints::{check_call_capabilities, check_where_clauses};
use super::monomorphization::maybe_record_mono_instance;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError};

/// Infer the type of a function call expression.
pub(crate) fn infer_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
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
    maybe_record_mono_instance(engine, func_name_id, &params);

    ret
}

/// Infer the type of a named-argument function call.
pub(crate) fn infer_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
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
    maybe_record_mono_instance(engine, func_name_id, &params);

    // Validate where-clause constraints after argument type-checking.
    // At this point, generic type variables have been unified with concrete types.
    if let Some(func_name) = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(n) | ExprKind::Ident(n) => Some(*n),
        _ => None,
    } {
        check_where_clauses(engine, func_name, &params, span);
    }

    ret
}
