//! Free variable collection for closure self-capture detection.
//!
//! This module collects free variables from expressions to detect closure
//! self-capture. A variable is "free" if it's referenced but not bound
//! within the expression.
//!
//! Uses `BoundContext` for efficient scope tracking without `HashSet` cloning.

use super::bound_context::BoundContext;
use super::collect_match_pattern_names;
use crate::checker::TypeChecker;
use ori_ir::{ExprId, ExprKind, FunctionSeq, Name, SeqBinding, StmtKind};
use std::collections::HashSet;

/// Collect free variables from an expression (public API).
///
/// This is used for closure self-capture detection.
/// Wraps the internal implementation that uses `BoundContext`.
#[expect(
    clippy::implicit_hasher,
    reason = "Standard HashMap/HashSet sufficient here"
)]
pub fn collect_free_vars_inner(
    checker: &TypeChecker<'_>,
    expr_id: ExprId,
    bound: &HashSet<Name>,
    free: &mut HashSet<Name>,
) {
    let mut ctx = BoundContext::new(bound);
    collect_free_vars_impl(checker, expr_id, &mut ctx, free);
}

/// Internal implementation using `BoundContext` for efficient scope tracking.
fn collect_free_vars_impl(
    checker: &TypeChecker<'_>,
    expr_id: ExprId,
    bound: &mut BoundContext<'_>,
    free: &mut HashSet<Name>,
) {
    let expr = checker.context.arena.get_expr(expr_id);

    match &expr.kind {
        // Variable or function reference - free if not bound
        ExprKind::Ident(name) | ExprKind::FunctionRef(name) => {
            if !bound.contains(*name) {
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
        | ExprKind::Error => {}

        // Binary - check both sides
        ExprKind::Binary { left, right, .. } => {
            collect_free_vars_impl(checker, *left, bound, free);
            collect_free_vars_impl(checker, *right, bound, free);
        }

        // Unary - check operand
        ExprKind::Unary { operand, .. } => {
            collect_free_vars_impl(checker, *operand, bound, free);
        }

        // Call - check function and args
        ExprKind::Call { func, args } => {
            collect_free_vars_impl(checker, *func, bound, free);
            for arg_id in checker.context.arena.get_expr_list(*args) {
                collect_free_vars_impl(checker, *arg_id, bound, free);
            }
        }

        // Named call
        ExprKind::CallNamed { func, args } => {
            collect_free_vars_impl(checker, *func, bound, free);
            for arg in checker.context.arena.get_call_args(*args) {
                collect_free_vars_impl(checker, arg.value, bound, free);
            }
        }

        // Method call
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_free_vars_impl(checker, *receiver, bound, free);
            for arg_id in checker.context.arena.get_expr_list(*args) {
                collect_free_vars_impl(checker, *arg_id, bound, free);
            }
        }

        // Method call with named arguments
        ExprKind::MethodCallNamed { receiver, args, .. } => {
            collect_free_vars_impl(checker, *receiver, bound, free);
            for arg in checker.context.arena.get_call_args(*args) {
                collect_free_vars_impl(checker, arg.value, bound, free);
            }
        }

        // Field access
        ExprKind::Field { receiver, .. } => {
            collect_free_vars_impl(checker, *receiver, bound, free);
        }

        // Index access
        ExprKind::Index { receiver, index } => {
            collect_free_vars_impl(checker, *receiver, bound, free);
            collect_free_vars_impl(checker, *index, bound, free);
        }

        // If expression
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_free_vars_impl(checker, *cond, bound, free);
            collect_free_vars_impl(checker, *then_branch, bound, free);
            if let Some(else_id) = else_branch {
                collect_free_vars_impl(checker, *else_id, bound, free);
            }
        }

        // Match expression - each arm introduces pattern bindings
        ExprKind::Match { scrutinee, arms } => {
            collect_free_vars_match(checker, *scrutinee, *arms, bound, free);
        }

        // For loop - binding is bound in body
        ExprKind::For {
            binding,
            iter,
            guard,
            body,
            ..
        } => {
            collect_free_vars_for(checker, *binding, *iter, *guard, *body, bound, free);
        }

        // Loop
        ExprKind::Loop { body } => {
            collect_free_vars_impl(checker, *body, bound, free);
        }

        // Block - statements can introduce bindings
        ExprKind::Block { stmts, result } => {
            collect_free_vars_block(checker, *stmts, *result, bound, free);
        }

        // Let binding (as expression)
        ExprKind::Let { init, .. } => {
            // Init is evaluated before the binding
            collect_free_vars_impl(checker, *init, bound, free);
        }

        // Lambda - params are bound in body
        ExprKind::Lambda { params, body, .. } => {
            collect_free_vars_lambda(checker, *params, *body, bound, free);
        }

        // List and Tuple
        ExprKind::List(elements) | ExprKind::Tuple(elements) => {
            for elem_id in checker.context.arena.get_expr_list(*elements) {
                collect_free_vars_impl(checker, *elem_id, bound, free);
            }
        }

        // Map
        ExprKind::Map(entries) => {
            for entry in checker.context.arena.get_map_entries(*entries) {
                collect_free_vars_impl(checker, entry.key, bound, free);
                collect_free_vars_impl(checker, entry.value, bound, free);
            }
        }

        // Struct literal
        ExprKind::Struct { fields, .. } => {
            for init in checker.context.arena.get_field_inits(*fields) {
                if let Some(value_id) = init.value {
                    collect_free_vars_impl(checker, value_id, bound, free);
                } else {
                    // Shorthand field: { x } is equivalent to { x: x }
                    if !bound.contains(init.name) {
                        free.insert(init.name);
                    }
                }
            }
        }

        // Range
        ExprKind::Range { start, end, .. } => {
            if let Some(start_id) = start {
                collect_free_vars_impl(checker, *start_id, bound, free);
            }
            if let Some(end_id) = end {
                collect_free_vars_impl(checker, *end_id, bound, free);
            }
        }

        // Variant constructors
        ExprKind::Ok(inner) | ExprKind::Err(inner) => {
            if let Some(id) = inner {
                collect_free_vars_impl(checker, *id, bound, free);
            }
        }

        // Expressions with single inner expression
        ExprKind::Some(inner) | ExprKind::Await(inner) | ExprKind::Try(inner) => {
            collect_free_vars_impl(checker, *inner, bound, free);
        }

        // Control flow with optional value
        ExprKind::Return(value) | ExprKind::Break(value) | ExprKind::Continue(value) => {
            if let Some(id) = value {
                collect_free_vars_impl(checker, *id, bound, free);
            }
        }

        ExprKind::Assign { target, value } => {
            collect_free_vars_impl(checker, *target, bound, free);
            collect_free_vars_impl(checker, *value, bound, free);
        }

        // WithCapability
        ExprKind::WithCapability { provider, body, .. } => {
            collect_free_vars_impl(checker, *provider, bound, free);
            collect_free_vars_impl(checker, *body, bound, free);
        }

        // FunctionSeq
        ExprKind::FunctionSeq(func_seq) => {
            collect_free_vars_function_seq_impl(checker, func_seq, bound, free);
        }

        // FunctionExp
        ExprKind::FunctionExp(func_exp) => {
            for prop in checker.context.arena.get_named_exprs(func_exp.props) {
                collect_free_vars_impl(checker, prop.value, bound, free);
            }
        }
    }
}

