//! Binary and unary operation type checking.

use super::super::infer_expr;
use crate::checker::TypeChecker;
use crate::operators::{check_binary_operation, TypeOpResult};
use ori_ir::{BinaryOp, ExprId, Span, UnaryOp};
use ori_types::Type;

/// Infer type for a binary operation.
pub fn infer_binary(
    checker: &mut TypeChecker<'_>,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
    span: Span,
) -> Type {
    let left_ty = infer_expr(checker, left);
    let right_ty = infer_expr(checker, right);
    check_binary_op(checker, op, &left_ty, &right_ty, span)
}

/// Check a binary operation.
fn check_binary_op(
    checker: &mut TypeChecker<'_>,
    op: BinaryOp,
    left: &Type,
    right: &Type,
    span: Span,
) -> Type {
    match check_binary_operation(
        &mut checker.inference.ctx,
        checker.context.interner,
        op,
        left,
        right,
        span,
    ) {
        TypeOpResult::Ok(ty) => ty,
        TypeOpResult::Err(e) => {
            checker.push_error(e.message, span, e.code);
            Type::Error
        }
    }
}

/// Infer type for a unary operation.
pub fn infer_unary(
    checker: &mut TypeChecker<'_>,
    op: UnaryOp,
    operand: ExprId,
    span: Span,
) -> Type {
    let operand_ty = infer_expr(checker, operand);
    check_unary_op(checker, op, &operand_ty, span)
}

/// Check a unary operation.
fn check_unary_op(checker: &mut TypeChecker<'_>, op: UnaryOp, operand: &Type, span: Span) -> Type {
    match op {
        UnaryOp::Neg => {
            let resolved = checker.inference.ctx.resolve(operand);
            match resolved {
                Type::Int | Type::Float | Type::Var(_) => resolved,
                _ => {
                    checker.push_error(
                        format!(
                            "cannot negate `{}`: negation requires a numeric type (int or float)",
                            operand.display(checker.context.interner)
                        ),
                        span,
                        ori_diagnostic::ErrorCode::E2001,
                    );
                    Type::Error
                }
            }
        }
        UnaryOp::Not => {
            if let Err(e) = checker.inference.ctx.unify(operand, &Type::Bool) {
                checker.report_type_error(&e, span);
            }
            Type::Bool
        }
        UnaryOp::BitNot => {
            if let Err(e) = checker.inference.ctx.unify(operand, &Type::Int) {
                checker.report_type_error(&e, span);
            }
            Type::Int
        }
        UnaryOp::Try => {
            let ok_ty = checker.inference.ctx.fresh_var();
            let err_ty = checker.inference.ctx.fresh_var();
            let result_ty = checker.inference.ctx.make_result(ok_ty.clone(), err_ty);
            if let Err(e) = checker.inference.ctx.unify(operand, &result_ty) {
                checker.report_type_error(&e, span);
            }
            checker.inference.ctx.resolve(&ok_ty)
        }
    }
}
