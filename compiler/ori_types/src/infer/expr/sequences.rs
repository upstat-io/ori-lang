//! Sequence pattern inference — `function_seq`, try, for-pattern, and bindings.

use ori_ir::{ExprArena, ExprId, FieldBinding, Name, Span};

use super::super::InferEngine;
use super::{
    check_expr, check_match_pattern, infer_expr, infer_let_binding_core, infer_match,
    lookup_struct_field_types,
};
use crate::{ContextKind, Expected, Idx, PatternKey, Tag, TypeCheckError};

/// Infer type for a `function_seq` expression (try, match, for).
///
/// `FunctionSeq` represents sequential expressions where order matters:
/// - **Try**: `try(let x = fallible()?, result)` - auto-unwrap `Result`/`Option`
/// - **Match**: `match(scrutinee, Pattern -> expr, ...)` - pattern matching
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
        } => {
            // Delegate to existing match inference
            infer_match(engine, arena, *scrutinee, *arms, *match_span)
        }

        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => infer_for_pattern(engine, arena, *over, *map, arm, *default, span),
    }
}

/// Infer type for `try(let x = fallible()?, result)`.
///
/// Like run, but auto-unwraps Result/Option types in let bindings.
/// The entire expression returns a Result or Option wrapping the result.
///
/// BD-2 propagation: when `expected` resolves to a
/// concrete `Result<T, E>`, the result expression is checked against
/// `Check(T)` and the propagating let-binding error type is reconciled
/// against `E`. For non-Result expectations (`NoExpectation`, fresh Var,
/// or any other tag), the function falls back to bottom-up synthesis
/// so unification downstream catches mismatches.
///
/// # Returns
///
/// The wrapped `Result` or `Option` type.
pub(crate) fn infer_try_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmts: ori_ir::StmtRange,
    result: ExprId,
    span: Span,
    expected: &Expected,
) -> Idx {
    let propagation = if expected.has_expectation() {
        let resolved = engine.resolve(expected.ty);
        if engine.pool().tag(resolved) == Tag::Result {
            Some((
                resolved,
                engine.pool().result_ok(resolved),
                engine.pool().result_err(resolved),
            ))
        } else {
            None
        }
    } else {
        None
    };

    engine.enter_scope();

    let mut error_ty: Option<Idx> = None;

    let stmts_list = arena.get_stmt_range(stmts);
    for stmt in stmts_list {
        if let ori_ir::StmtKind::Let { init, .. } = &stmt.kind {
            let value_ty = infer_expr(engine, arena, *init);
            let resolved = engine.resolve(value_ty);
            let tag = engine.pool().tag(resolved);
            if tag == Tag::Result && error_ty.is_none() {
                error_ty = Some(engine.pool().result_err(resolved));
            }
        }
        infer_try_stmt(engine, arena, stmt);
    }

    let final_ty = if let Some((outer_result, inner_ok_ty, inner_err_ty)) = propagation {
        // Spec: Tail-less try block: `result == ExprId::INVALID` (the `collect_block_stmts`
        // contract); the block value type is `void`. Guard like `infer_block` —
        // reconcile UNIT against the expected `Ok` payload, never index the INVALID
        // sentinel into the arena. Spec: Clause 16 (try), Clause 11 (block value).
        let result_expected = Expected::from_context(inner_ok_ty, span, ContextKind::TryExpression);
        if result.is_present() {
            let _ = check_expr(engine, arena, result, &result_expected, span);
        } else {
            // No tail expr: reconcile the block's `void` value against the expected
            // `Ok` payload (accumulates E2001 on mismatch) instead of indexing the
            // INVALID sentinel. `check_type` reports; `unify_types` would be silent.
            let _ = engine.check_type(Idx::UNIT, &result_expected, span);
        }
        if let Some(et) = error_ty {
            let _ = engine.unify_types(et, inner_err_ty);
        }
        outer_result
    } else {
        // Tail-less try block: no result expr to infer; the block value is `void`.
        let result_ty = if result.is_present() {
            infer_expr(engine, arena, result)
        } else {
            Idx::UNIT
        };
        let resolved = engine.resolve(result_ty);
        let tag = engine.pool().tag(resolved);
        match (tag, error_ty) {
            // Why: Result already wraps; unify inner Err with the let-binding
            // propagating error type so we get `Result<T, E>` instead of
            // the spurious `Result<Result<T, _>, E>` double-wrap. Reverts
            // the existing wrap-anything bug surfaced by control_flow.ori.
            (Tag::Result, Some(et)) => {
                let inner_err = engine.pool().result_err(resolved);
                let _ = engine.unify_types(inner_err, et);
                result_ty
            }
            (Tag::Result, None) | (Tag::Option, _) => result_ty,
            (_, Some(et)) => engine.pool_mut().result(result_ty, et),
            (_, None) => {
                let _ = span;
                let err_var = engine.fresh_var();
                engine.pool_mut().result(result_ty, err_var)
            }
        }
    };

    engine.exit_scope();
    final_ty
}

