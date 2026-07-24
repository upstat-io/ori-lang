//! Result/Option constructor and control-flow expression inference.

use ori_ir::{ExprArena, ExprId, Name, Span};

use crate::{ContextKind, Expected, Idx, Tag, TypeCheckError};

use super::super::{scope::TryPropagation, InferEngine};
use super::{check_expr, infer_expr, infer_optional_or_unit};

/// Infer the type of `Ok(value)`.
pub(crate) fn infer_ok(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    inner: ExprId,
    _span: Span,
) -> Idx {
    let ok_ty = infer_optional_or_unit(engine, arena, inner);
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
    let err_ty = infer_optional_or_unit(engine, arena, inner);
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
/// expression is checked against `Check(T)`, propagating the annotation
/// into `inner` instead of leaving the `Err` slot a fresh unification
/// variable, and the constructor returns the outer `Result<T, E>` type.
/// For non-Result expectations, falls through to bottom-up synthesis so
/// subsequent unification catches mismatches.
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
            engine.record_try_propagation(TryPropagation::Option { span });
            engine.pool().option_inner(resolved)
        }
        Tag::Result => {
            // Result<T, E>? -> T (propagates Err)
            let error_ty = engine.pool().result_err(resolved);
            engine.record_try_propagation(TryPropagation::Result { error_ty, span });
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
    engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    _inner: ExprId,
    span: Span,
) -> Idx {
    engine.push_error(TypeCheckError::unsupported_feature(
        span,
        "await expressions",
    ));
    Idx::ERROR
}

/// Infer the type of a `with capability = provider in body` expression.
pub(crate) fn infer_with_capability(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    capability: Name,
    provider: ExprId,
    body: ExprId,
    span: Span,
) -> Idx {
    // Infer and freeze the provider type before entering the lexical frame.
    let inferred_provider_ty = infer_expr(engine, arena, provider);
    let provider_ty = engine.resolve(inferred_provider_ty);
    let capability_name = engine.lookup_name(capability);
    if matches!(capability_name, Some("Suspend" | "Unsafe")) {
        engine.push_error(TypeCheckError::unsatisfied_bound(
            span,
            format!(
                "marker capability `{}` cannot be explicitly bound; use its discharge context instead",
                capability_name.unwrap_or("<marker>")
            ),
        ));
    } else if !provider_satisfies_registered_capability(engine, capability, provider_ty) {
        let name = capability_name.unwrap_or("<capability>");
        engine.push_error(TypeCheckError::unsatisfied_bound(
            span,
            format!(
                "provider for capability `{name}` does not implement `{name}`; add an impl or bind a compatible provider value"
            ),
        ));
    }

    // Bind the capability name in a child scope so the body can
    // reference it as an identifier (e.g., `with Http = mock in Http`).
    engine.enter_scope();
    engine.env_mut().bind(capability, provider_ty);

    // Provide the capability for the duration of the body.
    // This makes calls to functions `uses <capability>` valid within.
    let body_ty = engine.with_capability_provider(
        crate::CapabilityProvider {
            capability,
            provider_type: provider_ty,
            source: crate::CapabilityProviderSource::WithBinding { provider },
        },
        |engine| infer_expr(engine, arena, body),
    );

    engine.exit_scope();
    body_ty
}

/// A `with` provider must satisfy a registered capability trait. Unregistered
/// names retain the legacy value-namespace behavior used by existing source;
/// unresolved generic providers are checked when their concrete impl methods
/// are selected. Concrete registered providers fail closed here even when the
/// body does not invoke a capability method.
fn provider_satisfies_registered_capability(
    engine: &InferEngine<'_>,
    capability: Name,
    provider_ty: Idx,
) -> bool {
    if provider_ty == Idx::ERROR
        || engine.pool().flags(provider_ty).has_any_var_or_infer()
        || engine
            .rigid_var_bounds(provider_ty)
            .is_some_and(|bounds| bounds.contains(&capability))
    {
        return true;
    }
    let Some(registry) = engine.trait_registry() else {
        return true;
    };
    let Some(trait_entry) = registry.get_trait_by_name(capability) else {
        return true;
    };
    registry.find_impl(trait_entry.idx, provider_ty).is_some()
        || registry.impls_of_trait(trait_entry.idx).any(|entry| {
            super::calls::match_self_type(
                engine.pool(),
                entry.self_type,
                provider_ty,
                &entry.type_params,
            )
            .is_some()
        })
}
