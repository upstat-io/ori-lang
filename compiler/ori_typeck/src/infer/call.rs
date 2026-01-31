//! Call expression type inference.
//!
//! Handles function calls, method calls, and named argument calls.

use super::builtin_methods::{BuiltinMethodRegistry, MethodTypeResult};
use super::infer_expr;
use crate::checker::TypeChecker;
use ori_ir::{CallArgRange, ExprId, ExprKind, ExprRange, Name, Span};
use ori_types::Type;

/// Infer type for a function call (positional arguments).
pub fn infer_call(
    checker: &mut TypeChecker<'_>,
    func: ExprId,
    args: ExprRange,
    span: Span,
) -> Type {
    let func_expr = checker.context.arena.get_expr(func);
    let positional_allowed = match &func_expr.kind {
        ExprKind::Ident(name) => {
            let name_str = checker.context.interner.lookup(*name);
            if matches!(name_str, "int" | "float" | "str" | "byte") {
                true
            } else {
                !checker.scope.function_sigs.contains_key(name)
            }
        }
        _ => true,
    };

    let arg_ids = checker.context.arena.get_expr_list(args);

    if !positional_allowed && !arg_ids.is_empty() {
        // Extract function name for better error message
        let func_name = match &func_expr.kind {
            ExprKind::Ident(name) => Some(checker.context.interner.lookup(*name)),
            _ => None,
        };
        let message = match func_name {
            Some(name) => {
                format!("named arguments required when calling `@{name}` (use name: value syntax)")
            }
            None => {
                "named arguments required for function calls (use name: value syntax)".to_string()
            }
        };
        checker.push_error(message, span, ori_diagnostic::ErrorCode::E2011);
        return Type::Error;
    }

    let func_ty = infer_expr(checker, func);

    // Pre-allocate to avoid repeated reallocations
    let mut arg_types = Vec::with_capacity(arg_ids.len());
    for id in arg_ids {
        arg_types.push(infer_expr(checker, *id));
    }

    check_call(checker, &func_ty, &arg_types, span)
}

/// Check a function call.
fn check_call(checker: &mut TypeChecker<'_>, func: &Type, args: &[Type], span: Span) -> Type {
    let result = checker.inference.ctx.fresh_var();
    let expected = Type::Function {
        params: args.to_vec(),
        ret: Box::new(result.clone()),
    };

    if let Err(e) = checker.inference.ctx.unify(func, &expected) {
        checker.report_type_error(&e, span);
        return Type::Error;
    }

    checker.inference.ctx.resolve(&result)
}

/// Infer type for a function call with named arguments.
pub fn infer_call_named(
    checker: &mut TypeChecker<'_>,
    func: ExprId,
    args: CallArgRange,
    span: Span,
) -> Type {
    let func_name = {
        let func_expr = checker.context.arena.get_expr(func);
        match &func_expr.kind {
            ExprKind::Ident(name) => Some(*name),
            _ => None,
        }
    };

    let func_ty = infer_expr(checker, func);
    let call_args = checker.context.arena.get_call_args(args);

    // Pre-allocate to avoid repeated reallocations
    let mut arg_types = Vec::with_capacity(call_args.len());
    for arg in call_args {
        arg_types.push(infer_expr(checker, arg.value));
    }

    let (result, resolved_params) = match func_ty {
        Type::Function { params, ret } => {
            let has_arity_error = params.len() != arg_types.len();
            if has_arity_error {
                let message = if let Some(name) = func_name {
                    format!(
                        "function `{}` expects {} arguments, found {}",
                        checker.context.interner.lookup(name),
                        params.len(),
                        arg_types.len()
                    )
                } else {
                    format!(
                        "expected {} arguments, found {}",
                        params.len(),
                        arg_types.len()
                    )
                };
                checker.push_error(message, span, ori_diagnostic::ErrorCode::E2004);
            }

            // Type-check available arguments even on arity mismatch to catch more errors
            for (i, (param_ty, arg_ty)) in params.iter().zip(arg_types.iter()).enumerate() {
                if let Err(e) = checker.inference.ctx.unify(param_ty, arg_ty) {
                    let arg_span = call_args[i].span;
                    checker.report_type_error(&e, arg_span);
                }
            }

            if has_arity_error {
                return Type::Error;
            }

            // Pre-allocate to avoid repeated reallocations
            let mut resolved = Vec::with_capacity(params.len());
            for p in &params {
                resolved.push(checker.inference.ctx.resolve(p));
            }

            (*ret, Some(resolved))
        }
        Type::Error => (Type::Error, None),
        _ => {
            checker.push_error(
                "expected function type for call".to_string(),
                span,
                ori_diagnostic::ErrorCode::E2001,
            );
            (Type::Error, None)
        }
    };

    if let Some(name) = func_name {
        check_generic_bounds(checker, name, resolved_params.as_deref(), span);
        check_capability_propagation(checker, name, span);
    }

    result
}

