//! Concurrency expression inference — catch, recurse, parallel, spawn, timeout, cache, with.

use ori_ir::ExprArena;

use super::super::InferEngine;
use super::infer_expr;
use crate::{Idx, Tag};

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

/// Infer type for `parallel(tasks: [expr])`.
pub(crate) fn infer_parallel(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Parallel takes a list of tasks and returns a list of results
    // For now, return [?a] with fresh variable
    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        // If it's a list, extract element type and wrap result
        let resolved = engine.resolve(ty);
        if engine.pool().tag(resolved) == Tag::List {
            let elem_ty = engine.pool().list_elem(resolved);
            return engine.pool_mut().list(elem_ty);
        }
    }

    let result_ty = engine.fresh_var();
    engine.pool_mut().list(result_ty)
}

/// Infer type for `spawn(task: expr)`.
pub(crate) fn infer_spawn(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Spawn returns a handle to the task
    // For now, return the task's result type wrapped in a fresh type
    // (Would need a Task<T> type in the pool)
    for prop in props {
        let _ = infer_expr(engine, arena, prop.value);
    }
    // TODO: Return proper Task<T> type when Task is added
    engine.fresh_var()
}

/// Infer type for `timeout(duration: Duration, task: expr)`.
pub(crate) fn infer_timeout(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    // Returns Option<T> where T is the task result
    let mut task_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        // Skip duration, capture task type (first non-duration property)
        if task_ty.is_none() {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) != Tag::Duration {
                task_ty = Some(ty);
            }
        }
        // If we already have a task type, just evaluate for type checking
    }

    let inner = task_ty.unwrap_or_else(|| engine.fresh_var());
    engine.pool_mut().option(inner)
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

/// Infer type for `with(acquire: expr, action: expr, release: expr)`.
///
/// Returns the `action` expression's type.
pub(crate) fn infer_with(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    props: &[ori_ir::NamedExpr],
) -> Idx {
    let mut action_ty = None;

    for prop in props {
        let ty = infer_expr(engine, arena, prop.value);
        if engine.lookup_name(prop.name) == Some("action") {
            action_ty = Some(ty);
        }
    }

    action_ty.unwrap_or_else(|| engine.fresh_var())
}
