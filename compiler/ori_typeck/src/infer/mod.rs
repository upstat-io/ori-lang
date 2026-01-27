//! Type inference for expressions.
//!
//! This module contains the expression type inference logic, split into:
//! - `expr.rs`: Literals, binary/unary operations, identifiers
//! - `call.rs`: Function calls, method calls
//! - `control.rs`: Control flow (if/else, match, loops)
//! - `pattern.rs`: Pattern expressions (run, try, match, map, etc.)
//! - `match_binding.rs`: Match pattern binding extraction

pub mod builtin_methods;
mod call;
mod control;
mod expr;
mod match_binding;
mod pattern;

use super::checker::bound_checking;
use crate::checker::TypeChecker;
use crate::ensure_sufficient_stack;
use ori_ir::{ExprId, ExprKind, FunctionSeq, Name, SeqBinding, StmtKind};
use ori_types::Type;
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
        ExprKind::Continue => Type::Never,

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
fn collect_free_vars_function_seq(
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

/// Infer a let binding's initializer type with closure self-capture check.
///
/// This is the first step of let binding type checking:
/// 1. Checks for closure self-capture
/// 2. Infers and returns the initializer type
///
/// Use `check_type_annotation` afterwards to check against type annotations.
pub fn infer_let_init(
    checker: &mut TypeChecker<'_>,
    pattern: &ori_ir::BindingPattern,
    value: ori_ir::ExprId,
    span: ori_ir::Span,
) -> Type {
    checker.check_closure_self_capture(pattern, value, span);
    infer_expr(checker, value)
}

/// Check an optional type annotation (`ParsedType`) against a binding type.
///
/// If a type annotation is present, unifies it with `binding_ty` and
/// returns the declared type. Otherwise returns `binding_ty` unchanged.
///
/// This is the second step of let binding type checking, after `infer_let_init`.
/// For `run` patterns, `binding_ty` is the init type.
/// For `try` patterns, `binding_ty` is the unwrapped type (Result/Option inner type).
pub fn check_type_annotation(
    checker: &mut TypeChecker<'_>,
    ty: Option<&ori_ir::ParsedType>,
    binding_ty: Type,
    value: ori_ir::ExprId,
) -> Type {
    if let Some(parsed_ty) = ty {
        let declared_ty = checker.parsed_type_to_type(parsed_ty);
        if let Err(e) = checker.inference.ctx.unify(&declared_ty, &binding_ty) {
            checker.report_type_error(&e, checker.context.arena.get_expr(value).span);
        }
        declared_ty
    } else {
        binding_ty
    }
}

/// Check an optional type annotation (`TypeId`) against a binding type.
///
/// Same as `check_type_annotation` but takes a `TypeId` instead of `ParsedType`.
/// Used for block statements where the type is already resolved.
pub fn check_type_annotation_id(
    checker: &mut TypeChecker<'_>,
    ty: Option<ori_ir::TypeId>,
    binding_ty: Type,
    value: ori_ir::ExprId,
) -> Type {
    if let Some(type_id) = ty {
        let declared_ty = checker.type_id_to_type(type_id);
        if let Err(e) = checker.inference.ctx.unify(&declared_ty, &binding_ty) {
            checker.report_type_error(&e, checker.context.arena.get_expr(value).span);
        }
        declared_ty
    } else {
        binding_ty
    }
}
