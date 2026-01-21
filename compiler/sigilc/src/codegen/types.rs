// Type mapping for C code generation
// Converts Sigil types to C types and infers types from expressions

use super::CodeGen;
use crate::ast::*;

impl CodeGen {
    pub(super) fn type_to_c(&self, ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Named(name) => match name.as_str() {
                "int" => "int64_t".to_string(),
                "float" => "double".to_string(),
                "bool" => "bool".to_string(),
                "str" => "String".to_string(),
                "void" => "void".to_string(),
                other => other.to_string(),
            },
            TypeExpr::Optional(inner) => {
                // For now, just use pointer for optional
                format!("{}*", self.type_to_c(inner))
            }
            TypeExpr::List(inner) => {
                format!("Array_{}", self.type_to_c(inner))
            }
            _ => "void*".to_string(),
        }
    }

    pub(super) fn infer_c_type(&self, expr: &Expr) -> String {
        match expr {
            Expr::Int(_) => "int64_t".to_string(),
            Expr::Float(_) => "double".to_string(),
            Expr::String(_) => "String".to_string(),
            Expr::Bool(_) => "bool".to_string(),
            _ => "void*".to_string(),
        }
    }

    pub(super) fn is_string_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::String(_) => true,
            Expr::Config(_) => true, // Assume configs could be strings
            Expr::Binary {
                op: BinaryOp::Add,
                left,
                ..
            } => self.is_string_expr(left),
            Expr::Call { func, .. } => {
                if let Expr::Ident(name) = func.as_ref() {
                    name == "str"
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub(super) fn extract_string_literal(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}
