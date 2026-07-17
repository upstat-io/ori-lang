//! Sequence pattern inference — `function_seq`, try, and for-pattern.

use ori_ir::{ExprArena, ExprId, Span};

use crate::{ContextKind, Expected, Idx, PatternKey, Tag};

use super::super::{scope::TryPropagation, InferEngine};
use super::{
    check_match_pattern, for_loop_elem_ty, infer_expr, infer_match, infer_optional_or_unit,
    infer_stmt,
};

/// Infer type for a `function_seq` expression (try, match, for).
///
/// `FunctionSeq` represents sequential expressions where order matters:
/// - **Try**: `try { stmts; result }` - capture explicit `?` propagation
/// - **Match**: `match scrutinee { Pattern -> expr, ... }` - pattern matching
/// - **`ForPattern`**: `for(over: items, match: Pattern -> expr, default: fallback)`
pub(crate) fn infer_function_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func_seq: &ori_ir::FunctionSeq,
    span: Span,
) -> Idx {
    use ori_ir::FunctionSeq;

    match func_seq {
        FunctionSeq::Try { stmts, result, .. } => infer_try_seq(
            engine,
            arena,
            *stmts,
            *result,
            span,
            &Expected::no_expectation(Idx::ERROR),
        ),

        FunctionSeq::Match {
            scrutinee,
            arms,
            span: match_span,
        } => infer_match(engine, arena, *scrutinee, *arms, *match_span),

        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => infer_for_pattern(engine, arena, *over, *map, arm, *default, span),
    }
}

/// Infer type for `try { let x = fallible()?; result }`.
///
/// Like a block, but explicit `?` expressions propagate to this boundary;
/// ordinary let bindings retain their initializer's source type. The whole
/// expression returns a [`Result`] or [`Option`] wrapping the result.
/// When `expected` resolves to a concrete `Result<T, E>` or `Option<T>`, the
/// result expression and every propagated carrier reconcile against that
/// boundary; otherwise inference falls back to bottom-up synthesis. Returns
/// the wrapped `Result` or `Option` type.
pub(crate) fn infer_try_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmts: ori_ir::StmtRange,
    result: ExprId,
    span: Span,
    expected: &Expected,
) -> Idx {
    let expected_try = if expected.has_expectation() {
        let resolved = engine.resolve(expected.ty);
        match engine.pool().tag(resolved) {
            Tag::Result => Some(ExpectedTry::Result {
                outer: resolved,
                payload: engine.pool().result_ok(resolved),
                error: engine.pool().result_err(resolved),
            }),
            Tag::Option => Some(ExpectedTry::Option {
                outer: resolved,
                payload: engine.pool().option_inner(resolved),
            }),
            _ => None,
        }
    } else {
        None
    };

    engine.push_try_boundary();
    engine.enter_scope();

    let stmts_list = arena.get_stmt_range(stmts);
    for stmt in stmts_list {
        infer_stmt(engine, arena, stmt);
    }

    let result_ty = infer_optional_or_unit(engine, arena, result);

    engine.exit_scope();
    let propagations = engine.pop_try_boundary();

    if let Some(expected_try) = expected_try {
        check_try_result(engine, result_ty, result.is_present(), expected_try, span);
        let expected_carrier = expected_try.carrier();
        let _ = reconcile_try_propagations(engine, Some(expected_carrier), &propagations, span);
        expected_try.outer()
    } else {
        let carrier = reconcile_try_propagations(engine, None, &propagations, span);
        synthesize_try_result(engine, result_ty, carrier, span)
    }
}

#[derive(Copy, Clone)]
enum ExpectedTry {
    Result {
        outer: Idx,
        payload: Idx,
        error: Idx,
    },
    Option {
        outer: Idx,
        payload: Idx,
    },
}

impl ExpectedTry {
    fn outer(self) -> Idx {
        match self {
            Self::Result { outer, .. } | Self::Option { outer, .. } => outer,
        }
    }

    fn payload(self) -> Idx {
        match self {
            Self::Result { payload, .. } | Self::Option { payload, .. } => payload,
        }
    }

    fn carrier(self) -> TryCarrier {
        match self {
            Self::Result { error, .. } => TryCarrier::Result(error),
            Self::Option { .. } => TryCarrier::Option,
        }
    }
}

#[derive(Copy, Clone)]
enum TryCarrier {
    Result(Idx),
    Option,
}

/// Check the try block's tail against either the complete expected carrier
/// (when the tail is already wrapped) or the carrier's payload (a bare tail).
fn check_try_result(
    engine: &mut InferEngine<'_>,
    result_ty: Idx,
    result_is_present: bool,
    expected_try: ExpectedTry,
    span: Span,
) {
    let resolved_result = engine.resolve(result_ty);
    let result_tag = engine.pool().tag(resolved_result);
    let tail_is_wrapped = matches!(
        (expected_try, result_tag),
        (ExpectedTry::Result { .. }, Tag::Result) | (ExpectedTry::Option { .. }, Tag::Option)
    );
    let expected_ty = if tail_is_wrapped {
        expected_try.outer()
    } else {
        expected_try.payload()
    };
    let expected = Expected::from_context(expected_ty, span, ContextKind::TryExpression);
    let inferred = if result_is_present {
        result_ty
    } else {
        Idx::UNIT
    };
    let _ = engine.check_type(inferred, &expected, span);
}