/// Check trait bounds for a generic function call.
fn check_generic_bounds(
    checker: &mut TypeChecker<'_>,
    func_name: Name,
    resolved_params: Option<&[Type]>,
    span: Span,
) {
    checker.check_function_bounds(func_name, resolved_params, span);
}

/// Check capability propagation for a function call.
fn check_capability_propagation(checker: &mut TypeChecker<'_>, func_name: Name, span: Span) {
    let Some(func_sig) = checker.scope.function_sigs.get(&func_name) else {
        return;
    };

    // Collect missing capabilities first to avoid borrow conflict
    // (iterating over func_sig while calling checker.push_error)
    let missing_caps: Vec<_> = func_sig
        .capabilities
        .iter()
        .filter(|cap| {
            !checker.scope.current_function_caps.contains(cap)
                && !checker.scope.provided_caps.contains(cap)
        })
        .copied()
        .collect();

    // Now push errors for each missing capability
    for required_cap in missing_caps {
        let func_name_str = checker.context.interner.lookup(func_name);
        let cap_name_str = checker.context.interner.lookup(required_cap);
        checker.push_error(
            format!(
                "function `{func_name_str}` uses `{cap_name_str}` capability, \
                 but caller does not declare or provide it"
            ),
            span,
            ori_diagnostic::ErrorCode::E2014,
        );
    }
}

/// Infer type for a method call (positional arguments).
pub fn infer_method_call(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    method: Name,
    args: ExprRange,
    span: Span,
) -> Type {
    let arg_ids = checker.context.arena.get_expr_list(args);
    if !arg_ids.is_empty() {
        checker.push_error(
            "named arguments required for method calls (name: value)".to_string(),
            span,
            ori_diagnostic::ErrorCode::E2011,
        );
        return Type::Error;
    }

    let receiver_ty = infer_expr(checker, receiver);
    let resolved_receiver = checker.inference.ctx.resolve(&receiver_ty);

    // Collect types and spans in single pass to avoid repeated iteration
    let (arg_types, arg_spans): (Vec<Type>, Vec<Span>) = arg_ids
        .iter()
        .map(|id| {
            let ty = infer_expr(checker, *id);
            let span = checker.context.arena.get_expr(*id).span;
            (ty, span)
        })
        .unzip();

    infer_method_call_core(
        checker,
        &resolved_receiver,
        method,
        &arg_types,
        &arg_spans,
        span,
    )
}

/// Infer type for a method call with named arguments.
pub fn infer_method_call_named(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    method: Name,
    args: CallArgRange,
    span: Span,
) -> Type {
    let receiver_ty = infer_expr(checker, receiver);
    let resolved_receiver = checker.inference.ctx.resolve(&receiver_ty);

    let call_args = checker.context.arena.get_call_args(args);
    // Collect types and spans in single pass to avoid repeated iteration
    let (arg_types, arg_spans): (Vec<Type>, Vec<Span>) = call_args
        .iter()
        .map(|arg| (infer_expr(checker, arg.value), arg.span))
        .unzip();

    infer_method_call_core(
        checker,
        &resolved_receiver,
        method,
        &arg_types,
        &arg_spans,
        span,
    )
}

