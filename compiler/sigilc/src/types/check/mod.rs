// Expression type checking for Sigil
// Dispatcher module - handles all expression type inference and validation

mod calls;
mod collections;
mod control_flow;
mod identifiers;
pub mod lambdas;
mod literals;
mod operators;
mod result_option;
mod structs;

use super::check_pattern::check_pattern_expr;
use super::context::TypeContext;
use crate::ast::*;

/// Check an expression with an optional expected type hint for bidirectional type inference
pub fn check_expr_with_hint(
    expr: &Expr,
    ctx: &TypeContext,
    expected: Option<&TypeExpr>,
) -> Result<TypeExpr, String> {
    match expr {
        Expr::Lambda { params, body } => lambdas::check_lambda(params, body, ctx, expected),
        // Empty list can be inferred from expected type
        Expr::List(exprs) if exprs.is_empty() => {
            if let Some(TypeExpr::List(elem_type)) = expected {
                return Ok(TypeExpr::List(elem_type.clone()));
            }
            // Fall through to regular check which will use current_return_type
            check_expr_inner(expr, ctx)
        }
        _ => check_expr_inner(expr, ctx),
    }
}

pub fn check_expr(expr: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    check_expr_with_hint(expr, ctx, None)
}

/// Check an expression within a block context (where assignments can modify scope)
pub fn check_block_expr(expr: &Expr, ctx: &mut TypeContext) -> Result<TypeExpr, String> {
    match expr {
        // New let binding: let x = value or let mut x = value
        Expr::Let { name, mutable, value } => {
            let value_type = check_expr_with_hint(value, ctx, None)?;
            ctx.define_local(name.clone(), value_type, *mutable);
            Ok(TypeExpr::Named("void".to_string()))
        }
        // Reassignment: x = value (only for mutable bindings)
        Expr::Reassign { target, value } => {
            let binding = ctx.lookup_local_binding(target).ok_or_else(|| {
                format!("Cannot assign to undeclared variable '{}'", target)
            })?;

            if !binding.is_mutable() {
                return Err(format!(
                    "Cannot assign twice to immutable variable '{}'\n\
                     help: consider making this binding mutable: `let mut {}`",
                    target, target
                ));
            }

            let expected_ty = binding.get().clone();
            let value_type = check_expr_with_hint(value, ctx, Some(&expected_ty))?;

            // Check type compatibility
            if !crate::types::compat::types_compatible(&value_type, &expected_ty, ctx) {
                return Err(format!(
                    "Type mismatch: expected {:?}, found {:?}",
                    expected_ty, value_type
                ));
            }

            Ok(TypeExpr::Named("void".to_string()))
        }
        Expr::For {
            binding,
            iterator,
            body,
        } => {
            let iter_type = check_expr(iterator, ctx)?;
            // Get element type from iterator
            let elem_type = match &iter_type {
                TypeExpr::List(inner) => inner.as_ref().clone(),
                TypeExpr::Named(n) if n == "Range" => TypeExpr::Named("int".to_string()),
                _ => return Err(format!("Cannot iterate over {:?}", iter_type)),
            };
            // Add loop binding to context (immutable, like Rust)
            ctx.define_local(binding.clone(), elem_type, false);
            check_block_expr(body, ctx)?;
            Ok(TypeExpr::Named("void".to_string()))
        }
        // Capability injection: with Capability = impl in body
        Expr::With { capability, implementation, body } => {
            // Check the implementation expression
            check_expr(implementation, ctx)?;

            // Add the capability to context for checking body
            ctx.add_capability(capability.clone());

            // Check the body with the capability available
            let body_type = check_block_expr(body, ctx)?;

            // Remove the capability after the with block
            ctx.remove_capability(capability);

            Ok(body_type)
        }
        // For other expressions, delegate to immutable check
        _ => check_expr_with_hint(expr, ctx, None),
    }
}

pub fn check_expr_inner(expr: &Expr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    match expr {
        // Literals
        Expr::Int(_) => literals::check_int(),
        Expr::Float(_) => literals::check_float(),
        Expr::String(_) => literals::check_string(),
        Expr::Bool(_) => literals::check_bool(),
        Expr::Nil => literals::check_nil(),

        // Identifiers and configs
        Expr::Ident(name) => identifiers::check_ident(name, ctx),
        Expr::Config(name) => identifiers::check_config(name, ctx),

        // Collections
        Expr::List(exprs) => collections::check_list(exprs, ctx),
        Expr::Tuple(exprs) => collections::check_tuple(exprs, ctx),
        Expr::MapLiteral(entries) => collections::check_map_literal(entries, ctx),
        Expr::Range { start, end } => collections::check_range(start, end, ctx),

        // Operators
        Expr::Binary { op, left, right } => operators::check_binary(op, left, right, ctx),
        Expr::Unary { op, operand } => operators::check_unary(op, operand, ctx),

        // Function calls
        Expr::Call { func, args } => calls::check_call(func, args, ctx),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => calls::check_method_call(receiver, method, args, ctx),

        // Result/Option types
        Expr::Ok(inner) => result_option::check_ok(inner, ctx),
        Expr::Err(inner) => result_option::check_err(inner, ctx),
        Expr::Some(inner) => result_option::check_some(inner, ctx),
        Expr::None_ => result_option::check_none(ctx),
        Expr::Coalesce { value, default } => result_option::check_coalesce(value, default, ctx),
        Expr::Unwrap(inner) => result_option::check_unwrap(inner, ctx),

        // Control flow
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => control_flow::check_if(condition, then_branch, else_branch.as_deref(), ctx),
        Expr::Match(m) => control_flow::check_match(m, ctx),
        Expr::Block(exprs) => control_flow::check_block(exprs, ctx),
        Expr::For {
            binding,
            iterator,
            body,
        } => control_flow::check_for(binding, iterator, body, ctx),

        // Lambdas (without type hint)
        Expr::Lambda { params, body } => lambdas::check_lambda(params, body, ctx, None),

        // Structs and field access
        Expr::Struct { name, fields } => structs::check_struct(name, fields, ctx),
        Expr::Field(expr, field) => structs::check_field(expr, field, ctx),
        Expr::Index(arr, index) => structs::check_index(arr, index, ctx),

        // Patterns
        Expr::Pattern(p) => check_pattern_expr(p, ctx),

        // Special
        Expr::LengthPlaceholder => Ok(TypeExpr::Named("int".to_string())),
        Expr::Let { value, .. } => {
            check_expr(value, ctx)?;
            Ok(TypeExpr::Named("void".to_string()))
        }
        Expr::Reassign { value, .. } => {
            check_expr(value, ctx)?;
            Ok(TypeExpr::Named("void".to_string()))
        }
        Expr::With { capability, implementation, body } => {
            // Check the implementation expression
            check_expr(implementation, ctx)?;

            // Create a child context with the capability added
            let mut child_ctx = ctx.child();
            child_ctx.add_capability(capability.clone());

            // Check the body with the capability available
            check_expr(body, &child_ctx)
        }

        Expr::Await(inner) => {
            let inner_type = check_expr(inner, ctx)?;
            // Unwrap async type
            match inner_type {
                TypeExpr::Async(t) => Ok(*t),
                _ => Err(format!(
                    "await requires async type, got {:?}",
                    inner_type
                )),
            }
        }
    }
}