/// Infer type for `for(over: items, [map: transform,] match: Pattern -> expr, default: fallback)`.
///
/// Iterates over a collection, applies optional map, finds first matching pattern,
/// or returns default.
pub(crate) fn infer_for_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    over: ExprId,
    map: Option<ExprId>,
    arm: &ori_ir::MatchArm,
    default: ExprId,
    _span: Span,
) -> Idx {
    // Infer the iterable type
    let over_ty = infer_expr(engine, arena, over);
    let resolved_over = engine.resolve(over_ty);

    // Extract element type from collection
    let elem_ty = match engine.pool().tag(resolved_over) {
        Tag::List => engine.pool().list_elem(resolved_over),
        Tag::Set => engine.pool().set_elem(resolved_over),
        Tag::Range => engine.pool().range_elem(resolved_over),
        Tag::Map => engine.pool().map_key(resolved_over),
        _ => engine.fresh_var(), // Unknown iterable, create type var
    };

    // Apply optional map function
    let scrutinee_ty = if let Some(map_fn) = map {
        let map_fn_ty = infer_expr(engine, arena, map_fn);
        let resolved_map = engine.resolve(map_fn_ty);

        if engine.pool().tag(resolved_map) == Tag::Function {
            // Map function return type becomes the new element type
            engine.pool().function_return(resolved_map)
        } else {
            // Not a function, just use elem_ty
            elem_ty
        }
    } else {
        elem_ty
    };

    // Enter scope for pattern bindings
    engine.enter_scope();

    // Check pattern against scrutinee type.
    // for-pattern arms don't have an ArmRange, use a sentinel key.
    check_match_pattern(
        engine,
        arena,
        &arm.pattern,
        scrutinee_ty,
        PatternKey::Arm(u32::MAX),
        arm.span,
    );

    // Check guard if present
    if let Some(guard_id) = arm.guard {
        engine.push_context(ContextKind::MatchArmGuard { arm_index: 0 });
        let guard_ty = infer_expr(engine, arena, guard_id);
        let _ = engine.unify_types(guard_ty, Idx::BOOL);
        engine.pop_context();
    }

    // Infer arm body
    let arm_ty = infer_expr(engine, arena, arm.body);

    // Exit scope
    engine.exit_scope();

    // Infer default expression
    let default_ty = infer_expr(engine, arena, default);

    // Arm and default must have same type
    let _ = engine.unify_types(arm_ty, default_ty);

    arm_ty
}

/// Process a try-block statement (let or expression).
///
/// Auto-unwraps `Result`/`Option` types in let bindings.
pub(crate) fn infer_try_stmt(engine: &mut InferEngine<'_>, arena: &ExprArena, stmt: &ori_ir::Stmt) {
    match &stmt.kind {
        ori_ir::StmtKind::Let {
            pattern, ty, init, ..
        } => {
            let _ = infer_let_binding_core(engine, arena, *pattern, *ty, *init, true, stmt.span);
        }

        ori_ir::StmtKind::Expr(expr) => {
            // Statement expression - evaluate for side effects
            infer_expr(engine, arena, *expr);
        }
    }
}

/// Unwrap Result<T, E> -> T or Option<T> -> T.
pub(crate) fn unwrap_result_or_option(engine: &mut InferEngine<'_>, ty: Idx) -> Idx {
    let resolved = engine.resolve(ty);
    let tag = engine.pool().tag(resolved);

    match tag {
        Tag::Result => engine.pool().result_ok(resolved),
        Tag::Option => engine.pool().option_inner(resolved),
        _ => ty, // Not wrapped, return as-is
    }
}

/// Bind a binding pattern to a type, introducing variables into scope.
pub(crate) fn bind_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::BindingPattern,
    ty: Idx,
) {
    use ori_ir::BindingPattern;

    match pattern {
        BindingPattern::Name { name, mutable } => {
            engine.env_mut().bind_with_mutability(*name, ty, *mutable);
        }

        BindingPattern::Tuple(patterns) => {
            bind_tuple_pattern(engine, arena, patterns, ty);
        }

        BindingPattern::Struct { fields } => {
            bind_struct_pattern(engine, arena, fields, ty);
        }

        BindingPattern::List { elements, rest } => {
            bind_list_pattern(engine, arena, elements, *rest, ty);
        }

        BindingPattern::Wildcard => {
            // Wildcard binds nothing
        }
    }
}

