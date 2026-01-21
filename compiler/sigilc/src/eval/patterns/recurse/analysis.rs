// Analysis utilities for recurse pattern
// Contains helper functions for analyzing expressions

use crate::ast::*;

/// Check if an expression contains a self() call
pub fn contains_self_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call { func, args } => {
            if let Expr::Ident(name) = func.as_ref() {
                if name == "self" {
                    return true;
                }
            }
            args.iter().any(contains_self_call)
        }
        Expr::Binary { left, right, .. } => contains_self_call(left) || contains_self_call(right),
        Expr::Unary { operand, .. } => contains_self_call(operand),
        _ => false,
    }
}
