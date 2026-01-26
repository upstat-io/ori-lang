//! Call expression type inference.
//!
//! Handles function calls, method calls, and named argument calls.

use crate::ir::{Name, Span, ExprId, ExprRange, CallArgRange, ExprKind};
use crate::types::Type;
use super::super::checker::{TypeChecker, TypeCheckError};
use super::infer_expr;

/// Infer type for a function call (positional arguments).
///
/// Positional args are only allowed for:
/// - Type conversions (int, float, str, byte)
/// - Calls through function variables (param names unknowable)
///
/// Direct function calls require named arguments (use CallNamed).
pub fn infer_call(
    checker: &mut TypeChecker<'_>,
    func: ExprId,
    args: ExprRange,
    span: Span,
) -> Type {
    // Check if positional args are allowed for this call
    let func_expr = checker.context.arena.get_expr(func);
    let positional_allowed = match &func_expr.kind {
        ExprKind::Ident(name) => {
            let name_str = checker.context.interner.lookup(*name);
            // Type conversions allow positional
            if matches!(name_str, "int" | "float" | "str" | "byte") {
                true
            } else {
                // Check if this is a function definition (requires named)
                // or a variable of function type (allows positional)
                !checker.scope.function_sigs.contains_key(name)
            }
        }
        // Non-identifier callees (e.g., array[0](...), obj.field(...))
        // are always function variables, so allow positional
        _ => true,
    };

    let arg_ids = checker.context.arena.get_expr_list(args);

    if !positional_allowed && !arg_ids.is_empty() {
        checker.diagnostics.errors.push(TypeCheckError {
            message: "named arguments required for function calls (name: value)".to_string(),
            span,
            code: crate::diagnostic::ErrorCode::E2011,
        });
        return Type::Error;
    }

    let func_ty = infer_expr(checker, func);
    let arg_types: Vec<Type> = arg_ids.iter()
        .map(|id| infer_expr(checker, *id))
        .collect();

    check_call(checker, &func_ty, &arg_types, span)
}

/// Check a function call.
fn check_call(
    checker: &mut TypeChecker<'_>,
    func: &Type,
    args: &[Type],
    span: Span,
) -> Type {
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
    // Get the function name if the callee is an identifier
    let func_name = {
        let func_expr = checker.context.arena.get_expr(func);
        match &func_expr.kind {
            ExprKind::Ident(name) => Some(*name),
            _ => None,
        }
    };

    let func_ty = infer_expr(checker, func);
    let call_args = checker.context.arena.get_call_args(args);

    // Type check each argument
    let arg_types: Vec<Type> = call_args.iter()
        .map(|arg| infer_expr(checker, arg.value))
        .collect();

    // Unify with function type
    let (result, resolved_params) = match func_ty {
        Type::Function { params, ret } => {
            // Check argument count
            if params.len() != arg_types.len() {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!(
                        "expected {} arguments, found {}",
                        params.len(),
                        arg_types.len()
                    ),
                    span,
                    code: crate::diagnostic::ErrorCode::E2004,
                });
                return Type::Error;
            }

            // Unify argument types with parameter types
            for (i, (param_ty, arg_ty)) in params.iter().zip(arg_types.iter()).enumerate() {
                if let Err(e) = checker.inference.ctx.unify(param_ty, arg_ty) {
                    let arg_span = call_args[i].span;
                    checker.report_type_error(&e, arg_span);
                }
            }

            // Resolve the params after unification to get concrete types
            let resolved: Vec<Type> = params.iter()
                .map(|p| checker.inference.ctx.resolve(p))
                .collect();

            (*ret, Some(resolved))
        }
        Type::Error => (Type::Error, None),
        _ => {
            checker.diagnostics.errors.push(TypeCheckError {
                message: "expected function type for call".to_string(),
                span,
                code: crate::diagnostic::ErrorCode::E2001,
            });
            (Type::Error, None)
        }
    };

    // After unification, check trait bounds for generic functions
    if let Some(name) = func_name {
        check_generic_bounds(checker, name, resolved_params.as_deref(), span);
        // Check capability propagation
        check_capability_propagation(checker, name, span);
    }

    result
}

/// Check trait bounds for a generic function call.
///
/// After unification has resolved the generic type variables, this function
/// verifies that the concrete types satisfy the required trait bounds.
/// Delegates to `TypeChecker::check_function_bounds` for centralized bound checking.
///
/// `resolved_params` contains the resolved parameter types after unification,
/// used to determine what concrete types were passed for generic parameters.
fn check_generic_bounds(
    checker: &mut TypeChecker<'_>,
    func_name: Name,
    resolved_params: Option<&[Type]>,
    span: Span,
) {
    checker.check_function_bounds(func_name, resolved_params, span);
}

