// Operator type checking (Binary, Unary)

use crate::ast::{BinaryOp, Expr, TypeExpr, UnaryOp};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};

use super::super::compat::{is_numeric, types_compatible};
use super::super::context::TypeContext;
use super::{check_expr, check_expr_with_hint};

pub fn check_binary(
    op: &BinaryOp,
    left: &Expr,
    right: &Expr,
    ctx: &TypeContext,
) -> DiagnosticResult<TypeExpr> {
    // For equality/comparison, use left type as hint for right
    match op {
        BinaryOp::Eq | BinaryOp::NotEq => {
            let left_type = check_expr(left, ctx)?;
            // Use left type as hint for right side (helps with empty lists)
            check_expr_with_hint(right, ctx, Some(&left_type))?;
            Ok(TypeExpr::Named("bool".to_string()))
        }
        BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
            check_expr(left, ctx)?;
            check_expr(right, ctx)?;
            Ok(TypeExpr::Named("bool".to_string()))
        }
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::IntDiv
        | BinaryOp::Mod => {
            let left_type = check_expr(left, ctx)?;
            let right_type = check_expr(right, ctx)?;
            if is_numeric(&left_type) && is_numeric(&right_type) {
                Ok(left_type)
            } else if matches!((&left_type, op), (TypeExpr::Named(n), BinaryOp::Add) if n == "str")
            {
                Ok(TypeExpr::Named("str".to_string()))
            } else if matches!(
                (&left_type, &right_type, op),
                (TypeExpr::List(_), TypeExpr::List(_), BinaryOp::Add)
            ) {
                Ok(left_type)
            } else {
                Err(Diagnostic::error(
                    ErrorCode::E3006,
                    format!(
                        "cannot apply {:?} to {:?} and {:?}",
                        op, left_type, right_type
                    ),
                )
                .with_label(ctx.make_span(0..0), "invalid operation"))
            }
        }
        BinaryOp::And | BinaryOp::Or => {
            check_expr(left, ctx)?;
            check_expr(right, ctx)?;
            Ok(TypeExpr::Named("bool".to_string()))
        }
        BinaryOp::Pipe => {
            check_expr(left, ctx)?;
            let right_type = check_expr(right, ctx)?;
            Ok(right_type)
        }
    }
}

pub fn check_unary(op: &UnaryOp, operand: &Expr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let operand_type = check_expr(operand, ctx)?;
    match op {
        UnaryOp::Neg => {
            if is_numeric(&operand_type) {
                Ok(operand_type)
            } else {
                Err(Diagnostic::error(
                    ErrorCode::E3006,
                    format!("cannot negate non-numeric type: {:?}", operand_type),
                )
                .with_label(ctx.make_span(0..0), "expected numeric type"))
            }
        }
        UnaryOp::Not => {
            if types_compatible(&operand_type, &TypeExpr::Named("bool".to_string()), ctx) {
                Ok(TypeExpr::Named("bool".to_string()))
            } else {
                Err(Diagnostic::error(
                    ErrorCode::E3006,
                    format!("cannot apply ! to non-bool type: {:?}", operand_type),
                )
                .with_label(ctx.make_span(0..0), "expected bool"))
            }
        }
    }
}
