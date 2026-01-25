//! Call expression type inference.
//!
//! Handles function calls, method calls, and named argument calls.

use crate::ir::{Name, Span, ExprId, ExprRange, CallArgRange, ExprKind};
use crate::types::Type;
use super::super::checker::{TypeChecker, TypeCheckError};
use super::infer_expr;

/// Infer type for a function call.
pub fn infer_call(
    checker: &mut TypeChecker<'_>,
    func: ExprId,
    args: ExprRange,
    span: Span,
) -> Type {
    let func_ty = infer_expr(checker, func);
    let arg_ids = checker.arena.get_expr_list(args);
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
    let result = checker.ctx.fresh_var();
    let expected = Type::Function {
        params: args.to_vec(),
        ret: Box::new(result.clone()),
    };

    if let Err(e) = checker.ctx.unify(func, &expected) {
        checker.report_type_error(e, span);
        return Type::Error;
    }

    checker.ctx.resolve(&result)
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
        let func_expr = checker.arena.get_expr(func);
        match &func_expr.kind {
            ExprKind::Ident(name) => Some(*name),
            _ => None,
        }
    };

    let func_ty = infer_expr(checker, func);
    let call_args = checker.arena.get_call_args(args);

    // Type check each argument
    let arg_types: Vec<Type> = call_args.iter()
        .map(|arg| infer_expr(checker, arg.value))
        .collect();

    // Unify with function type
    let result = match func_ty {
        Type::Function { params, ret } => {
            // Check argument count
            if params.len() != arg_types.len() {
                checker.errors.push(TypeCheckError {
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
                if let Err(e) = checker.ctx.unify(param_ty, arg_ty) {
                    let arg_span = call_args[i].span;
                    checker.report_type_error(e, arg_span);
                }
            }

            *ret
        }
        Type::Error => Type::Error,
        _ => {
            checker.errors.push(TypeCheckError {
                message: "expected function type for call".to_string(),
                span,
                code: crate::diagnostic::ErrorCode::E2001,
            });
            Type::Error
        }
    };

    // After unification, check trait bounds for generic functions
    if let Some(name) = func_name {
        check_generic_bounds(checker, name, span);
    }

    result
}

/// Check trait bounds for a generic function call.
///
/// After unification has resolved the generic type variables, this function
/// verifies that the concrete types satisfy the required trait bounds.
/// Delegates to TypeChecker::check_function_bounds for centralized bound checking.
fn check_generic_bounds(
    checker: &mut TypeChecker<'_>,
    func_name: Name,
    span: Span,
) {
    checker.check_function_bounds(func_name, span);
}

/// Infer type for a method call.
pub fn infer_method_call(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    method: Name,
    args: ExprRange,
    span: Span,
) -> Type {
    let receiver_ty = infer_expr(checker, receiver);
    let resolved_receiver = checker.ctx.resolve(&receiver_ty);

    // Type check arguments first
    let arg_ids = checker.arena.get_expr_list(args);
    let arg_types: Vec<Type> = arg_ids.iter()
        .map(|id| infer_expr(checker, *id))
        .collect();

    // Try to look up the method in the trait registry
    if let Some(method_lookup) = checker.trait_registry.lookup_method(&resolved_receiver, method) {
        // Found method - check argument count
        // The first param is 'self', so method_params includes self
        let expected_arg_count = if method_lookup.params.is_empty() {
            0
        } else {
            // Subtract 1 for self parameter
            method_lookup.params.len().saturating_sub(1)
        };

        if arg_types.len() != expected_arg_count {
            checker.errors.push(TypeCheckError {
                message: format!(
                    "method `{}` expects {} arguments, found {}",
                    checker.interner.lookup(method),
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
            if let Err(e) = checker.ctx.unify(param_ty, arg_ty) {
                // Use the span of the specific argument if available
                let arg_span = if i < arg_ids.len() {
                    checker.arena.get_expr(arg_ids[i]).span
                } else {
                    span
                };
                checker.report_type_error(e, arg_span);
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
    let method_name = checker.interner.lookup(method);

    match receiver_ty {
        // String methods
        Type::Str => match method_name {
            "len" => Type::Int,
            "is_empty" => Type::Bool,
            "to_uppercase" => Type::Str,
            "to_lowercase" => Type::Str,
            "trim" => Type::Str,
            "contains" => Type::Bool,
            "starts_with" => Type::Bool,
            "ends_with" => Type::Bool,
            "split" => Type::List(Box::new(Type::Str)),
            "chars" => Type::List(Box::new(Type::Char)),
            "bytes" => Type::List(Box::new(Type::Byte)),
            _ => {
                checker.errors.push(TypeCheckError {
                    message: format!("unknown method `{}` for type `str`", method_name),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // List methods
        Type::List(elem_ty) => match method_name {
            "len" => Type::Int,
            "is_empty" => Type::Bool,
            "first" => Type::Option(elem_ty.clone()),
            "last" => Type::Option(elem_ty.clone()),
            "push" => Type::Unit,
            "pop" => Type::Option(elem_ty.clone()),
            "contains" => Type::Bool,
            "map" => {
                // map takes a function T -> U and returns [U]
                let result_elem = checker.ctx.fresh_var();
                Type::List(Box::new(result_elem))
            }
            "filter" => Type::List(elem_ty.clone()),
            "find" => Type::Option(elem_ty.clone()),
            "fold" => {
                // fold returns the accumulator type (first arg)
                if let Some(acc_ty) = arg_types.first() {
                    acc_ty.clone()
                } else {
                    checker.ctx.fresh_var()
                }
            }
            "reverse" => Type::List(elem_ty.clone()),
            "sort" => Type::List(elem_ty.clone()),
            _ => {
                checker.errors.push(TypeCheckError {
                    message: format!("unknown method `{}` for type `[T]`", method_name),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Map methods
        Type::Map { key: key_ty, value: val_ty } => match method_name {
            "len" => Type::Int,
            "is_empty" => Type::Bool,
            "contains_key" => Type::Bool,
            "get" => Type::Option(val_ty.clone()),
            "insert" => Type::Option(val_ty.clone()),
            "remove" => Type::Option(val_ty.clone()),
            "keys" => Type::List(key_ty.clone()),
            "values" => Type::List(val_ty.clone()),
            _ => {
                checker.errors.push(TypeCheckError {
                    message: format!("unknown method `{}` for type `{{K: V}}`", method_name),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Option methods
        Type::Option(inner_ty) => match method_name {
            "is_some" => Type::Bool,
            "is_none" => Type::Bool,
            "unwrap" => (**inner_ty).clone(),
            "unwrap_or" => (**inner_ty).clone(),
            "map" => {
                let result_inner = checker.ctx.fresh_var();
                Type::Option(Box::new(result_inner))
            }
            "and_then" => {
                let result_inner = checker.ctx.fresh_var();
                Type::Option(Box::new(result_inner))
            }
            "filter" => Type::Option(inner_ty.clone()),
            "ok_or" => {
                let err_ty = checker.ctx.fresh_var();
                Type::Result { ok: inner_ty.clone(), err: Box::new(err_ty) }
            }
            _ => {
                checker.errors.push(TypeCheckError {
                    message: format!("unknown method `{}` for type `Option<T>`", method_name),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Result methods
        Type::Result { ok: ok_ty, err: err_ty } => match method_name {
            "is_ok" => Type::Bool,
            "is_err" => Type::Bool,
            "unwrap" => (**ok_ty).clone(),
            "unwrap_or" => (**ok_ty).clone(),
            "unwrap_err" => (**err_ty).clone(),
            "ok" => Type::Option(ok_ty.clone()),
            "err" => Type::Option(err_ty.clone()),
            "map" => {
                let result_ok = checker.ctx.fresh_var();
                Type::Result { ok: Box::new(result_ok), err: err_ty.clone() }
            }
            "map_err" => {
                let result_err = checker.ctx.fresh_var();
                Type::Result { ok: ok_ty.clone(), err: Box::new(result_err) }
            }
            "and_then" => {
                let result_ok = checker.ctx.fresh_var();
                Type::Result { ok: Box::new(result_ok), err: err_ty.clone() }
            }
            _ => {
                checker.errors.push(TypeCheckError {
                    message: format!("unknown method `{}` for type `Result<T, E>`", method_name),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Integer methods
        Type::Int => match method_name {
            "abs" => Type::Int,
            "to_string" => Type::Str,
            "compare" => Type::Named(checker.interner.intern("Ordering")),
            "min" => Type::Int,
            "max" => Type::Int,
            _ => {
                checker.errors.push(TypeCheckError {
                    message: format!("unknown method `{}` for type `int`", method_name),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Float methods
        Type::Float => match method_name {
            "abs" => Type::Float,
            "floor" => Type::Float,
            "ceil" => Type::Float,
            "round" => Type::Float,
            "sqrt" => Type::Float,
            "to_string" => Type::Str,
            "compare" => Type::Named(checker.interner.intern("Ordering")),
            "min" => Type::Float,
            "max" => Type::Float,
            _ => {
                checker.errors.push(TypeCheckError {
                    message: format!("unknown method `{}` for type `float`", method_name),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Bool methods
        Type::Bool => match method_name {
            "to_string" => Type::Str,
            _ => {
                checker.errors.push(TypeCheckError {
                    message: format!("unknown method `{}` for type `bool`", method_name),
                    span,
                    code: crate::diagnostic::ErrorCode::E2002,
                });
                Type::Error
            }
        },

        // Type variable - can't check methods yet, return fresh var
        Type::Var(_) => checker.ctx.fresh_var(),

        // Error type - propagate
        Type::Error => Type::Error,

        // Other types - no known methods
        _ => {
            checker.errors.push(TypeCheckError {
                message: format!(
                    "type `{}` has no method `{}`",
                    receiver_ty.display(checker.interner),
                    method_name
                ),
                span,
                code: crate::diagnostic::ErrorCode::E2002,
            });
            Type::Error
        }
    }
}