/// Check capability propagation for a function call.
///
/// When calling a function that requires capabilities, the caller must either:
/// - Declare those capabilities in its own `uses` clause, OR
/// - Provide them via `with Capability = ... in call()`
///
/// Reports E2014 if a required capability is neither declared nor provided.
fn check_capability_propagation(
    checker: &mut TypeChecker<'_>,
    func_name: Name,
    span: Span,
) {
    // Look up the called function's signature
    let Some(func_sig) = checker.scope.function_sigs.get(&func_name) else {
        // Not a known function - might be a variable or external
        return;
    };

    // Check each capability required by the called function
    for required_cap in &func_sig.capabilities.clone() {
        let is_declared = checker.scope.current_function_caps.contains(required_cap);
        let is_provided = checker.scope.provided_caps.contains(required_cap);

        if !is_declared && !is_provided {
            let func_name_str = checker.context.interner.lookup(func_name);
            let cap_name_str = checker.context.interner.lookup(*required_cap);
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "function `{func_name_str}` uses `{cap_name_str}` capability, \
                     but caller does not declare or provide it"
                ),
                span,
                code: crate::diagnostic::ErrorCode::E2014,
            });
        }
    }
}

/// Infer type for a method call (positional arguments).
///
/// Method calls require named arguments. Positional args are only allowed
/// when there are no arguments (zero-arg method calls).
pub fn infer_method_call(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    method: Name,
    args: ExprRange,
    span: Span,
) -> Type {
    // Method calls require named arguments
    let arg_ids = checker.context.arena.get_expr_list(args);
    if !arg_ids.is_empty() {
        checker.diagnostics.errors.push(TypeCheckError {
            message: "named arguments required for method calls (name: value)".to_string(),
            span,
            code: crate::diagnostic::ErrorCode::E2011,
        });
        return Type::Error;
    }

    let receiver_ty = infer_expr(checker, receiver);
    let resolved_receiver = checker.inference.ctx.resolve(&receiver_ty);

    // Type check arguments first
    let arg_types: Vec<Type> = arg_ids.iter()
        .map(|id| infer_expr(checker, *id))
        .collect();

    // Try to look up the method in the trait registry
    if let Some(method_lookup) = checker.registries.traits.lookup_method(&resolved_receiver, method) {
        // Found method - check argument count
        // The first param is 'self', so method_params includes self
        let expected_arg_count = if method_lookup.params.is_empty() {
            0
        } else {
            // Subtract 1 for self parameter
            method_lookup.params.len().saturating_sub(1)
        };

        if arg_types.len() != expected_arg_count {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "method `{}` expects {} arguments, found {}",
                    checker.context.interner.lookup(method),
                    expected_arg_count,
                    arg_types.len()
                ),
                span,
                code: crate::diagnostic::ErrorCode::E2004,
            });
            return Type::Error;
        }

        // Unify argument types with parameter types (skip self param)
        let param_types: Vec<_> = method_lookup.params.iter().skip(1).collect();
        for (i, (param_ty, arg_ty)) in param_types.iter().zip(arg_types.iter()).enumerate() {
            if let Err(e) = checker.inference.ctx.unify(param_ty, arg_ty) {
                // Use the span of the specific argument if available
                let arg_span = if i < arg_ids.len() {
                    checker.context.arena.get_expr(arg_ids[i]).span
                } else {
                    span
                };
                checker.report_type_error(&e, arg_span);
            }
        }

        return method_lookup.return_ty.clone();
    }

    // Fall back to built-in method checking for common types
    infer_builtin_method(checker, &resolved_receiver, method, &arg_types, span)
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

    // Type check arguments first
    let call_args = checker.context.arena.get_call_args(args);
    let arg_types: Vec<Type> = call_args.iter()
        .map(|arg| infer_expr(checker, arg.value))
        .collect();

    // Try to look up the method in the trait registry
    if let Some(method_lookup) = checker.registries.traits.lookup_method(&resolved_receiver, method) {
        // Found method - check argument count
        // The first param is 'self', so method_params includes self
        let expected_arg_count = if method_lookup.params.is_empty() {
            0
        } else {
            // Subtract 1 for self parameter
            method_lookup.params.len().saturating_sub(1)
        };

        if arg_types.len() != expected_arg_count {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "method `{}` expects {} arguments, found {}",
                    checker.context.interner.lookup(method),
                    expected_arg_count,
                    arg_types.len()
                ),
                span,
                code: crate::diagnostic::ErrorCode::E2004,
            });
            return Type::Error;
        }

        // Unify argument types with parameter types (skip self param)
        let param_types: Vec<_> = method_lookup.params.iter().skip(1).collect();
        for (i, (param_ty, arg_ty)) in param_types.iter().zip(arg_types.iter()).enumerate() {
            if let Err(e) = checker.inference.ctx.unify(param_ty, arg_ty) {
                // Use the span of the specific argument if available
                let arg_span = if i < call_args.len() {
                    call_args[i].span
                } else {
                    span
                };
                checker.report_type_error(&e, arg_span);
            }
        }

        return method_lookup.return_ty.clone();
    }

    // Fall back to built-in method checking for common types
    infer_builtin_method(checker, &resolved_receiver, method, &arg_types, span)
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

    match receiver_ty {
        // String methods
        Type::Str => match method_name {
            "len" => Type::Int,
            "is_empty" | "contains" | "starts_with" | "ends_with" => Type::Bool,
            "to_uppercase" | "to_lowercase" | "trim" => Type::Str,
            "split" => Type::List(Box::new(Type::Str)),
            "chars" => Type::List(Box::new(Type::Char)),
            "bytes" => Type::List(Box::new(Type::Byte)),
            _ => {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!("unknown method `{method_name}` for type `str`"),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // List methods
        Type::List(elem_ty) => match method_name {
            "len" => Type::Int,
            "is_empty" | "contains" => Type::Bool,
            "first" | "last" | "pop" | "find" => Type::Option(elem_ty.clone()),
            "push" => Type::Unit,
            "map" => {
                // map takes a function T -> U and returns [U]
                let result_elem = checker.inference.ctx.fresh_var();
                Type::List(Box::new(result_elem))
            }
            "filter" | "reverse" | "sort" => Type::List(elem_ty.clone()),
            "fold" => {
                // fold returns the accumulator type (first arg)
                if let Some(acc_ty) = arg_types.first() {
                    acc_ty.clone()
                } else {
                    checker.inference.ctx.fresh_var()
                }
            }
            _ => {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!("unknown method `{method_name}` for type `[T]`"),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Map methods
        Type::Map { key: key_ty, value: val_ty } => match method_name {
            "len" => Type::Int,
            "is_empty" | "contains_key" => Type::Bool,
            "get" | "insert" | "remove" => Type::Option(val_ty.clone()),
            "keys" => Type::List(key_ty.clone()),
            "values" => Type::List(val_ty.clone()),
            _ => {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!("unknown method `{method_name}` for type `{{K: V}}`"),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Option methods
        Type::Option(inner_ty) => match method_name {
            "is_some" | "is_none" => Type::Bool,
            "unwrap" | "unwrap_or" => (**inner_ty).clone(),
            "map" | "and_then" => {
                let result_inner = checker.inference.ctx.fresh_var();
                Type::Option(Box::new(result_inner))
            }
            "filter" => Type::Option(inner_ty.clone()),
            "ok_or" => {
                let err_ty = checker.inference.ctx.fresh_var();
                Type::Result { ok: inner_ty.clone(), err: Box::new(err_ty) }
            }
            _ => {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!("unknown method `{method_name}` for type `Option<T>`"),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Result methods
        Type::Result { ok: ok_ty, err: err_ty } => match method_name {
            "is_ok" | "is_err" => Type::Bool,
            "unwrap" | "unwrap_or" => (**ok_ty).clone(),
            "unwrap_err" => (**err_ty).clone(),
            "ok" => Type::Option(ok_ty.clone()),
            "err" => Type::Option(err_ty.clone()),
            "map" | "and_then" => {
                let result_ok = checker.inference.ctx.fresh_var();
                Type::Result { ok: Box::new(result_ok), err: err_ty.clone() }
            }
            "map_err" => {
                let result_err = checker.inference.ctx.fresh_var();
                Type::Result { ok: ok_ty.clone(), err: Box::new(result_err) }
            }
            _ => {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!("unknown method `{method_name}` for type `Result<T, E>`"),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Integer methods
        Type::Int => match method_name {
            "abs" | "min" | "max" => Type::Int,
            "to_string" => Type::Str,
            "compare" => Type::Named(checker.context.interner.intern("Ordering")),
            _ => {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!("unknown method `{method_name}` for type `int`"),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Float methods
        Type::Float => match method_name {
            "abs" | "floor" | "ceil" | "round" | "sqrt" | "min" | "max" => Type::Float,
            "to_string" => Type::Str,
            "compare" => Type::Named(checker.context.interner.intern("Ordering")),
            _ => {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!("unknown method `{method_name}` for type `float`"),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Bool methods
        Type::Bool => if method_name == "to_string" { Type::Str } else {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!("unknown method `{method_name}` for type `bool`"),
                span,
                code: crate::diagnostic::ErrorCode::E2002,
            });
            Type::Error
        },

        // Type variable - can't check methods yet, return fresh var
        Type::Var(_) => checker.inference.ctx.fresh_var(),

        // Error type - propagate
        Type::Error => Type::Error,

        // Other types - no known methods
        _ => {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "type `{}` has no method `{}`",
                    receiver_ty.display(checker.context.interner),
                    method_name
                ),
                span,
                code: crate::diagnostic::ErrorCode::E2002,
            });
            Type::Error
        }
    }
}
