// Result/Option type checking (Ok, Err, Some, None, Coalesce, Unwrap)

use crate::ast::{Expr, TypeExpr};

use super::super::compat::types_compatible;
use super::super::context::TypeContext;
use super::check_expr;

pub fn check_ok(inner: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
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

pub fn check_err(inner: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
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

pub fn check_some(inner: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    let inner_type = check_expr(inner, ctx)?;
    Ok(TypeExpr::Optional(Box::new(inner_type)))
}

pub fn check_none(ctx: &TypeContext) -> Result<TypeExpr, String> {
    // None needs context to determine the inner type
    if let Some(TypeExpr::Optional(inner)) = ctx.current_return_type() {
        return Ok(TypeExpr::Optional(inner));
    }
    Err(
        "Cannot infer type of None. Use in a context where the optional type is clear."
            .to_string(),
    )
}

pub fn check_coalesce(
    value: &Expr,
    default: &Expr,
    ctx: &TypeContext,
) -> Result<TypeExpr, String> {
    let value_type = check_expr(value, ctx)?;
    let default_type = check_expr(default, ctx)?;

    // value should be Optional<T>, default should be T
    if let TypeExpr::Optional(inner) = value_type {
        if types_compatible(&default_type, &inner, ctx) {
            Ok(*inner)
        } else {
            Err(format!(
                "Coalesce default type {:?} doesn't match optional inner type {:?}",
                default_type, inner
            ))
        }
    } else {
        Err(format!(
            "Coalesce (??) requires optional type, got {:?}",
            value_type
        ))
    }
}

pub fn check_unwrap(inner: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    let inner_type = check_expr(inner, ctx)?;
    match inner_type {
        TypeExpr::Optional(t) => Ok(*t),
        TypeExpr::Generic(name, args) if name == "Result" && !args.is_empty() => {
            Ok(args[0].clone())
        }
        _ => Err(format!(
            "Cannot unwrap non-optional/non-result type: {:?}",
            inner_type
        )),
    }
}
