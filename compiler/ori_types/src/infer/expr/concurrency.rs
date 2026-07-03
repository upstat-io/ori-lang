//! Expression inference — catch, recurse, cache, and other function-exp built-ins.

use ori_ir::{ExprArena, FunctionExp, FunctionExpKind};

use crate::{Idx, TypeCheckError};

use super::super::InferEngine;
use super::infer_expr;

/// Infer type for a `function_exp` expression (recurse, parallel, print, etc.).
///
/// `FunctionExp` represents named property expressions:
/// - **Print**: `print(value: expr)` -> unit
/// - **Panic**: `panic(message: expr)` -> never
/// - **Todo/Unreachable**: `todo(message?: expr)` -> never
/// - **Catch**: `catch(try: expr, catch: expr)` -> T
/// - **Recurse**: `recurse(condition: expr, base: expr, step: expr)` -> T
/// - **Parallel/Spawn/Timeout/Cache/With**: Concurrency patterns
///
/// # Returns
///
/// The inferred type.
pub(crate) fn infer_function_exp(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func_exp: &FunctionExp,
) -> Idx {
    let props = arena.get_named_exprs(func_exp.props);

    match func_exp.kind {
        // Simple built-ins
        FunctionExpKind::Print => {
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::UNIT
        }

        FunctionExpKind::Panic | FunctionExpKind::Todo | FunctionExpKind::Unreachable => {
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        // Error handling
        FunctionExpKind::Catch => infer_catch(engine, arena, props),

        // Recursion
        FunctionExpKind::Recurse => {
            // Complex: step can reference `self` (the recursive function)
            infer_recurse(engine, arena, props)
        }

        FunctionExpKind::Cache => {
            // cache(key: expr, op: expr, ttl: Duration) -> T
            infer_cache(engine, arena, props)
        }

        // Post-2026 concurrency — rejected at type checking (E2040)
        FunctionExpKind::Parallel
        | FunctionExpKind::Spawn
        | FunctionExpKind::Timeout
        | FunctionExpKind::With
        | FunctionExpKind::Channel
        | FunctionExpKind::ChannelIn
        | FunctionExpKind::ChannelOut
        | FunctionExpKind::ChannelAll => {
            engine.push_error(TypeCheckError::unsupported_feature(
                func_exp.span,
                func_exp.kind.name(),
            ));
            Idx::ERROR
        }
    }
}

/// Infer type for `catch(expr: expression)`.
///
/// Returns `Result<T, str>` where `T` is the type of the `expr` property.
fn infer_catch(
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
fn infer_recurse(
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
fn infer_cache(
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