/// Collect free variables from a match expression.
fn collect_free_vars_match(
    checker: &TypeChecker<'_>,
    scrutinee: ExprId,
    arms: ori_ir::ArmRange,
    bound: &mut BoundContext<'_>,
    free: &mut HashSet<Name>,
) {
    collect_free_vars_impl(checker, scrutinee, bound, free);
    for arm in checker.context.arena.get_arms(arms) {
        bound.with_scope(|inner| {
            // Collect pattern bindings
            let pattern_names = collect_match_pattern_names(&arm.pattern, checker.context.arena);
            inner.add_bindings(pattern_names);

            // Check guard with pattern bindings in scope
            if let Some(guard_id) = arm.guard {
                collect_free_vars_impl(checker, guard_id, inner, free);
            }
            collect_free_vars_impl(checker, arm.body, inner, free);
        });
    }
}

/// Collect free variables from a for loop.
fn collect_free_vars_for(
    checker: &TypeChecker<'_>,
    binding: Name,
    iter: ExprId,
    guard: Option<ExprId>,
    body: ExprId,
    bound: &mut BoundContext<'_>,
    free: &mut HashSet<Name>,
) {
    collect_free_vars_impl(checker, iter, bound, free);
    bound.with_scope(|inner| {
        inner.add_binding(binding);
        if let Some(guard_id) = guard {
            collect_free_vars_impl(checker, guard_id, inner, free);
        }
        collect_free_vars_impl(checker, body, inner, free);
    });
}

/// Collect free variables from a block expression.
fn collect_free_vars_block(
    checker: &TypeChecker<'_>,
    stmts: ori_ir::StmtRange,
    result: Option<ExprId>,
    bound: &mut BoundContext<'_>,
    free: &mut HashSet<Name>,
) {
    bound.with_scope(|inner| {
        for stmt in checker.context.arena.get_stmt_range(stmts) {
            match &stmt.kind {
                StmtKind::Expr(e) => {
                    collect_free_vars_impl(checker, *e, inner, free);
                }
                StmtKind::Let { pattern, init, .. } => {
                    // Init is evaluated before the binding is in scope
                    collect_free_vars_impl(checker, *init, inner, free);
                    // Add pattern bindings for subsequent statements
                    add_pattern_bindings_to_context(pattern, inner);
                }
            }
        }
        if let Some(result_id) = result {
            collect_free_vars_impl(checker, result_id, inner, free);
        }
    });
}

