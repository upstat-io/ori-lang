//! Type inference for expressions.
//!
//! This module contains the expression type inference logic, split into:
//! - `expressions/`: Literals, binary/unary operations, identifiers, collections, structs
//! - `call.rs`: Function calls, method calls
//! - `control.rs`: Control flow (if/else, match, loops)
//! - `pattern.rs`: Pattern expressions (run, try, match, map, etc.)
//! - `match_binding.rs`: Match pattern binding extraction
//! - `free_vars.rs`: Free variable collection for closure self-capture detection
//! - `type_annotations.rs`: Type annotation checking for let bindings
//! - `bound_context.rs`: Stack-based scope tracking for free variable collection

mod bound_context;
pub mod builtin_methods;
mod call;
mod control;
pub mod expressions;
mod free_vars;
mod match_binding;
mod pattern;
mod pattern_types;
mod pattern_unification;
#[cfg(test)]
mod tests;
mod type_annotations;

use super::checker::bound_checking;
use crate::checker::TypeChecker;
use ori_ir::{ExprId, ExprKind};
use ori_stack::ensure_sufficient_stack;
use ori_types::Type;

pub use call::*;
pub use control::*;
pub use expressions::*;
pub use free_vars::{add_pattern_bindings, collect_free_vars_inner};
pub use match_binding::*;
pub use pattern::*;
pub use type_annotations::{check_type_annotation, check_type_annotation_id, infer_let_init};

/// Infer the type of an expression.
///
/// This is the main entry point for expression type inference.
/// It dispatches to specialized handlers based on expression kind.
///
/// Uses `ensure_sufficient_stack` to prevent stack overflow
/// on deeply nested expressions.
pub fn infer_expr(checker: &mut TypeChecker<'_>, expr_id: ExprId) -> Type {
    ensure_sufficient_stack(|| infer_expr_inner(checker, expr_id))
}

