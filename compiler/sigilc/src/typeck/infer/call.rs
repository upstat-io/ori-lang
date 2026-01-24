//! Call expression type inference.
//!
//! Handles function calls, method calls, and named argument calls.

use crate::ir::{Name, Span, ExprId, ExprRange, CallArgRange};
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
    let func_ty = infer_expr(checker, func);
    let call_args = checker.arena.get_call_args(args);

    // Type check each argument
    let arg_types: Vec<Type> = call_args.iter()
        .map(|arg| infer_expr(checker, arg.value))
        .collect();

    // Unify with function type
    match func_ty {
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
    }
}

/// Infer type for a method call.
pub fn infer_method_call(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    _method: Name,
    args: ExprRange,
    _span: Span,
) -> Type {
    let _receiver_ty = infer_expr(checker, receiver);
    let arg_ids = checker.arena.get_expr_list(args);
    for id in arg_ids {
        infer_expr(checker, *id);
    }
    // TODO: implement proper method resolution
    checker.ctx.fresh_var()
}