/// Collect free variables from a lambda expression.
fn collect_free_vars_lambda(
    checker: &TypeChecker<'_>,
    params: ori_ir::ParamRange,
    body: ExprId,
    bound: &mut BoundContext<'_>,
    free: &mut HashSet<Name>,
) {
    bound.with_scope(|inner| {
        for param in checker.context.arena.get_params(params) {
            inner.add_binding(param.name);
        }
        collect_free_vars_impl(checker, body, inner, free);
    });
}

/// Collect free variables from a `FunctionSeq` (run, try, match).
fn collect_free_vars_function_seq_impl(
    checker: &TypeChecker<'_>,
    func_seq: &FunctionSeq,
    bound: &mut BoundContext<'_>,
    free: &mut HashSet<Name>,
) {
    match func_seq {
        FunctionSeq::Run {
            bindings, result, ..
        }
        | FunctionSeq::Try {
            bindings, result, ..
        } => {
            bound.with_scope(|inner| {
                for binding in checker.context.arena.get_seq_bindings(*bindings) {
                    match binding {
                        SeqBinding::Let { pattern, value, .. } => {
                            collect_free_vars_impl(checker, *value, inner, free);
                            add_pattern_bindings_to_context(pattern, inner);
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            collect_free_vars_impl(checker, *expr, inner, free);
                        }
                    }
                }
                collect_free_vars_impl(checker, *result, inner, free);
            });
        }
        FunctionSeq::Match {
            scrutinee, arms, ..
        } => {
            collect_free_vars_impl(checker, *scrutinee, bound, free);
            for arm in checker.context.arena.get_arms(*arms) {
                bound.with_scope(|inner| {
                    let pattern_names =
                        collect_match_pattern_names(&arm.pattern, checker.context.arena);
                    inner.add_bindings(pattern_names);

                    if let Some(guard_id) = arm.guard {
                        collect_free_vars_impl(checker, guard_id, inner, free);
                    }
                    collect_free_vars_impl(checker, arm.body, inner, free);
                });
            }
        }
        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => {
            collect_free_vars_impl(checker, *over, bound, free);
            if let Some(map_fn) = map {
                collect_free_vars_impl(checker, *map_fn, bound, free);
            }
            bound.with_scope(|inner| {
                let pattern_names =
                    collect_match_pattern_names(&arm.pattern, checker.context.arena);
                inner.add_bindings(pattern_names);

                if let Some(guard_id) = arm.guard {
                    collect_free_vars_impl(checker, guard_id, inner, free);
                }
                collect_free_vars_impl(checker, arm.body, inner, free);
            });
            collect_free_vars_impl(checker, *default, bound, free);
        }
    }
}

/// Add bindings from a pattern to the bound context.
fn add_pattern_bindings_to_context(pattern: &ori_ir::BindingPattern, bound: &mut BoundContext<'_>) {
    match pattern {
        ori_ir::BindingPattern::Name(name) => {
            bound.add_binding(*name);
        }
        ori_ir::BindingPattern::Struct { fields } => {
            for (field_name, opt_pattern) in fields {
                match opt_pattern {
                    Some(nested) => add_pattern_bindings_to_context(nested, bound),
                    None => {
                        bound.add_binding(*field_name);
                    }
                }
            }
        }
        ori_ir::BindingPattern::Tuple(patterns) => {
            for p in patterns {
                add_pattern_bindings_to_context(p, bound);
            }
        }
        ori_ir::BindingPattern::List { elements, rest } => {
            for p in elements {
                add_pattern_bindings_to_context(p, bound);
            }
            if let Some(rest_name) = rest {
                bound.add_binding(*rest_name);
            }
        }
        ori_ir::BindingPattern::Wildcard => {}
    }
}

/// Add bindings from a pattern to a set of bound names.
///
/// This is the public API for external callers that use `HashSet`.
#[expect(clippy::implicit_hasher, reason = "Standard HashSet sufficient here")]
pub fn add_pattern_bindings(pattern: &ori_ir::BindingPattern, bound: &mut HashSet<Name>) {
    match pattern {
        ori_ir::BindingPattern::Name(name) => {
            bound.insert(*name);
        }
        ori_ir::BindingPattern::Struct { fields } => {
            for (field_name, opt_pattern) in fields {
                match opt_pattern {
                    Some(nested) => add_pattern_bindings(nested, bound),
                    None => {
                        bound.insert(*field_name);
                    }
                }
            }
        }
        ori_ir::BindingPattern::Tuple(patterns) => {
            for p in patterns {
                add_pattern_bindings(p, bound);
            }
        }
        ori_ir::BindingPattern::List { elements, rest } => {
            for p in elements {
                add_pattern_bindings(p, bound);
            }
            if let Some(rest_name) = rest {
                bound.insert(*rest_name);
            }
        }
        ori_ir::BindingPattern::Wildcard => {}
    }
}
