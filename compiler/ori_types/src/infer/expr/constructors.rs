//! Result/Option constructor and control-flow expression inference.

use ori_ir::{ExprArena, ExprId, Name, Span};

use crate::{ContextKind, Expected, Idx, Tag, TypeCheckError};

use super::super::InferEngine;
use super::{check_expr, infer_expr};

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

/// BD-2 check `Ok(value)` against an outer `Result<T, E>` expectation.
///
/// When `expected` resolves to a concrete `Result<T, E>`, the inner
/// expression is checked against `Check(T)` and the constructor returns
/// the outer `Result<T, E>` type — closing the LHS-propagation gap that
/// previously left the `Err` slot as a fresh unification variable. For
/// non-Result expectations, falls through to bottom-up synthesis so
/// unification downstream catches mismatches.
pub(crate) fn check_ok(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ExprId,
    span: Span,
    expected: &Expected,
) -> Idx {
    if expected.has_expectation() {
        let resolved = engine.resolve(expected.ty);
        if engine.pool().tag(resolved) == Tag::Result {
            let inner_ok_ty = engine.pool().result_ok(resolved);
            let inner_err_ty = engine.pool().result_err(resolved);
            if inner.is_present() {
                let nested = Expected::from_context(inner_ok_ty, span, ContextKind::TryExpression);
                let _ = check_expr(engine, arena, inner, &nested, span);
            }
            return engine.pool_mut().result(inner_ok_ty, inner_err_ty);
        }
    }
    infer_ok(engine, arena, inner, span)
}

/// BD-2 check `Err(value)` against an outer `Result<T, E>` expectation.
///
/// Mirrors `check_ok` but propagates `Check(E)` into the inner expression.
pub(crate) fn check_err(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ExprId,
    span: Span,
    expected: &Expected,
) -> Idx {
    if expected.has_expectation() {
        let resolved = engine.resolve(expected.ty);
        if engine.pool().tag(resolved) == Tag::Result {
            let inner_ok_ty = engine.pool().result_ok(resolved);
            let inner_err_ty = engine.pool().result_err(resolved);
            if inner.is_present() {
                let nested = Expected::from_context(inner_err_ty, span, ContextKind::TryExpression);
                let _ = check_expr(engine, arena, inner, &nested, span);
            }
            return engine.pool_mut().result(inner_ok_ty, inner_err_ty);
        }
    }
    infer_err(engine, arena, inner, span)
}

/// BD-2 check `Some(value)` against an outer `Option<T>` expectation.
///
/// When `expected` resolves to `Option<T>`, propagates `Check(T)` to the
/// inner expression and returns `Option<T>`. Otherwise falls through to
/// bottom-up synthesis.
pub(crate) fn check_some(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ExprId,
    span: Span,
    expected: &Expected,
) -> Idx {
    if expected.has_expectation() {
        let resolved = engine.resolve(expected.ty);
        if engine.pool().tag(resolved) == Tag::Option {
            let inner_ty = engine.pool().option_inner(resolved);
            let nested = Expected::from_context(inner_ty, span, ContextKind::TryExpression);
            let _ = check_expr(engine, arena, inner, &nested, span);
            return engine.infer_option(inner_ty);
        }
    }
    infer_some(engine, arena, inner, span)
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