/// Core method call inference logic shared between positional and named variants.
///
/// This function handles:
/// - Associated function calls on type names (e.g., `Point.origin`, `Duration.from_seconds`)
/// - Looking up methods in the trait registry
/// - Checking argument count matches expected
/// - Unifying argument types with parameter types
/// - Falling back to builtin methods if not found in registry
fn infer_method_call_core(
    checker: &mut TypeChecker<'_>,
    resolved_receiver: &Type,
    method: Name,
    arg_types: &[Type],
    arg_spans: &[Span],
    span: Span,
) -> Type {
    // Check for associated function calls on type names (e.g., Point.origin, Duration.from_seconds)
    if let Type::Named(type_name) = resolved_receiver {
        // First, check user-defined associated functions in the registry
        if let Some(method_lookup) = checker
            .registries
            .traits
            .lookup_associated_function(*type_name, method)
        {
            // Associated functions have no self parameter, so all params are arguments
            if arg_types.len() != method_lookup.params.len() {
                checker.push_error(
                    format!(
                        "associated function `{}` expects {} arguments, found {}",
                        checker.context.interner.lookup(method),
                        method_lookup.params.len(),
                        arg_types.len()
                    ),
                    span,
                    ori_diagnostic::ErrorCode::E2004,
                );
                return Type::Error;
            }

            // Unify argument types with parameter types
            for (i, (param_ty, arg_ty)) in method_lookup
                .params
                .iter()
                .zip(arg_types.iter())
                .enumerate()
            {
                if let Err(e) = checker.inference.ctx.unify(param_ty, arg_ty) {
                    let arg_span = arg_spans.get(i).copied().unwrap_or(span);
                    checker.report_type_error(&e, arg_span);
                }
            }

            return method_lookup.return_ty.clone();
        }

        // Check for def impl methods on trait names (e.g., Calculator.add when there's def impl Calculator)
        if let Some(method_lookup) = checker
            .registries
            .traits
            .lookup_def_impl_method(*type_name, method)
        {
            // def impl methods have no self parameter, so all params are arguments
            if arg_types.len() != method_lookup.params.len() {
                checker.push_error(
                    format!(
                        "def impl method `{}` expects {} arguments, found {}",
                        checker.context.interner.lookup(method),
                        method_lookup.params.len(),
                        arg_types.len()
                    ),
                    span,
                    ori_diagnostic::ErrorCode::E2004,
                );
                return Type::Error;
            }

            // Unify argument types with parameter types
            for (i, (param_ty, arg_ty)) in method_lookup
                .params
                .iter()
                .zip(arg_types.iter())
                .enumerate()
            {
                if let Err(e) = checker.inference.ctx.unify(param_ty, arg_ty) {
                    let arg_span = arg_spans.get(i).copied().unwrap_or(span);
                    checker.report_type_error(&e, arg_span);
                }
            }

            return method_lookup.return_ty.clone();
        }

        // Fall back to built-in associated functions (Duration, Size)
        let type_name_str = checker.context.interner.lookup(*type_name);
        if is_builtin_type_with_associated_functions(type_name_str) {
            return infer_builtin_associated_function(
                checker,
                type_name_str,
                method,
                arg_types,
                span,
            );
        }
    }

    if let Some(method_lookup) = checker
        .registries
        .traits
        .lookup_method(resolved_receiver, method)
    {
        // Calculate expected arg count (excluding self parameter)
        let expected_arg_count = method_lookup.params.len().saturating_sub(1);

        if arg_types.len() != expected_arg_count {
            checker.push_error(
                format!(
                    "method `{}` expects {} arguments, found {}",
                    checker.context.interner.lookup(method),
                    expected_arg_count,
                    arg_types.len()
                ),
                span,
                ori_diagnostic::ErrorCode::E2004,
            );
            return Type::Error;
        }

        // Skip self parameter when unifying
        let param_types: Vec<_> = method_lookup.params.iter().skip(1).collect();
        for (i, (param_ty, arg_ty)) in param_types.iter().zip(arg_types.iter()).enumerate() {
            if let Err(e) = checker.inference.ctx.unify(param_ty, arg_ty) {
                let arg_span = arg_spans.get(i).copied().unwrap_or(span);
                checker.report_type_error(&e, arg_span);
            }
        }

        return method_lookup.return_ty.clone();
    }

    infer_builtin_method(checker, resolved_receiver, method, arg_types, span)
}

