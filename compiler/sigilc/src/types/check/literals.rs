// Literal type checking

use crate::ast::TypeExpr;
use crate::errors::DiagnosticResult;

pub fn check_int() -> DiagnosticResult<TypeExpr> {
    Ok(TypeExpr::Named("int".to_string()))
}

pub fn check_float() -> DiagnosticResult<TypeExpr> {
    Ok(TypeExpr::Named("float".to_string()))
}

pub fn check_string() -> DiagnosticResult<TypeExpr> {
    Ok(TypeExpr::Named("str".to_string()))
}

pub fn check_bool() -> DiagnosticResult<TypeExpr> {
    Ok(TypeExpr::Named("bool".to_string()))
}

pub fn check_nil() -> DiagnosticResult<TypeExpr> {
    Ok(TypeExpr::Named("nil".to_string()))
}
