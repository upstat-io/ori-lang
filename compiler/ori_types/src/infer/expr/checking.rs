//! Bidirectional checking against an expected expression type.

use ori_ir::{ExprArena, ExprId, ExprKind, Span};

use crate::{Expected, Idx, Tag};

use super::{
    check_collect_method_call, check_err, check_ok, check_some, infer_expr, infer_method_call,
    infer_method_call_named, sequences, InferEngine,
};

/// Coerces integer literals in range 0-255 to byte when expected type is byte.
fn check_int_literal_coercion(
    engine: &mut InferEngine<'_>,
    expr_id: ExprId,
    kind: &ExprKind,
    expected_tag: Tag,
) -> Option<Idx> {
    if let ExprKind::Int(value) = kind {
        if expected_tag == Tag::Byte && *value >= 0 && *value <= 255 {
            engine.store_type(expr_id.raw() as usize, Idx::BYTE);
            return Some(Idx::BYTE);
        }
    }
    None
}

fn check_sum_constructors(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    kind: &ExprKind,
    expected: &Expected,
    span: Span,
) -> Option<Idx> {
    match kind {
        ExprKind::Ok(inner) => {
            let ty = check_ok(engine, arena, *inner, span, expected);
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }

        ExprKind::Err(inner) => {
            let ty = check_err(engine, arena, *inner, span, expected);
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }

        ExprKind::Some(inner) => {
            let ty = check_some(engine, arena, *inner, span, expected);
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }

        _ => None,
    }
}

fn check_method_calls(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    kind: &ExprKind,
    expected: &Expected,
    span: Span,
) -> Option<Idx> {
    match kind {
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let ty = infer_method_call(
                engine,
                arena,
                expr_id,
                *receiver,
                *method,
                *args,
                span,
                Some(expected),
            );
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }

        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => {
            let ty = infer_method_call_named(
                engine,
                arena,
                expr_id,
                *receiver,
                *method,
                *args,
                span,
                Some(expected),
            );
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            Some(ty)
        }

        _ => None,
    }
}

/// Check an expression against an expected type.
///
/// This implements the check direction of bidirectional type checking,
/// letting the type checker guide literal and method-call typing.
#[tracing::instrument(level = "trace", skip(engine, arena, expected))]
pub fn check_expr(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    expected: &Expected,
    span: Span,
) -> Idx {
    let expr = arena.get_expr(expr_id);

    let expected_ty = engine.resolve(expected.ty);
    let expected_tag = engine.pool().tag(expected_ty);

    if let Some(ty) = check_int_literal_coercion(engine, expr_id, &expr.kind, expected_tag) {
        return ty;
    }

    // Why: Bidirectional inference for iter.collect() with expected Set<T> resolves to Set<T> instead of [T].
    if let Some(ty) = check_collect_method_call(
        engine,
        arena,
        expr_id,
        &expr.kind,
        expected,
        expected_tag,
        span,
    ) {
        return ty;
    }

    // Why: Thread expected type through infer_try_seq to check against T and avoid double-wrapping.
    if let ExprKind::FunctionSeq(seq_id) = &expr.kind {
        if let ori_ir::FunctionSeq::Try { stmts, result, .. } = arena.get_function_seq(*seq_id) {
            let ty = sequences::infer_try_seq(engine, arena, *stmts, *result, span, expected);
            engine.store_type(expr_id.raw() as usize, ty);
            let _ = engine.check_type(ty, expected, span);
            return ty;
        }
    }

    if let Some(ty) = check_sum_constructors(engine, arena, expr_id, &expr.kind, expected, span) {
        return ty;
    }

    if let Some(ty) = check_method_calls(engine, arena, expr_id, &expr.kind, expected, span) {
        return ty;
    }

    let inferred = infer_expr(engine, arena, expr_id);
    let _ = engine.check_type(inferred, expected, span);
    inferred
}
