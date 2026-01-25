//! Type inference for expressions.
//!
//! This module contains the expression type inference logic, split into:
//! - `expr.rs`: Literals, binary/unary operations, identifiers
//! - `call.rs`: Function calls, method calls
//! - `control.rs`: Control flow (if/else, match, loops)
//! - `pattern.rs`: Pattern expressions (run, try, match, map, etc.)
//! - `match_binding.rs`: Match pattern binding extraction

mod call;
mod control;
mod expr;
mod match_binding;
mod pattern;

use crate::ir::{ExprId, ExprKind, Name};
use crate::types::Type;
use crate::stack::ensure_sufficient_stack;
use super::checker::TypeChecker;
use std::collections::HashSet;

pub use call::*;
pub use control::*;
pub use expr::*;
pub use match_binding::*;
pub use pattern::*;

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
    let expr = checker.arena.get_expr(expr_id);
    let span = expr.span;

    let ty = match &expr.kind {
        // Literals - handled in expr.rs
        ExprKind::Int(_) => Type::Int,
        ExprKind::Float(_) => Type::Float,
        ExprKind::Bool(_) => Type::Bool,
        ExprKind::String(_) => Type::Str,
        ExprKind::Char(_) => Type::Char,
        ExprKind::Duration { .. } => Type::Duration,
        ExprKind::Size { .. } => Type::Size,
        ExprKind::Unit => Type::Unit,
        ExprKind::HashLength => Type::Int,

        // Variable reference
        ExprKind::Ident(name) => infer_ident(checker, *name, span),

        // Function reference
        ExprKind::FunctionRef(name) => infer_function_ref(checker, *name, span),

        // Binary operations
        ExprKind::Binary { op, left, right } => {
            infer_binary(checker, *op, *left, *right, span)
        }

        // Unary operations
        ExprKind::Unary { op, operand } => {
            infer_unary(checker, *op, *operand, span)
        }

        // Function call
        ExprKind::Call { func, args } => {
            infer_call(checker, *func, *args, span)
        }

        // Named call
        ExprKind::CallNamed { func, args } => {
            infer_call_named(checker, *func, *args, span)
        }

        // Method call
        ExprKind::MethodCall { receiver, method, args } => {
            infer_method_call(checker, *receiver, *method, *args, span)
        }

        // If expression
        ExprKind::If { cond, then_branch, else_branch } => {
            infer_if(checker, *cond, *then_branch, *else_branch, span)
        }

        // Match expression
        ExprKind::Match { scrutinee, arms } => {
            infer_match(checker, *scrutinee, *arms, span)
        }

        // For loop
        ExprKind::For { binding, iter, guard, body, is_yield } => {
            infer_for(checker, *binding, *iter, *guard, *body, *is_yield, span)
        }

        // Loop
        ExprKind::Loop { body } => {
            infer_loop(checker, *body)
        }

        // Block
        ExprKind::Block { stmts, result } => {
            infer_block(checker, *stmts, *result, span)
        }

        // Let binding (as expression)
        ExprKind::Let { pattern, ty, init, .. } => {
            infer_let(checker, pattern, ty.clone(), *init, span)
        }

        // Lambda
        ExprKind::Lambda { params, ret_ty, body } => {
            infer_lambda(checker, *params, ret_ty.clone(), *body, span)
        }

        // List
        ExprKind::List(elements) => {
            infer_list(checker, *elements)
        }

        // Tuple
        ExprKind::Tuple(elements) => {
            infer_tuple(checker, *elements)
        }

        // Map
        ExprKind::Map(entries) => {
            infer_map(checker, *entries, span)
        }

        // Struct literal
        ExprKind::Struct { name, fields } => {
            infer_struct(checker, *name, *fields)
        }

        // Range
        ExprKind::Range { start, end, inclusive } => {
            infer_range(checker, *start, *end, *inclusive, span)
        }

        // Field access
        ExprKind::Field { receiver, field } => {
            infer_field(checker, *receiver, *field)
        }

        // Index access
        ExprKind::Index { receiver, index } => {
            infer_index(checker, *receiver, *index, span)
        }

        // FunctionSeq: run, try, match
        ExprKind::FunctionSeq(func_seq) => {
            infer_function_seq(checker, func_seq, span)
        }

        // FunctionExp: map, filter, fold, etc.
        ExprKind::FunctionExp(func_exp) => {
            infer_function_exp(checker, func_exp)
        }

        // Variant constructors
        ExprKind::Ok(inner) => infer_ok(checker, *inner),
        ExprKind::Err(inner) => infer_err(checker, *inner),
        ExprKind::Some(inner) => infer_some(checker, *inner),
        ExprKind::None => infer_none(checker),

        // Control flow
        ExprKind::Return(value) => infer_return(checker, *value),
        ExprKind::Break(value) => infer_break(checker, *value),
        ExprKind::Continue => Type::Never,

        ExprKind::Await(inner) => infer_await(checker, *inner),
        ExprKind::Try(inner) => infer_try(checker, *inner, span),

        ExprKind::Assign { target, value } => {
            infer_assign(checker, *target, *value, span)
        }

        // Config reference
        ExprKind::Config(_name) => {
            // TODO: implement config type lookup
            checker.ctx.fresh_var()
        }

        // Self reference
        ExprKind::SelfRef => {
            // TODO: implement self type in impl blocks
            checker.ctx.fresh_var()
        }

        // Error placeholder
        ExprKind::Error => Type::Error,
    };

    // Store the type
    checker.store_type(expr_id, ty.clone());
    ty
}

