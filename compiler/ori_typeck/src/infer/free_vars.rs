//! Free variable collection for closure self-capture detection.

use crate::checker::TypeChecker;
use ori_ir::{ExprId, ExprKind, FunctionSeq, Name, SeqBinding, StmtKind};
use std::collections::HashSet;

use super::collect_match_pattern_names;

/// Collect free variables from an expression (inner recursive helper).
///
/// This is used for closure self-capture detection.
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
    let expr = checker.context.arena.get_expr(expr_id);

    match &expr.kind {
        // Variable or function reference - free if not bound
        ExprKind::Ident(name) | ExprKind::FunctionRef(name) => {
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
            for arg_id in checker.context.arena.get_expr_list(*args) {
                collect_free_vars_inner(checker, *arg_id, bound, free);
            }
        }

        // Named call
        ExprKind::CallNamed { func, args } => {
            collect_free_vars_inner(checker, *func, bound, free);
            for arg in checker.context.arena.get_call_args(*args) {
                collect_free_vars_inner(checker, arg.value, bound, free);
            }
        }

        // Method call
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_free_vars_inner(checker, *receiver, bound, free);
            for arg_id in checker.context.arena.get_expr_list(*args) {
                collect_free_vars_inner(checker, *arg_id, bound, free);
            }
        }

        // Method call with named arguments
        ExprKind::MethodCallNamed { receiver, args, .. } => {
            collect_free_vars_inner(checker, *receiver, bound, free);
            for arg in checker.context.arena.get_call_args(*args) {
                collect_free_vars_inner(checker, arg.value, bound, free);
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
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_free_vars_inner(checker, *cond, bound, free);
            collect_free_vars_inner(checker, *then_branch, bound, free);
            if let Some(else_id) = else_branch {
                collect_free_vars_inner(checker, *else_id, bound, free);
            }
        }

        // Match expression
        ExprKind::Match { scrutinee, arms } => {
            collect_free_vars_inner(checker, *scrutinee, bound, free);
            for arm in checker.context.arena.get_arms(*arms) {
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
        ExprKind::For {
            binding,
            iter,
            guard,
            body,
            ..
        } => {
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
            for stmt in checker.context.arena.get_stmt_range(*stmts) {
                match &stmt.kind {
                    StmtKind::Expr(e) => {
                        collect_free_vars_inner(checker, *e, &block_bound, free);
                    }
                    StmtKind::Let { pattern, init, .. } => {
                        // Init is evaluated before the binding is in scope
                        collect_free_vars_inner(checker, *init, &block_bound, free);
                        // Add pattern bindings for subsequent statements
                        add_pattern_bindings(pattern, &mut block_bound);
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
            for param in checker.context.arena.get_params(*params) {
                lambda_bound.insert(param.name);
            }
            collect_free_vars_inner(checker, *body, &lambda_bound, free);
        }

        // List and Tuple
        ExprKind::List(elements) | ExprKind::Tuple(elements) => {
            for elem_id in checker.context.arena.get_expr_list(*elements) {
                collect_free_vars_inner(checker, *elem_id, bound, free);
            }
        }

        // Map
        ExprKind::Map(entries) => {
            for entry in checker.context.arena.get_map_entries(*entries) {
                collect_free_vars_inner(checker, entry.key, bound, free);
                collect_free_vars_inner(checker, entry.value, bound, free);
            }
        }

        // Struct literal
        ExprKind::Struct { fields, .. } => {
            for init in checker.context.arena.get_field_inits(*fields) {
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

        // Expressions with single inner expression
        ExprKind::Some(inner) | ExprKind::Await(inner) | ExprKind::Try(inner) => {
            collect_free_vars_inner(checker, *inner, bound, free);
        }

        // Control flow with optional value
        ExprKind::Return(value) | ExprKind::Break(value) => {
            if let Some(id) = value {
                collect_free_vars_inner(checker, *id, bound, free);
            }
        }

        ExprKind::Assign { target, value } => {
            collect_free_vars_inner(checker, *target, bound, free);
            collect_free_vars_inner(checker, *value, bound, free);
        }

        // WithCapability
        ExprKind::WithCapability { provider, body, .. } => {
            collect_free_vars_inner(checker, *provider, bound, free);
            collect_free_vars_inner(checker, *body, bound, free);
        }

        // FunctionSeq
        ExprKind::FunctionSeq(func_seq) => {
            collect_free_vars_function_seq(checker, func_seq, bound, free);
        }

        // FunctionExp
        ExprKind::FunctionExp(func_exp) => {
            for prop in checker.context.arena.get_named_exprs(func_exp.props) {
                collect_free_vars_inner(checker, prop.value, bound, free);
            }
        }
    }
}

/// Collect free variables from a `FunctionSeq` (run, try, match).
pub fn collect_free_vars_function_seq(
    checker: &TypeChecker<'_>,
    func_seq: &FunctionSeq,
    bound: &HashSet<Name>,
    free: &mut HashSet<Name>,
) {
    match func_seq {
        FunctionSeq::Run {
            bindings, result, ..
        }
        | FunctionSeq::Try {
            bindings, result, ..
        } => {
            let mut seq_bound = bound.clone();
            for binding in checker.context.arena.get_seq_bindings(*bindings) {
                match binding {
                    SeqBinding::Let { pattern, value, .. } => {
                        collect_free_vars_inner(checker, *value, &seq_bound, free);
                        add_pattern_bindings(pattern, &mut seq_bound);
                    }
                    SeqBinding::Stmt { expr, .. } => {
                        collect_free_vars_inner(checker, *expr, &seq_bound, free);
                    }
                }
            }
            collect_free_vars_inner(checker, *result, &seq_bound, free);
        }
        FunctionSeq::Match {
            scrutinee, arms, ..
        } => {
            collect_free_vars_inner(checker, *scrutinee, bound, free);
            for arm in checker.context.arena.get_arms(*arms) {
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
        FunctionSeq::ForPattern {
            over,
            map,
            arm,
            default,
            ..
        } => {
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

/// Add bindings from a pattern to a set of bound names.
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
