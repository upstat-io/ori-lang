//! Result/Option constructor and control-flow expression inference.

use ori_ir::{ExprArena, ExprId, Name, Span};

use super::super::InferEngine;
use super::infer_expr;
use crate::{Idx, Tag, TypeCheckError};

/// Infer the type of `Ok(value)`.
pub(crate) fn infer_ok(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ExprId,
    _span: Span,
) -> Idx {
    let ok_ty = if inner.is_present() {
        infer_expr(engine, arena, inner)
    } else {
        Idx::UNIT
    };
    let err_ty = engine.fresh_var();
    engine.infer_result(ok_ty, err_ty)
}

/// Infer the type of `Err(value)`.
pub(crate) fn infer_err(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ExprId,
    _span: Span,
) -> Idx {
    let err_ty = if inner.is_present() {
        infer_expr(engine, arena, inner)
    } else {
        Idx::UNIT
    };
    let ok_ty = engine.fresh_var();
    engine.infer_result(ok_ty, err_ty)
}

/// Infer the type of `Some(value)`.
pub(crate) fn infer_some(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ExprId,
    _span: Span,
) -> Idx {
    let inner_ty = infer_expr(engine, arena, inner);
    engine.infer_option(inner_ty)
}

/// Infer the type of `None`.
pub(crate) fn infer_none(engine: &mut InferEngine<'_>) -> Idx {
    let inner_ty = engine.fresh_var();
    engine.infer_option(inner_ty)
}

/// Infer the type of the `?` (try) operator.
pub(crate) fn infer_try(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ExprId,
    span: Span,
) -> Idx {
    let inner_ty = infer_expr(engine, arena, inner);
    let resolved = engine.resolve(inner_ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Option => {
            // Option<T>? -> T (propagates None)
            engine.pool().option_inner(resolved)
        }
        Tag::Result => {
            // Result<T, E>? -> T (propagates Err)
            engine.pool().result_ok(resolved)
        }
        _ => {
            engine.push_error(TypeCheckError::try_requires_option_or_result(
                span, resolved,
            ));
            Idx::ERROR
        }
    }
}

/// Infer the type of an await expression.
pub(crate) fn infer_await(
    _engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _inner: ExprId,
    _span: Span,
) -> Idx {
    // TODO: Implement await inference
    Idx::ERROR
}

/// Infer the type of a `with capability = provider in body` expression.
pub(crate) fn infer_with_capability(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    capability: Name,
    provider: ExprId,
    body: ExprId,
    _span: Span,
) -> Idx {
    // Infer provider type (validates the provider expression)
    let provider_ty = infer_expr(engine, arena, provider);

    // Bind the capability name in a child scope so the body can
    // reference it as an identifier (e.g., `with Http = mock in Http`).
    engine.enter_scope();
    engine.env_mut().bind(capability, provider_ty);

    // Provide the capability for the duration of the body.
    // This makes calls to functions `uses <capability>` valid within.
    let body_ty =
        engine.with_provided_capability(capability, |engine| infer_expr(engine, arena, body));

    engine.exit_scope();
    body_ty
}