/// Why: Sub-pattern binder helper for tuple patterns.
fn bind_tuple_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    patterns: &[ori_ir::BindingPattern],
    ty: Idx,
) {
    let resolved = engine.resolve(ty);
    if engine.pool().tag(resolved) == Tag::Tuple {
        let elem_types = engine.pool().tuple_elems(resolved);
        for (pat, elem_ty) in patterns.iter().zip(elem_types.iter()) {
            bind_pattern(engine, arena, pat, *elem_ty);
        }
    } else {
        // Why: Type mismatch - bind each to fresh var
        for pat in patterns {
            let var = engine.fresh_var();
            bind_pattern(engine, arena, pat, var);
        }
    }
}

/// Why: Sub-pattern binder helper for struct patterns.
fn bind_struct_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    fields: &[FieldBinding],
    ty: Idx,
) {
    let resolved = engine.resolve(ty);
    let field_type_map = match engine.pool().tag(resolved) {
        Tag::Named => {
            let type_name = engine.pool().named_name(resolved);
            lookup_struct_field_types(engine, type_name, None)
        }
        Tag::Applied => {
            let type_name = engine.pool().applied_name(resolved);
            let type_args = engine.pool().applied_args(resolved);
            lookup_struct_field_types(engine, type_name, Some(&type_args))
        }
        _ => None,
    };

    for field in fields {
        let field_ty = field_type_map
            .as_ref()
            .and_then(|m| m.get(&field.name).copied())
            .unwrap_or_else(|| engine.fresh_var());
        if let Some(sub_pat) = &field.pattern {
            bind_pattern(engine, arena, sub_pat, field_ty);
        } else {
            // Why: Shorthand: { x } or { $x } — use field's own mutability
            engine
                .env_mut()
                .bind_with_mutability(field.name, field_ty, field.mutable);
        }
    }
}

/// Why: Sub-pattern binder helper for list patterns.
fn bind_list_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: &[ori_ir::BindingPattern],
    rest: Option<(Name, ori_ir::Mutability)>,
    ty: Idx,
) {
    let resolved = engine.resolve(ty);
    if engine.pool().tag(resolved) == Tag::List {
        let elem_ty = engine.pool().list_elem(resolved);
        for pat in elements {
            bind_pattern(engine, arena, pat, elem_ty);
        }
        if let Some((rest_name, rest_mut)) = rest {
            // Why: Rest binding gets the full list type, respecting $ mutability
            engine
                .env_mut()
                .bind_with_mutability(rest_name, ty, rest_mut);
        }
    } else {
        // Why: Type mismatch - bind each to fresh var
        for pat in elements {
            let var = engine.fresh_var();
            bind_pattern(engine, arena, pat, var);
        }
        if let Some((rest_name, rest_mut)) = rest {
            engine
                .env_mut()
                .bind_with_mutability(rest_name, ty, rest_mut);
        }
    }
}

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
    func_exp: &ori_ir::FunctionExp,
) -> Idx {
    use ori_ir::FunctionExpKind;

    let props = arena.get_named_exprs(func_exp.props);

    match func_exp.kind {
        // Simple built-ins
        FunctionExpKind::Print => {
            // print(value: expr) -> unit
            // Evaluate the value (if present) for type checking
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::UNIT
        }

        FunctionExpKind::Panic => {
            // panic(message: expr) -> never
            // Evaluate message for type checking
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        FunctionExpKind::Todo => {
            // todo(message?: expr) -> never
            // Optional message
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        FunctionExpKind::Unreachable => {
            // unreachable(message?: expr) -> never
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            Idx::NEVER
        }

        // Error handling
        FunctionExpKind::Catch => {
            // catch(expr: expression) -> Result<T, str>
            super::infer_catch(engine, arena, props)
        }

        // Recursion
        FunctionExpKind::Recurse => {
            // recurse(condition: expr, base: expr, step: expr)
            // Complex: step can reference `self` (the recursive function)
            super::infer_recurse(engine, arena, props)
        }

        FunctionExpKind::Cache => {
            // cache(key: expr, op: expr, ttl: Duration) -> T
            super::infer_cache(engine, arena, props)
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
