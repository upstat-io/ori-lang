// Collection type checking (List, Tuple, MapLiteral, Range)

use crate::ast::{Expr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};

use super::super::compat::{is_numeric, types_compatible};
use super::super::context::TypeContext;
use super::check_expr;

pub fn check_list(exprs: &[Expr], ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    if exprs.is_empty() {
        // Empty list gets type from context (function return type)
        if let Some(TypeExpr::List(elem_type)) = ctx.current_return_type() {
            return Ok(TypeExpr::List(elem_type));
        }
        // For empty lists in other contexts, we need to infer from usage
        // For now, allow it if we're in a context where the type is clear
        Err(
            Diagnostic::error(ErrorCode::E3005, "cannot infer type of empty list")
                .with_label(ctx.make_span(0..0), "type annotation needed")
                .with_help("add a type annotation or ensure context provides the type"),
        )
    } else {
        let elem_type = check_expr(&exprs[0], ctx)?;
        // Check all elements have the same type
        for (i, e) in exprs.iter().enumerate().skip(1) {
            let t = check_expr(e, ctx)?;
            if !types_compatible(&t, &elem_type, ctx) {
                return Err(Diagnostic::error(
                    ErrorCode::E3001,
                    format!(
                        "list element {} has type {:?} but expected {:?}",
                        i, t, elem_type
                    ),
                )
                .with_label(ctx.make_span(0..0), format!("expected {:?}", elem_type)));
            }
        }
        Ok(TypeExpr::List(Box::new(elem_type)))
    }
}

pub fn check_tuple(exprs: &[Expr], ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let types: Result<Vec<_>, _> = exprs.iter().map(|e| check_expr(e, ctx)).collect();
    Ok(TypeExpr::Tuple(types?))
}

pub fn check_map_literal(
    entries: &[(Expr, Expr)],
    ctx: &TypeContext,
) -> DiagnosticResult<TypeExpr> {
    if entries.is_empty() {
        return Err(
            Diagnostic::error(ErrorCode::E3005, "cannot infer type of empty map literal")
                .with_label(ctx.make_span(0..0), "type annotation needed"),
        );
    }
    let (key, value) = &entries[0];
    let key_type = check_expr(key, ctx)?;
    let value_type = check_expr(value, ctx)?;
    Ok(TypeExpr::Map(Box::new(key_type), Box::new(value_type)))
}

pub fn check_range(start: &Expr, end: &Expr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let start_type = check_expr(start, ctx)?;
    let end_type = check_expr(end, ctx)?;
    if !is_numeric(&start_type) || !is_numeric(&end_type) {
        return Err(Diagnostic::error(
            ErrorCode::E3001,
            format!(
                "range bounds must be numeric, got {:?}..{:?}",
                start_type, end_type
            ),
        )
        .with_label(ctx.make_span(0..0), "expected int or float"));
    }
    // Range is a special type that can be iterated
    Ok(TypeExpr::Named("Range".to_string()))
}