/// Reconcile the carriers from explicit `?` expressions. The expected try
/// type, when present, seeds the carrier so every propagation operation is
/// checked against the declared boundary type.
fn reconcile_try_propagations(
    engine: &mut InferEngine<'_>,
    mut carrier: Option<TryCarrier>,
    propagations: &[TryPropagation],
    boundary_span: Span,
) -> Option<TryCarrier> {
    for propagation in propagations {
        let (observed, propagation_span) = match *propagation {
            TryPropagation::Option { span } => (TryCarrier::Option, span),
            TryPropagation::Result { error_ty, span } => (TryCarrier::Result(error_ty), span),
        };
        carrier = Some(reconcile_try_carrier(
            engine,
            carrier,
            observed,
            propagation_span,
            boundary_span,
        ));
    }
    carrier
}

fn reconcile_try_carrier(
    engine: &mut InferEngine<'_>,
    current: Option<TryCarrier>,
    observed: TryCarrier,
    propagation_span: Span,
    boundary_span: Span,
) -> TryCarrier {
    let Some(current) = current else {
        return observed;
    };

    match (current, observed) {
        (TryCarrier::Result(expected_error), TryCarrier::Result(actual_error)) => {
            let expected =
                Expected::from_context(expected_error, boundary_span, ContextKind::TryExpression);
            let _ = engine.check_type(actual_error, &expected, propagation_span);
        }
        (TryCarrier::Option, TryCarrier::Option) => {}
        (expected_carrier, actual_carrier) => {
            let expected_ty = carrier_marker_type(engine, expected_carrier);
            let actual_ty = carrier_marker_type(engine, actual_carrier);
            let expected =
                Expected::from_context(expected_ty, boundary_span, ContextKind::TryExpression);
            let _ = engine.check_type(actual_ty, &expected, propagation_span);
        }
    }
    current
}

fn carrier_marker_type(engine: &mut InferEngine<'_>, carrier: TryCarrier) -> Idx {
    match carrier {
        TryCarrier::Result(error_ty) => engine.pool_mut().result(Idx::UNIT, error_ty),
        TryCarrier::Option => engine.pool_mut().option(Idx::UNIT),
    }
}

fn synthesize_try_result(
    engine: &mut InferEngine<'_>,
    result_ty: Idx,
    carrier: Option<TryCarrier>,
    span: Span,
) -> Idx {
    let resolved = engine.resolve(result_ty);
    let tag = engine.pool().tag(resolved);

    match (carrier, tag) {
        (Some(TryCarrier::Result(error_ty)), Tag::Result) => {
            let tail_error = engine.pool().result_err(resolved);
            let _ = reconcile_try_carrier(
                engine,
                Some(TryCarrier::Result(error_ty)),
                TryCarrier::Result(tail_error),
                span,
                span,
            );
            result_ty
        }
        (Some(TryCarrier::Option), Tag::Option) | (None, Tag::Result | Tag::Option) => result_ty,
        (Some(expected_carrier), Tag::Result) => {
            let tail_error = engine.pool().result_err(resolved);
            let _ = reconcile_try_carrier(
                engine,
                Some(expected_carrier),
                TryCarrier::Result(tail_error),
                span,
                span,
            );
            result_ty
        }
        (Some(expected_carrier), Tag::Option) => {
            let _ = reconcile_try_carrier(
                engine,
                Some(expected_carrier),
                TryCarrier::Option,
                span,
                span,
            );
            result_ty
        }
        (Some(TryCarrier::Result(error_ty)), _) => engine.pool_mut().result(result_ty, error_ty),
        (Some(TryCarrier::Option), _) => engine.pool_mut().option(result_ty),
        (None, _) => {
            let error_ty = engine.fresh_var();
            engine.pool_mut().result(result_ty, error_ty)
        }
    }
}

/// Infer type for `for(over: items, [map: transform,] match: Pattern -> expr, default: fallback)`.
///
/// Iterates over a collection, applies optional map, finds first matching pattern,
/// or returns default.
fn infer_for_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    over: ExprId,
    map: Option<ExprId>,
    arm: &ori_ir::MatchArm,
    default: ExprId,
    span: Span,
) -> Idx {
    let over_ty = infer_expr(engine, arena, over);
    let elem_ty = for_loop_elem_ty(engine, over_ty, span);

    let scrutinee_ty = if let Some(map_fn) = map {
        let map_fn_ty = infer_expr(engine, arena, map_fn);
        let resolved_map = engine.resolve(map_fn_ty);

        if engine.pool().tag(resolved_map) == Tag::Function {
            engine.pool().function_return(resolved_map)
        } else {
            elem_ty
        }
    } else {
        elem_ty
    };

    engine.enter_scope();

    // Why: A for-pattern carries one inline MatchArm, not an ArmRange, so no
    // real arm_index exists per the PatternKey::Arm contract; u32::MAX is an
    // out-of-range placeholder key ori_canon never reads via resolve_pattern.
    check_match_pattern(
        engine,
        arena,
        &arm.pattern,
        scrutinee_ty,
        PatternKey::Arm(u32::MAX),
        arm.span,
    );

    if let Some(guard_id) = arm.guard {
        engine.push_context(ContextKind::MatchArmGuard { arm_index: 0 });
        let guard_ty = infer_expr(engine, arena, guard_id);
        let expected = Expected::from_context(
            Idx::BOOL,
            arena.get_expr(guard_id).span,
            ContextKind::MatchArmGuard { arm_index: 0 },
        );
        let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard_id).span);
        engine.pop_context();
    }

    let arm_ty = infer_expr(engine, arena, arm.body);

    engine.exit_scope();

    let default_ty = infer_expr(engine, arena, default);

    let expected = Expected::from_context(
        arm_ty,
        arena.get_expr(default).span,
        ContextKind::MatchArm { arm_index: 0 },
    );
    let _ = engine.check_type(default_ty, &expected, arena.get_expr(default).span);

    arm_ty
}