/// Collect free variables from an expression (inner recursive helper).
///
/// This is used for closure self-capture detection.
pub fn collect_free_vars_inner(
    checker: &TypeChecker<'_>,
    expr_id: ExprId,
    bound: &HashSet<Name>,
    free: &mut HashSet<Name>,
) {
    let expr = checker.arena.get_expr(expr_id);

    match &expr.kind {
        // Variable reference - free if not bound
        ExprKind::Ident(name) => {
            if !bound.contains(name) {
                free.insert(*name);
            }
        }

        // Function reference - check if it refers to a local binding
        ExprKind::FunctionRef(name) => {
            if !bound.contains(name) {
                free.insert(*name);
            }
        }

        // Literals - no free variables
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Char(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Unit
        | ExprKind::Config(_)
        | ExprKind::SelfRef
        | ExprKind::HashLength
        | ExprKind::None
        | ExprKind::Continue
        | ExprKind::Error => {}

        // Binary - check both sides
        ExprKind::Binary { left, right, .. } => {
            collect_free_vars_inner(checker, *left, bound, free);
            collect_free_vars_inner(checker, *right, bound, free);
        }

        // Unary - check operand
        ExprKind::Unary { operand, .. } => {
            collect_free_vars_inner(checker, *operand, bound, free);
        }

        // Call - check function and args
        ExprKind::Call { func, args } => {
            collect_free_vars_inner(checker, *func, bound, free);
            for arg_id in checker.arena.get_expr_list(*args) {
                collect_free_vars_inner(checker, *arg_id, bound, free);
            }
        }

        // Named call
        ExprKind::CallNamed { func, args } => {
            collect_free_vars_inner(checker, *func, bound, free);
            for arg in checker.arena.get_call_args(*args) {
                collect_free_vars_inner(checker, arg.value, bound, free);
            }
        }

        // Method call
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_free_vars_inner(checker, *receiver, bound, free);
            for arg_id in checker.arena.get_expr_list(*args) {
                collect_free_vars_inner(checker, *arg_id, bound, free);
            }
        }

        // Field access
        ExprKind::Field { receiver, .. } => {
            collect_free_vars_inner(checker, *receiver, bound, free);
        }

        // Index access
        ExprKind::Index { receiver, index } => {
            collect_free_vars_inner(checker, *receiver, bound, free);
            collect_free_vars_inner(checker, *index, bound, free);
        }

        // If expression
        ExprKind::If { cond, then_branch, else_branch } => {
            collect_free_vars_inner(checker, *cond, bound, free);
            collect_free_vars_inner(checker, *then_branch, bound, free);
            if let Some(else_id) = else_branch {
                collect_free_vars_inner(checker, *else_id, bound, free);
            }
        }

        // Match expression
        ExprKind::Match { scrutinee, arms } => {
            collect_free_vars_inner(checker, *scrutinee, bound, free);
            for arm in checker.arena.get_arms(*arms) {
                // Collect pattern bindings
                let pattern_names = collect_match_pattern_names(&arm.pattern);
                let mut arm_bound = bound.clone();
                arm_bound.extend(pattern_names);

                // Check guard with pattern bindings in scope
                if let Some(guard_id) = arm.guard {
                    collect_free_vars_inner(checker, guard_id, &arm_bound, free);
                }
                collect_free_vars_inner(checker, arm.body, &arm_bound, free);
            }
        }

        // For loop - binding is bound in body
        ExprKind::For { binding, iter, guard, body, .. } => {
            collect_free_vars_inner(checker, *iter, bound, free);
            let mut body_bound = bound.clone();
            body_bound.insert(*binding);
            if let Some(guard_id) = guard {
                collect_free_vars_inner(checker, *guard_id, &body_bound, free);
            }
            collect_free_vars_inner(checker, *body, &body_bound, free);
        }

        // Loop
        ExprKind::Loop { body } => {
            collect_free_vars_inner(checker, *body, bound, free);
        }

        // Block - statements can introduce bindings
        ExprKind::Block { stmts, result } => {
            let mut block_bound = bound.clone();
            for stmt in checker.arena.get_stmt_range(*stmts) {
                match &stmt.kind {
                    crate::ir::StmtKind::Expr(e) => {
                        collect_free_vars_inner(checker, *e, &block_bound, free);
                    }
                    crate::ir::StmtKind::Let { pattern, init, .. } => {
                        // Init is evaluated before the binding is in scope
                        collect_free_vars_inner(checker, *init, &block_bound, free);
                        // Add pattern bindings for subsequent statements
                        checker.add_pattern_bindings(pattern, &mut block_bound);
                    }
                }
            }
            if let Some(result_id) = result {
                collect_free_vars_inner(checker, *result_id, &block_bound, free);
            }
        }

        // Let binding (as expression)
        ExprKind::Let { init, .. } => {
            // Init is evaluated before the binding
            collect_free_vars_inner(checker, *init, bound, free);
        }

        // Lambda - params are bound in body
        ExprKind::Lambda { params, body, .. } => {
            let mut lambda_bound = bound.clone();
            for param in checker.arena.get_params(*params) {
                lambda_bound.insert(param.name);
            }
            collect_free_vars_inner(checker, *body, &lambda_bound, free);
        }

        // List
        ExprKind::List(elements) => {
            for elem_id in checker.arena.get_expr_list(*elements) {
                collect_free_vars_inner(checker, *elem_id, bound, free);
            }
        }

        // Map
        ExprKind::Map(entries) => {
            for entry in checker.arena.get_map_entries(*entries) {
                collect_free_vars_inner(checker, entry.key, bound, free);
                collect_free_vars_inner(checker, entry.value, bound, free);
            }
        }

        // Struct literal
        ExprKind::Struct { fields, .. } => {
            for init in checker.arena.get_field_inits(*fields) {
                if let Some(value_id) = init.value {
                    collect_free_vars_inner(checker, value_id, bound, free);
                } else {
                    // Shorthand field: { x } is equivalent to { x: x }
                    if !bound.contains(&init.name) {
                        free.insert(init.name);
                    }
                }
            }
        }

        // Tuple
        ExprKind::Tuple(elements) => {
            for elem_id in checker.arena.get_expr_list(*elements) {
                collect_free_vars_inner(checker, *elem_id, bound, free);
            }
        }

        // Range
        ExprKind::Range { start, end, .. } => {
            if let Some(start_id) = start {
                collect_free_vars_inner(checker, *start_id, bound, free);
            }
            if let Some(end_id) = end {
                collect_free_vars_inner(checker, *end_id, bound, free);
            }
        }

        // Variant constructors
        ExprKind::Ok(inner) | ExprKind::Err(inner) => {
            if let Some(id) = inner {
                collect_free_vars_inner(checker, *id, bound, free);
            }
        }
        ExprKind::Some(inner) => {
            collect_free_vars_inner(checker, *inner, bound, free);
        }

        // Control flow
        ExprKind::Return(value) | ExprKind::Break(value) => {
            if let Some(id) = value {
                collect_free_vars_inner(checker, *id, bound, free);
            }
        }

        ExprKind::Await(inner) | ExprKind::Try(inner) => {
            collect_free_vars_inner(checker, *inner, bound, free);
        }

        ExprKind::Assign { target, value } => {
            collect_free_vars_inner(checker, *target, bound, free);
            collect_free_vars_inner(checker, *value, bound, free);
        }

        // FunctionSeq
        ExprKind::FunctionSeq(func_seq) => {
            collect_free_vars_function_seq(checker, func_seq, bound, free);
        }

        // FunctionExp
        ExprKind::FunctionExp(func_exp) => {
            for prop in checker.arena.get_named_exprs(func_exp.props) {
                collect_free_vars_inner(checker, prop.value, bound, free);
            }
        }
    }
}