/// Infer type for built-in methods on primitive and collection types.
fn infer_builtin_method(
    checker: &mut TypeChecker<'_>,
    receiver_ty: &Type,
    method: Name,
    arg_types: &[Type],
    span: Span,
) -> Type {
    let method_name = checker.context.interner.lookup(method);

    // Handle special cases first
    match receiver_ty {
        Type::Var(_) => return checker.inference.ctx.fresh_var(),
        Type::Error => return Type::Error,
        _ => {}
    }

    // Handle newtype methods (unwrap)
    if let Type::Named(name) = receiver_ty {
        if let Some(entry) = checker.registries.types.get_by_name(*name) {
            if let Some(underlying) = checker
                .registries
                .types
                .get_newtype_underlying(entry.type_id)
            {
                if method_name == "unwrap" {
                    if !arg_types.is_empty() {
                        checker.push_error(
                            format!("`unwrap` takes no arguments, found {}", arg_types.len()),
                            span,
                            ori_diagnostic::ErrorCode::E2004,
                        );
                    }
                    return underlying;
                }
                // Unknown method on newtype
                checker.push_error(
                    format!(
                        "newtype `{}` has no method `{}`; use `.unwrap()` to access the underlying value",
                        checker.context.interner.lookup(*name),
                        method_name
                    ),
                    span,
                    ori_diagnostic::ErrorCode::E2002,
                );
                return Type::Error;
            }
        }
    }

    // Use the registry to check the method
    let registry = BuiltinMethodRegistry::new();
    if let Some(result) = registry.check(
        &mut checker.inference.ctx,
        checker.context.interner,
        receiver_ty,
        method_name,
        arg_types,
        span,
    ) {
        match result {
            MethodTypeResult::Ok(ty) => ty,
            MethodTypeResult::Err(e) => {
                checker.push_error(e.message, span, e.code);
                Type::Error
            }
        }
    } else {
        // No handler found for this type
        checker.push_error(
            format!(
                "type `{}` has no method `{}`",
                receiver_ty.display(checker.context.interner),
                method_name
            ),
            span,
            ori_diagnostic::ErrorCode::E2002,
        );
        Type::Error
    }
}

/// Check if a type name is a built-in type with associated functions.
///
/// These built-in types have factory methods like `Duration.from_seconds(s:)` that
/// are implemented in the compiler rather than user code.
fn is_builtin_type_with_associated_functions(name: &str) -> bool {
    matches!(name, "Duration" | "Size")
}

/// Infer the type of a built-in associated function call.
///
/// This handles associated functions on built-in types like Duration and Size,
/// e.g., `Duration.from_seconds(s: 10)`.
fn infer_builtin_associated_function(
    checker: &mut TypeChecker<'_>,
    type_name: &str,
    method: Name,
    arg_types: &[Type],
    span: Span,
) -> Type {
    let method_str = checker.context.interner.lookup(method);

    let registry = BuiltinMethodRegistry::new();
    if let Some(result) =
        registry.check_associated(&mut checker.inference.ctx, type_name, method_str, arg_types)
    {
        return match result {
            MethodTypeResult::Ok(ty) => ty,
            MethodTypeResult::Err(e) => {
                checker.push_error(e.message, span, e.code);
                Type::Error
            }
        };
    }

    checker.push_error(
        format!("type `{type_name}` has no associated function `{method_str}`"),
        span,
        ori_diagnostic::ErrorCode::E2002,
    );
    Type::Error
}
