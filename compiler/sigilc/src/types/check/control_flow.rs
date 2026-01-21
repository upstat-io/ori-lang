// Control flow type checking (If, Match, Block, For)

use crate::ast::{Expr, MatchExpr, TypeExpr};

use super::super::compat::{get_iterable_element_type, types_compatible};
use super::super::context::TypeContext;
use super::{check_block_expr, check_expr};

pub fn check_if(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    ctx: &TypeContext,
) -> Result<TypeExpr, String> {
    let cond_type = check_expr(condition, ctx)?;
    if !types_compatible(&cond_type, &TypeExpr::Named("bool".to_string()), ctx) {
        return Err(format!("If condition must be bool, got {:?}", cond_type));
    }

    let then_type = check_expr(then_branch, ctx)?;

    if let Some(else_expr) = else_branch {
        let else_type = check_expr(else_expr, ctx)?;
        if !types_compatible(&then_type, &else_type, ctx) {
            return Err(format!(
                "If branches have different types: then={:?}, else={:?}",
                then_type, else_type
            ));
        }
    }
    Ok(then_type)
}

pub fn check_match(m: &MatchExpr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    // Check scrutinee
    check_expr(&m.scrutinee, ctx)?;

    // All arms must have the same type
    if m.arms.is_empty() {
        return Err("Match expression has no arms".to_string());
    }

    let first_type = check_expr(&m.arms[0].body, ctx)?;
    for (i, arm) in m.arms.iter().enumerate().skip(1) {
        let arm_type = check_expr(&arm.body, ctx)?;
        if !types_compatible(&arm_type, &first_type, ctx) {
            return Err(format!(
                "Match arm {} has type {:?} but expected {:?}",
                i, arm_type, first_type
            ));
        }
    }
    Ok(first_type)
}

pub fn check_block(exprs: &[Expr], ctx: &TypeContext) -> Result<TypeExpr, String> {
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
) -> Result<TypeExpr, String> {
    let iter_type = check_expr(iterator, ctx)?;
    let elem_type = get_iterable_element_type(&iter_type)
        .map_err(|_| format!("Cannot iterate over {:?}", iter_type))?;

    // Create a child context with the binding
    let child_ctx = TypeContext::child_with_locals(ctx, |locals| {
        locals.insert(binding.to_string(), elem_type);
    });
    check_expr(body, &child_ctx)?;
    Ok(TypeExpr::Named("void".to_string()))
}