/// Collect free variables from a FunctionSeq (run, try, match).
fn collect_free_vars_function_seq(
    checker: &TypeChecker<'_>,
    func_seq: &crate::ir::FunctionSeq,
    bound: &HashSet<Name>,
    free: &mut HashSet<Name>,
) {
    use crate::ir::{FunctionSeq, SeqBinding};

    match func_seq {
        FunctionSeq::Run { bindings, result, .. }
        | FunctionSeq::Try { bindings, result, .. } => {
            let mut seq_bound = bound.clone();
            for binding in checker.arena.get_seq_bindings(*bindings) {
                match binding {
                    SeqBinding::Let { pattern, value, .. } => {
                        collect_free_vars_inner(checker, *value, &seq_bound, free);
                        checker.add_pattern_bindings(pattern, &mut seq_bound);
                    }
                    SeqBinding::Stmt { expr, .. } => {
                        collect_free_vars_inner(checker, *expr, &seq_bound, free);
                    }
                }
            }
            collect_free_vars_inner(checker, *result, &seq_bound, free);
        }
        FunctionSeq::Match { scrutinee, arms, .. } => {
            collect_free_vars_inner(checker, *scrutinee, bound, free);
            for arm in checker.arena.get_arms(*arms) {
                // Collect pattern bindings
                let pattern_names = collect_match_pattern_names(&arm.pattern);
                let mut arm_bound = bound.clone();
                arm_bound.extend(pattern_names);

                // Check guard with pattern bindings in scope
                if let Some(guard_id) = arm.guard {
                    collect_free_vars_inner(checker, guard_id, &arm_bound, free);
                }
                collect_free_vars_inner(checker, arm.body, &arm_bound, free);
            }
        }
        FunctionSeq::ForPattern { over, map, arm, default, .. } => {
            collect_free_vars_inner(checker, *over, bound, free);
            if let Some(map_fn) = map {
                collect_free_vars_inner(checker, *map_fn, bound, free);
            }
            // Collect pattern bindings from the arm
            let pattern_names = collect_match_pattern_names(&arm.pattern);
            let mut arm_bound = bound.clone();
            arm_bound.extend(pattern_names);

            // Check guard with pattern bindings in scope
            if let Some(guard_id) = arm.guard {
                collect_free_vars_inner(checker, guard_id, &arm_bound, free);
            }
            collect_free_vars_inner(checker, arm.body, &arm_bound, free);
            collect_free_vars_inner(checker, *default, bound, free);
        }
    }
}
