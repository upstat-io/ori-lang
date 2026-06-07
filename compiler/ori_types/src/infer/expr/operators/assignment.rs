//! Assignment-expression inference.

use ori_ir::{ExprArena, ExprId, ExprKind, Span};

use super::super::super::InferEngine;
use super::super::infer_expr;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, TypeCheckError};

/// Infer the type of an assignment expression.
pub(crate) fn infer_assign(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    target: ExprId,
    value: ExprId,
    span: Span,
) -> Idx {
    // Check if target is an immutable binding (let $x = ...)
    if let ExprKind::Ident(name) = arena.get_expr(target).kind {
        if engine.env().is_mutable(name) == Some(false) {
            engine.push_error(TypeCheckError::assign_to_immutable(span, name));
        }
    }

    let target_ty = infer_expr(engine, arena, target);
    let value_ty = infer_expr(engine, arena, value);

    let expected = Expected {
        ty: target_ty,
        origin: ExpectedOrigin::Context {
            span: arena.get_expr(target).span,
            kind: ContextKind::Assignment,
        },
    };
    let _ = engine.check_type(value_ty, &expected, arena.get_expr(value).span);

    Idx::UNIT
}
