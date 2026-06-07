//! Cast-expression inference (`as` / `as?`).

use ori_ir::{ExprArena, ExprId, Span};

use super::super::super::InferEngine;
use super::super::{infer_expr, resolve_and_check_parsed_type};
use crate::Idx;

/// Infer the type of a cast expression.
pub(crate) fn infer_cast(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
    ty: &ori_ir::ParsedType,
    fallible: bool,
    span: Span,
) -> Idx {
    // Infer the expression type (for validation, though we don't check cast validity here)
    let _expr_ty = infer_expr(engine, arena, expr);

    // Resolve the target type
    let target_ty = resolve_and_check_parsed_type(engine, arena, ty, span);

    // Fallible casts return Option<T>, infallible return T directly
    if fallible {
        engine.pool_mut().option(target_ty)
    } else {
        target_ty
    }
}
