// Literal type checking

use crate::ast::TypeExpr;

pub fn check_int() -> Result<TypeExpr, String> {
    Ok(TypeExpr::Named("int".to_string()))
}

pub fn check_float() -> Result<TypeExpr, String> {
    Ok(TypeExpr::Named("float".to_string()))
}

pub fn check_string() -> Result<TypeExpr, String> {
    Ok(TypeExpr::Named("str".to_string()))
}

pub fn check_bool() -> Result<TypeExpr, String> {
    Ok(TypeExpr::Named("bool".to_string()))
}

pub fn check_nil() -> Result<TypeExpr, String> {
    Ok(TypeExpr::Named("nil".to_string()))
}
