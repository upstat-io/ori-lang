// Collection type checking (List, Tuple, MapLiteral, Range)

use crate::ast::{Expr, TypeExpr};

use super::super::compat::{is_numeric, types_compatible};
use super::super::context::TypeContext;
use super::check_expr;

pub fn check_list(exprs: &[Expr], ctx: &TypeContext) -> Result<TypeExpr, String> {
    if exprs.is_empty() {
        // Empty list gets type from context (function return type)
        if let Some(TypeExpr::List(elem_type)) = ctx.current_return_type() {
            return Ok(TypeExpr::List(elem_type));
        }
        // For empty lists in other contexts, we need to infer from usage
        // For now, allow it if we're in a context where the type is clear
        Err("Cannot infer type of empty list. Add a type annotation or ensure context provides the type.".to_string())
    } else {
        let elem_type = check_expr(&exprs[0], ctx)?;
        // Check all elements have the same type
        for (i, e) in exprs.iter().enumerate().skip(1) {
            let t = check_expr(e, ctx)?;
            if !types_compatible(&t, &elem_type, ctx) {
                return Err(format!(
                    "List element {} has type {:?} but expected {:?}",
                    i, t, elem_type
                ));
            }
        }
        Ok(TypeExpr::List(Box::new(elem_type)))
    }
}

pub fn check_tuple(exprs: &[Expr], ctx: &TypeContext) -> Result<TypeExpr, String> {
    let types: Result<Vec<_>, _> = exprs.iter().map(|e| check_expr(e, ctx)).collect();
    Ok(TypeExpr::Tuple(types?))
}

pub fn check_map_literal(entries: &[(Expr, Expr)], ctx: &TypeContext) -> Result<TypeExpr, String> {
    if entries.is_empty() {
        return Err("Cannot infer type of empty map literal".to_string());
    }
    let (key, value) = &entries[0];
    let key_type = check_expr(key, ctx)?;
    let value_type = check_expr(value, ctx)?;
    Ok(TypeExpr::Map(Box::new(key_type), Box::new(value_type)))
}

pub fn check_range(start: &Expr, end: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    let start_type = check_expr(start, ctx)?;
    let end_type = check_expr(end, ctx)?;
    if !is_numeric(&start_type) || !is_numeric(&end_type) {
        return Err(format!(
            "Range bounds must be numeric, got {:?}..{:?}",
            start_type, end_type
        ));
    }
    // Range is a special type that can be iterated
    Ok(TypeExpr::Named("Range".to_string()))
}
