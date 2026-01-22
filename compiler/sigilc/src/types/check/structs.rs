// Struct and field access type checking

use crate::ast::{Expr, TypeDefKind, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};

use super::super::context::TypeContext;
use super::check_expr;

pub fn check_struct(
    name: &str,
    fields: &[(String, Expr)],
    ctx: &TypeContext,
) -> DiagnosticResult<TypeExpr> {
    // Check field expressions
    for (_, expr) in fields {
        check_expr(expr, ctx)?;
    }
    Ok(TypeExpr::Named(name.to_string()))
}

pub fn check_field(expr: &Expr, field: &str, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let expr_type = check_expr(expr, ctx)?;
    match &expr_type {
        // Anonymous record type - look up field directly
        TypeExpr::Record(fields) => {
            if let Some((_, field_type)) = fields.iter().find(|(n, _)| n == field) {
                Ok(field_type.clone())
            } else {
                Err(
                    Diagnostic::error(ErrorCode::E3002, format!("record has no field '{}'", field))
                        .with_label(ctx.make_span(0..0), "field not found"),
                )
            }
        }
        // Named struct type - look up struct definition
        TypeExpr::Named(type_name) => {
            if let Some(type_def) = ctx.lookup_type(type_name) {
                if let TypeDefKind::Struct(struct_fields) = &type_def.kind {
                    if let Some(f) = struct_fields.iter().find(|f| f.name == field) {
                        Ok(f.ty.clone())
                    } else {
                        Err(Diagnostic::error(
                            ErrorCode::E3002,
                            format!("struct '{}' has no field '{}'", type_name, field),
                        )
                        .with_label(ctx.make_span(0..0), "field not found"))
                    }
                } else {
                    Err(Diagnostic::error(
                        ErrorCode::E3006,
                        format!("type '{}' is not a struct", type_name),
                    )
                    .with_label(ctx.make_span(0..0), "expected struct type"))
                }
            } else {
                Err(Diagnostic::error(
                    ErrorCode::E3006,
                    format!("cannot access field '{}' on type {:?}", field, expr_type),
                )
                .with_label(ctx.make_span(0..0), "type does not support field access"))
            }
        }
        _ => Err(Diagnostic::error(
            ErrorCode::E3006,
            format!("cannot access field '{}' on type {:?}", field, expr_type),
        )
        .with_label(ctx.make_span(0..0), "type does not support field access")),
    }
}

pub fn check_index(arr: &Expr, _index: &Expr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    let arr_type = check_expr(arr, ctx)?;
    if let TypeExpr::List(elem_type) = arr_type {
        Ok(*elem_type)
    } else if let TypeExpr::Named(name) = &arr_type {
        if name == "str" {
            Ok(TypeExpr::Named("str".to_string()))
        } else {
            Err(Diagnostic::error(
                ErrorCode::E3006,
                format!("cannot index into type {:?}", arr_type),
            )
            .with_label(ctx.make_span(0..0), "type does not support indexing"))
        }
    } else {
        Err(Diagnostic::error(
            ErrorCode::E3006,
            format!("cannot index into type {:?}", arr_type),
        )
        .with_label(ctx.make_span(0..0), "type does not support indexing"))
    }
}
