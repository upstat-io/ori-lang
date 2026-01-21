// Struct and field access type checking

use crate::ast::{Expr, TypeDefKind, TypeExpr};

use super::super::context::TypeContext;
use super::check_expr;

pub fn check_struct(
    name: &str,
    fields: &[(String, Expr)],
    ctx: &TypeContext,
) -> Result<TypeExpr, String> {
    // Check field expressions
    for (_, expr) in fields {
        check_expr(expr, ctx)?;
    }
    Ok(TypeExpr::Named(name.to_string()))
}

pub fn check_field(expr: &Expr, field: &str, ctx: &TypeContext) -> Result<TypeExpr, String> {
    let expr_type = check_expr(expr, ctx)?;
    match &expr_type {
        // Anonymous record type - look up field directly
        TypeExpr::Record(fields) => {
            if let Some((_, field_type)) = fields.iter().find(|(n, _)| n == field) {
                Ok(field_type.clone())
            } else {
                Err(format!("Record has no field '{}'", field))
            }
        }
        // Named struct type - look up struct definition
        TypeExpr::Named(type_name) => {
            if let Some(type_def) = ctx.lookup_type(type_name) {
                if let TypeDefKind::Struct(struct_fields) = &type_def.kind {
                    if let Some(f) = struct_fields.iter().find(|f| f.name == field) {
                        Ok(f.ty.clone())
                    } else {
                        Err(format!("Struct '{}' has no field '{}'", type_name, field))
                    }
                } else {
                    Err(format!("Type '{}' is not a struct", type_name))
                }
            } else {
                Err(format!(
                    "Cannot access field '{}' on type {:?}",
                    field, expr_type
                ))
            }
        }
        _ => Err(format!(
            "Cannot access field '{}' on type {:?}",
            field, expr_type
        )),
    }
}

pub fn check_index(arr: &Expr, _index: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    let arr_type = check_expr(arr, ctx)?;
    if let TypeExpr::List(elem_type) = arr_type {
        Ok(*elem_type)
    } else if let TypeExpr::Named(name) = &arr_type {
        if name == "str" {
            Ok(TypeExpr::Named("str".to_string()))
        } else {
            Err(format!("Cannot index into type {:?}", arr_type))
        }
    } else {
        Err(format!("Cannot index into type {:?}", arr_type))
    }
}
