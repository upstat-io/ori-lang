//! Expression inference — catch, recurse, cache.

use ori_ir::ExprArena;

use super::super::InferEngine;
use super::infer_expr;
use crate::Idx;

/// Infer type for `catch(expr: expression)`.
///
/// Returns `Result<T, str>` where `T` is the type of the `expr` property.
pub(crate) fn infer_catch(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    let mut expr_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if engine.lookup_name(prop.name) == Some("expr") {
            expr_ty = Some(ty);
        }
    }

    let inner = expr_ty.unwrap_or_else(|| engine.fresh_var());
    engine.pool_mut().result(inner, Idx::STR)
}

/// Infer type for `recurse(condition: expr, base: expr, step: expr)`.
pub(crate) fn infer_recurse(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // The step expression needs access to `self` (the recursive function)
    // For now, we'll infer base and use that as the result type
    // Full implementation needs Section 07 (scoped bindings)

    let mut condition_ty = None;
    let mut base_ty = None;
    let mut step_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if condition_ty.is_none() {
            // condition should be bool
            condition_ty = Some(ty);
        } else if base_ty.is_none() {
            base_ty = Some(ty);
        } else if step_ty.is_none() {
            step_ty = Some(ty);
        }
    }

    // Condition must be bool
    if let Some(cond) = condition_ty {
        let _ = engine.unify_types(cond, Idx::BOOL);
    }

    // Base and step must have same type
    if let (Some(b), Some(s)) = (base_ty, step_ty) {
        let _ = engine.unify_types(b, s);
    }

    base_ty.unwrap_or_else(|| engine.fresh_var())
}

/// Infer type for `cache(key: expr, op: expr, ttl: Duration)`.
pub(crate) fn infer_cache(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Returns the `op` expression's type.
    // Match on prop names to avoid positional fragility.
    let mut op_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if engine.lookup_name(prop.name) == Some("op") {
            op_ty = Some(ty);
        }
    }

    op_ty.unwrap_or_else(|| engine.fresh_var())
}
