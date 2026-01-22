// Control flow type checking (If, Match, Block, For)

use crate::ast::{Expr, MatchExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};

use super::super::compat::{get_iterable_element_type, types_compatible};
use super::super::context::TypeContext;
use super::{check_block_expr, check_expr};

pub fn check_if(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    ctx: &TypeContext,
) -> DiagnosticResult<TypeExpr> {
    let cond_type = check_expr(condition, ctx)?;
    if !types_compatible(&cond_type, &TypeExpr::Named("bool".to_string()), ctx) {
        return Err(Diagnostic::error(
            ErrorCode::E3001,
            format!("if condition must be bool, got {:?}", cond_type),
        )
        .with_label(ctx.make_span(0..0), "expected bool"));
    }

    let then_type = check_expr(then_branch, ctx)?;

    if let Some(else_expr) = else_branch {
        let else_type = check_expr(else_expr, ctx)?;
        if !types_compatible(&then_type, &else_type, ctx) {
            return Err(Diagnostic::error(
                ErrorCode::E3001,
                format!(
                    "if branches have different types: then={:?}, else={:?}",
                    then_type, else_type
                ),
            )
            .with_label(ctx.make_span(0..0), "branch type mismatch"));
        }
    }
    Ok(then_type)
}

pub fn check_match(m: &MatchExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    // Check scrutinee
    check_expr(&m.scrutinee, ctx)?;

    // All arms must have the same type
    if m.arms.is_empty() {
        return Err(
            Diagnostic::error(ErrorCode::E3009, "match expression has no arms")
                .with_label(ctx.make_span(0..0), "add at least one arm"),
        );
    }

    let first_type = check_expr(&m.arms[0].body, ctx)?;
    for (i, arm) in m.arms.iter().enumerate().skip(1) {
        let arm_type = check_expr(&arm.body, ctx)?;
        if !types_compatible(&arm_type, &first_type, ctx) {
            return Err(Diagnostic::error(
                ErrorCode::E3001,
                format!(
                    "match arm {} has type {:?} but expected {:?}",
                    i, arm_type, first_type
                ),
            )
            .with_label(ctx.make_span(0..0), format!("expected {:?}", first_type)));
        }
    }
    Ok(first_type)
}

pub fn check_block(exprs: &[Expr], ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    if exprs.is_empty() {
        return Ok(TypeExpr::Named("void".to_string()));
    }
    // Create a child context for block scope
    let mut block_ctx = ctx.child();
    // Check all expressions, tracking assignments
    let mut last_type = TypeExpr::Named("void".to_string());
    for expr in exprs.iter() {
        last_type = check_block_expr(expr, &mut block_ctx)?;
    }
    Ok(last_type)
}

pub fn check_for(
    binding: &str,
    iterator: &Expr,
    body: &Expr,
    ctx: &TypeContext,
) -> DiagnosticResult<TypeExpr> {
    let iter_type = check_expr(iterator, ctx)?;
    let elem_type = get_iterable_element_type(&iter_type).map_err(|_| {
        Diagnostic::error(
            ErrorCode::E3006,
            format!("cannot iterate over {:?}", iter_type),
        )
        .with_label(ctx.make_span(0..0), "expected iterable type")
    })?;

    // Create a child context with the binding (loop bindings are immutable)
    let child_ctx = ctx.child_with_locals(|locals| {
        locals.insert(
            binding.to_string(),
            crate::types::LocalBinding::immutable(elem_type),
        );
    });
    check_expr(body, &child_ctx)?;
    Ok(TypeExpr::Named("void".to_string()))
}