/// Inner type inference logic (wrapped by `infer_expr` for stack safety).
fn infer_expr_inner(checker: &mut TypeChecker<'_>, expr_id: ExprId) -> Type {
    let expr = checker.context.arena.get_expr(expr_id);
    let span = expr.span;

    let ty = match &expr.kind {
        // Literals and special tokens
        ExprKind::Int(_) | ExprKind::HashLength => Type::Int,
        ExprKind::Float(_) => Type::Float,
        ExprKind::Bool(_) => Type::Bool,
        ExprKind::String(_) => Type::Str,
        ExprKind::Char(_) => Type::Char,
        ExprKind::Duration { .. } => Type::Duration,
        ExprKind::Size { .. } => Type::Size,
        ExprKind::Unit => Type::Unit,

        // Variable reference
        ExprKind::Ident(name) => infer_ident(checker, *name, span),

        // Function reference
        ExprKind::FunctionRef(name) => infer_function_ref(checker, *name, span),

        // Binary operations
        ExprKind::Binary { op, left, right } => infer_binary(checker, *op, *left, *right, span),

        // Unary operations
        ExprKind::Unary { op, operand } => infer_unary(checker, *op, *operand, span),

        // Function call
        ExprKind::Call { func, args } => infer_call(checker, *func, *args, span),

        // Named call
        ExprKind::CallNamed { func, args } => infer_call_named(checker, *func, *args, span),

        // Method call
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => infer_method_call(checker, *receiver, *method, *args, span),

        // Method call with named arguments
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => infer_method_call_named(checker, *receiver, *method, *args, span),

        // If expression
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => infer_if(checker, *cond, *then_branch, *else_branch, span),

        // Match expression
        ExprKind::Match { scrutinee, arms } => infer_match(checker, *scrutinee, *arms, span),

        // For loop
        ExprKind::For {
            binding,
            iter,
            guard,
            body,
            is_yield,
        } => infer_for(checker, *binding, *iter, *guard, *body, *is_yield, span),

        // Loop
        ExprKind::Loop { body } => infer_loop(checker, *body),

        // Block
        ExprKind::Block { stmts, result } => infer_block(checker, *stmts, *result, span),

        // Let binding (as expression)
        ExprKind::Let {
            pattern, ty, init, ..
        } => infer_let(checker, pattern, ty.as_ref(), *init, span),

        // Lambda
        ExprKind::Lambda {
            params,
            ret_ty,
            body,
        } => infer_lambda(checker, *params, ret_ty.as_ref(), *body, span),

        // List
        ExprKind::List(elements) => infer_list(checker, *elements),

        // Tuple
        ExprKind::Tuple(elements) => infer_tuple(checker, *elements),

        // Map
        ExprKind::Map(entries) => infer_map(checker, *entries, span),

        // Struct literal
        ExprKind::Struct { name, fields } => infer_struct(checker, *name, *fields),

        // Range
        ExprKind::Range { start, end, .. } => infer_range(checker, *start, *end),

        // Field access
        ExprKind::Field { receiver, field } => infer_field(checker, *receiver, *field),

        // Index access
        ExprKind::Index { receiver, index } => infer_index(checker, *receiver, *index, span),

        // FunctionSeq: run, try, match
        ExprKind::FunctionSeq(func_seq) => infer_function_seq(checker, func_seq, span),

        // FunctionExp: map, filter, fold, etc.
        ExprKind::FunctionExp(func_exp) => infer_function_exp(checker, func_exp),

        // Variant constructors
        ExprKind::Ok(inner) => infer_ok(checker, *inner),
        ExprKind::Err(inner) => infer_err(checker, *inner),
        ExprKind::Some(inner) => infer_some(checker, *inner),
        ExprKind::None => infer_none(checker),

        // Control flow
        ExprKind::Return(value) => infer_return(checker, *value),
        ExprKind::Break(value) => infer_break(checker, *value),
        ExprKind::Continue(value) => infer_continue(checker, *value),

        ExprKind::Await(inner) => infer_await(checker, *inner, span),
        ExprKind::Try(inner) => infer_try(checker, *inner, span),

        ExprKind::Assign { target, value } => infer_assign(checker, *target, *value, span),

        // Config reference
        ExprKind::Config(name) => {
            if let Some(ty) = checker.scope.config_types.get(name) {
                ty.clone()
            } else {
                checker.push_error(
                    format!(
                        "undefined config variable `${}`",
                        checker.context.interner.lookup(*name)
                    ),
                    span,
                    ori_diagnostic::ErrorCode::E2004,
                );
                Type::Error
            }
        }

        // Self reference
        ExprKind::SelfRef => {
            if let Some(ref self_ty) = checker.scope.current_impl_self {
                self_ty.clone()
            } else {
                checker.push_error(
                    "`self` can only be used inside impl blocks",
                    span,
                    ori_diagnostic::ErrorCode::E2003,
                );
                Type::Error
            }
        }

        // Capability provision: type is the body's type
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => {
            // Infer the provider type (capability implementation)
            let provider_ty = infer_expr(checker, *provider);
            let resolved_provider_ty = checker.inference.ctx.resolve(&provider_ty);

            // Validate: if capability is a known trait, provider must implement it
            if checker.registries.traits.has_trait(*capability) {
                // Check if provider type implements the capability trait
                if !checker
                    .registries
                    .traits
                    .implements(&resolved_provider_ty, *capability)
                {
                    // Also check built-in trait implementations
                    let cap_name = checker.context.interner.lookup(*capability);
                    let implements_builtin =
                        bound_checking::primitive_implements_trait(&resolved_provider_ty, cap_name);

                    if !implements_builtin {
                        let provider_ty_str = format!("{resolved_provider_ty:?}");
                        checker.push_error(
                            format!(
                                "provider type `{provider_ty_str}` does not implement capability `{cap_name}`"
                            ),
                            span,
                            ori_diagnostic::ErrorCode::E2013,
                        );
                    }
                }
            }

            // Track this capability as provided for propagation checking
            checker.scope.provided_caps.insert(*capability);

            // The expression type is the body type
            let body_ty = infer_expr(checker, *body);

            // Remove the provided capability after leaving scope
            checker.scope.provided_caps.remove(capability);

            body_ty
        }

        // Error placeholder
        ExprKind::Error => Type::Error,
    };

    // Store the type
    checker.store_type(expr_id, &ty);
    ty
}
