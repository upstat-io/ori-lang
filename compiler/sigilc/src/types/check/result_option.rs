// Result/Option type checking (Ok, Err, Some, None, Coalesce, Unwrap)

use crate::ast::{Expr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};

use super::super::compat::types_compatible;
use super::super::context::TypeContext;
use super::check_expr;

pub fn check_ok(inner: &Expr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let inner_type = check_expr(inner, ctx)?;
    // For Ok, we know the success type but error type comes from context
    if let Some(TypeExpr::Generic(name, args)) = ctx.current_return_type() {
        if name == "Result" && args.len() == 2 {
            return Ok(TypeExpr::Generic(
                "Result".to_string(),
                vec![inner_type, args[1].clone()],
            ));
        }
    }
    Ok(TypeExpr::Generic(
        "Result".to_string(),
        vec![inner_type, TypeExpr::Named("void".to_string())],
    ))
}

pub fn check_err(inner: &Expr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let inner_type = check_expr(inner, ctx)?;
    // For Err, we know the error type but success type comes from context
    if let Some(TypeExpr::Generic(name, args)) = ctx.current_return_type() {
        if name == "Result" && args.len() == 2 {
            return Ok(TypeExpr::Generic(
                "Result".to_string(),
                vec![args[0].clone(), inner_type],
            ));
        }
    }
    Ok(TypeExpr::Generic(
        "Result".to_string(),
        vec![TypeExpr::Named("void".to_string()), inner_type],
    ))
}

pub fn check_some(inner: &Expr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let inner_type = check_expr(inner, ctx)?;
    Ok(TypeExpr::Optional(Box::new(inner_type)))
}

pub fn check_none(ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    // None needs context to determine the inner type
    if let Some(TypeExpr::Optional(inner)) = ctx.current_return_type() {
        return Ok(TypeExpr::Optional(inner));
    }
    Err(
        Diagnostic::error(ErrorCode::E3005, "cannot infer type of None")
            .with_label(ctx.make_span(0..0), "type annotation needed")
            .with_help("use in a context where the optional type is clear"),
    )
}

pub fn check_coalesce(
    value: &Expr,
    default: &Expr,
    ctx: &TypeContext,
) -> DiagnosticResult<TypeExpr> {
    let value_type = check_expr(value, ctx)?;
    let default_type = check_expr(default, ctx)?;

    // value should be Optional<T>, default should be T
    if let TypeExpr::Optional(inner) = value_type {
        if types_compatible(&default_type, &inner, ctx) {
            Ok(*inner)
        } else {
            Err(Diagnostic::error(
                ErrorCode::E3001,
                format!(
                    "coalesce default type {:?} doesn't match optional inner type {:?}",
                    default_type, inner
                ),
            )
            .with_label(ctx.make_span(0..0), format!("expected {:?}", inner)))
        }
    } else {
        Err(Diagnostic::error(
            ErrorCode::E3006,
            format!("coalesce (??) requires optional type, got {:?}", value_type),
        )
        .with_label(ctx.make_span(0..0), "expected optional type"))
    }
}

pub fn check_unwrap(inner: &Expr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let inner_type = check_expr(inner, ctx)?;
    match inner_type {
        TypeExpr::Optional(t) => Ok(*t),
        TypeExpr::Generic(name, args) if name == "Result" && !args.is_empty() => {
            Ok(args[0].clone())
        }
        _ => Err(Diagnostic::error(
            ErrorCode::E3006,
            format!(
                "cannot unwrap non-optional/non-result type: {:?}",
                inner_type
            ),
        )
        .with_label(ctx.make_span(0..0), "expected optional or Result type")),
    }
}
