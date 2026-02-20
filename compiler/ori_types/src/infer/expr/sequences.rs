//! Sequence pattern inference — `function_seq`, try, for-pattern, and bindings.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};

use super::super::InferEngine;
use super::{
    check_expr, check_match_pattern, infer_expr, infer_match, lookup_struct_field_types,
    pattern_first_name, resolve_and_check_parsed_type,
};
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, PatternKey, Tag};

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
        FunctionSeq::Try { stmts, result, .. } => {
            infer_try_seq(engine, arena, *stmts, *result, span)
        }

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
pub(crate) fn infer_try_seq(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmts: ori_ir::StmtRange,
    result: ExprId,
    span: Span,
) -> Idx {
    // Enter a new scope for the try block
    engine.enter_scope();

    // Track the error type for Result propagation
    let mut error_ty: Option<Idx> = None;

    // Process each statement in sequence (with unwrapping)
    let stmts_list = arena.get_stmt_range(stmts);
    for stmt in stmts_list {
        if let ori_ir::StmtKind::Let { init, .. } = &stmt.kind {
            // Infer the value type first
            let value_ty = infer_expr(engine, arena, *init);
            let resolved = engine.resolve(value_ty);
            let tag = engine.pool().tag(resolved);

            // Track error type from Result
            if tag == Tag::Result && error_ty.is_none() {
                error_ty = Some(engine.pool().result_err(resolved));
            }
        }
        // Process statement with try-unwrapping enabled
        infer_try_stmt(engine, arena, stmt, true);
    }

    // Infer the result expression
    let result_ty = infer_expr(engine, arena, result);

    // Exit scope
    engine.exit_scope();

    // The result type depends on what was in the bindings
    // If we saw Results, wrap the result in Result<T, E>
    // If we saw Options, wrap in Option<T>
    // Otherwise, return as-is (though this shouldn't happen in valid try blocks)
    if let Some(err_ty) = error_ty {
        engine.pool_mut().result(result_ty, err_ty)
    } else {
        // Check if result is already wrapped
        let resolved = engine.resolve(result_ty);
        let tag = engine.pool().tag(resolved);
        if tag == Tag::Result || tag == Tag::Option {
            result_ty
        } else {
            // Default to Result with a fresh error type for proper try semantics
            let _ = span; // Available for future error reporting
            let err_var = engine.fresh_var();
            engine.pool_mut().result(result_ty, err_var)
        }
    }
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
/// If `try_unwrap` is true, auto-unwrap Result/Option in let bindings.
pub(crate) fn infer_try_stmt(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    stmt: &ori_ir::Stmt,
    try_unwrap: bool,
) {
    match &stmt.kind {
        ori_ir::StmtKind::Let {
            pattern, ty, init, ..
        } => {
            let pat = arena.get_binding_pattern(*pattern);

            // Track error count for closure self-capture detection
            let binding_name = pattern_first_name(pat);
            let errors_before = engine.error_count();

            // Enter scope for let-polymorphism (allows generalization of lambdas)
            engine.enter_scope();

            // Handle type annotation if present, or generalize for let-polymorphism
            let final_ty = if ty.is_valid() {
                // With type annotation
                let parsed_ty = arena.get_parsed_type(*ty);
                let expected_ty =
                    resolve_and_check_parsed_type(engine, arena, parsed_ty, stmt.span);

                if try_unwrap {
                    // For try blocks: infer, unwrap, then check against annotation
                    // e.g., `let x: int = succeed(42)` where succeed returns Result<int>
                    let init_ty = infer_expr(engine, arena, *init);
                    let unwrapped = unwrap_result_or_option(engine, init_ty);

                    let expected = Expected {
                        ty: expected_ty,
                        origin: ExpectedOrigin::Annotation {
                            name: pattern_first_name(pat).unwrap_or(Name::EMPTY),
                            span: stmt.span,
                        },
                    };
                    let _ = engine.check_type(unwrapped, &expected, stmt.span);
                    expected_ty
                } else {
                    // For run blocks: use bidirectional checking (allows literal coercion)
                    // e.g., `let x: byte = 65` coerces int literal to byte
                    let expected = Expected {
                        ty: expected_ty,
                        origin: ExpectedOrigin::Annotation {
                            name: pattern_first_name(pat).unwrap_or(Name::EMPTY),
                            span: stmt.span,
                        },
                    };
                    let _init_ty = check_expr(engine, arena, *init, &expected, stmt.span);
                    expected_ty
                }
            } else {
                // No annotation: infer the initializer type
                let init_ty = infer_expr(engine, arena, *init);

                // Detect closure self-capture: if the init is a lambda and any new
                // errors are UnknownIdent matching the binding name, rewrite them.
                // Example: `run(let f = () -> f, ...)` — f isn't yet in scope.
                if let Some(name) = binding_name {
                    if matches!(arena.get_expr(*init).kind, ExprKind::Lambda { .. }) {
                        engine.rewrite_self_capture_errors(name, errors_before);
                    }
                }

                // For try blocks, unwrap Result/Option
                let bound_ty = if try_unwrap {
                    unwrap_result_or_option(engine, init_ty)
                } else {
                    init_ty
                };

                // Generalize free type variables for let-polymorphism
                // This enables: `let id = x -> x, id(42), id("hello")`
                engine.generalize(bound_ty)
            };

            // Exit scope before binding (generalization happens at current rank)
            engine.exit_scope();

            // Bind pattern to type
            bind_pattern(engine, arena, pat, final_ty);
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
#[expect(
    clippy::only_used_in_recursion,
    reason = "Arena is threaded through for recursive sub-pattern binding"
)]
pub(crate) fn bind_pattern(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pattern: &ori_ir::BindingPattern,
    ty: Idx,
) {
    use ori_ir::BindingPattern;

    match pattern {
        BindingPattern::Name { name, mutable } => {
            engine
                .env_mut()
                .bind_with_mutability(*name, ty, mutable.is_mutable());
        }

        BindingPattern::Tuple(patterns) => {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) == Tag::Tuple {
                let elem_types = engine.pool().tuple_elems(resolved);
                for (pat, elem_ty) in patterns.iter().zip(elem_types.iter()) {
                    bind_pattern(engine, arena, pat, *elem_ty);
                }
            } else {
                // Type mismatch - bind each to fresh var
                for pat in patterns {
                    let var = engine.fresh_var();
                    bind_pattern(engine, arena, pat, var);
                }
            }
        }

        BindingPattern::Struct { fields } => {
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
                    // Shorthand: { x } or { $x } — use field's own mutability
                    engine.env_mut().bind_with_mutability(
                        field.name,
                        field_ty,
                        field.mutable.is_mutable(),
                    );
                }
            }
        }

        BindingPattern::List { elements, rest } => {
            let resolved = engine.resolve(ty);
            if engine.pool().tag(resolved) == Tag::List {
                let elem_ty = engine.pool().list_elem(resolved);
                for pat in elements {
                    bind_pattern(engine, arena, pat, elem_ty);
                }
                if let Some((rest_name, rest_mut)) = rest {
                    // Rest binding gets the full list type, respecting $ mutability
                    engine
                        .env_mut()
                        .bind_with_mutability(*rest_name, ty, rest_mut.is_mutable());
                }
            } else {
                // Type mismatch - bind each to fresh var
                for pat in elements {
                    let var = engine.fresh_var();
                    bind_pattern(engine, arena, pat, var);
                }
                if let Some((rest_name, rest_mut)) = rest {
                    engine
                        .env_mut()
                        .bind_with_mutability(*rest_name, ty, rest_mut.is_mutable());
                }
            }
        }

        BindingPattern::Wildcard => {
            // Wildcard binds nothing
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

        // Concurrency patterns
        FunctionExpKind::Parallel => {
            // parallel(tasks: [expr]) -> [T]
            // Returns list of results from parallel execution
            super::infer_parallel(engine, arena, props)
        }

        FunctionExpKind::Spawn => {
            // spawn(task: expr) -> Task<T>
            // Returns a handle to the spawned task
            super::infer_spawn(engine, arena, props)
        }

        FunctionExpKind::Timeout => {
            // timeout(duration: Duration, task: expr) -> Option<T>
            // Returns Some(result) or None if timeout
            super::infer_timeout(engine, arena, props)
        }

        FunctionExpKind::Cache => {
            // cache(key: expr, op: expr, ttl: Duration) -> T
            super::infer_cache(engine, arena, props)
        }

        FunctionExpKind::With => {
            // with(acquire: expr, action: expr, release: expr) -> T
            super::infer_with(engine, arena, props)
        }

        // Channel constructors — stub: infer props, return fresh type var
        FunctionExpKind::Channel
        | FunctionExpKind::ChannelIn
        | FunctionExpKind::ChannelOut
        | FunctionExpKind::ChannelAll => {
            for prop in props {
                infer_expr(engine, arena, prop.value);
            }
            engine.fresh_var()
        }
    }
}
