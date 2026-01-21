// Type compatibility checking for Sigil
// Determines if types are compatible for assignments, returns, etc.

use super::TypeContext;
use crate::ast::{Expr, TypeExpr};

/// Check if two types are compatible
pub fn types_compatible(actual: &TypeExpr, expected: &TypeExpr, _ctx: &TypeContext) -> bool {
    match (actual, expected) {
        // 'any' is compatible with everything (for builtins)
        (_, TypeExpr::Named(e)) if e == "any" => true,
        (TypeExpr::Named(a), _) if a == "any" => true,

        // Type parameters (single uppercase letter) match anything
        // This handles generic function definitions like len: [T] -> int
        (_, TypeExpr::Named(e)) if is_type_parameter(e) => true,
        (TypeExpr::Named(a), _) if is_type_parameter(a) => true,

        // void is compatible with nil (nil is the value, void is the type)
        (TypeExpr::Named(a), TypeExpr::Named(e))
            if (a == "void" || a == "nil") && (e == "void" || e == "nil") =>
        {
            true
        }

        // Named types must match exactly
        (TypeExpr::Named(a), TypeExpr::Named(e)) => a == e,

        (TypeExpr::Optional(a), TypeExpr::Optional(e)) => types_compatible(a, e, _ctx),
        (TypeExpr::List(a), TypeExpr::List(e)) => types_compatible(a, e, _ctx),
        (TypeExpr::Generic(na, aa), TypeExpr::Generic(ne, ae)) => {
            na == ne
                && aa.len() == ae.len()
                && aa
                    .iter()
                    .zip(ae.iter())
                    .all(|(a, e)| types_compatible(a, e, _ctx))
        }
        (TypeExpr::Function(a_param, a_ret), TypeExpr::Function(e_param, e_ret)) => {
            types_compatible(a_param, e_param, _ctx) && types_compatible(a_ret, e_ret, _ctx)
        }
        (TypeExpr::Tuple(a), TypeExpr::Tuple(e)) => {
            a.len() == e.len()
                && a.iter()
                    .zip(e.iter())
                    .all(|(a, e)| types_compatible(a, e, _ctx))
        }
        (TypeExpr::Map(ak, av), TypeExpr::Map(ek, ev)) => {
            types_compatible(ak, ek, _ctx) && types_compatible(av, ev, _ctx)
        }
        // Record types are compatible if they have the same fields with compatible types
        (TypeExpr::Record(a_fields), TypeExpr::Record(e_fields)) => {
            if a_fields.len() != e_fields.len() {
                return false;
            }
            for (a_name, a_type) in a_fields {
                if let Some((_, e_type)) = e_fields.iter().find(|(n, _)| n == a_name) {
                    if !types_compatible(a_type, e_type, _ctx) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

/// Check if a type is numeric (int or float)
pub fn is_numeric(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Named(n) if n == "int" || n == "float")
}

/// Check if a type name is a type parameter (single uppercase letter like T, E, K, V)
pub fn is_type_parameter(name: &str) -> bool {
    name.len() == 1
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_uppercase())
            .unwrap_or(false)
}

/// Infer the type of a simple expression (for config values without type annotations)
pub fn infer_type(expr: &Expr) -> Result<TypeExpr, String> {
    match expr {
        Expr::Int(_) => Ok(TypeExpr::Named("int".to_string())),
        Expr::Float(_) => Ok(TypeExpr::Named("float".to_string())),
        Expr::String(_) => Ok(TypeExpr::Named("str".to_string())),
        Expr::Bool(_) => Ok(TypeExpr::Named("bool".to_string())),
        Expr::List(items) if !items.is_empty() => {
            let elem_type = infer_type(&items[0])?;
            Ok(TypeExpr::List(Box::new(elem_type)))
        }
        _ => Err("Cannot infer type - explicit type annotation required".to_string()),
    }
}
