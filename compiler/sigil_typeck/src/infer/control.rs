//! Control flow expression type inference.
//!
//! Handles if/else, match, loops, blocks, and other control flow.

use sigil_ir::{Name, Span, ExprId, StmtRange, ArmRange, ParsedType, BindingPattern};
use sigil_types::Type;
use crate::checker::{TypeChecker, TypeCheckError};
use super::infer_expr;

/// Infer type for an if expression.
pub fn infer_if(
    checker: &mut TypeChecker<'_>,
    cond: ExprId,
    then_branch: ExprId,
    else_branch: Option<ExprId>,
    span: Span,
) -> Type {
    let cond_ty = infer_expr(checker, cond);

    if let Err(e) = checker.inference.ctx.unify(&cond_ty, &Type::Bool) {
        checker.report_type_error(&e, checker.context.arena.get_expr(cond).span);
    }

    let then_ty = infer_expr(checker, then_branch);

    if let Some(else_id) = else_branch {
        let else_ty = infer_expr(checker, else_id);

        if let Err(e) = checker.inference.ctx.unify(&then_ty, &else_ty) {
            checker.report_type_error(&e, span);
        }

        then_ty
    } else {
        Type::Unit
    }
}

/// Infer type for a match expression.
pub fn infer_match(
    checker: &mut TypeChecker<'_>,
    scrutinee: ExprId,
    arms: ArmRange,
    _span: Span,
) -> Type {
    let scrutinee_ty = infer_expr(checker, scrutinee);
    let match_arms = checker.context.arena.get_arms(arms);

    if match_arms.is_empty() {
        checker.inference.ctx.fresh_var()
    } else {
        let mut result_ty: Option<Type> = None;

        for arm in match_arms {
            super::unify_pattern_with_scrutinee(checker, &arm.pattern, &scrutinee_ty, arm.span);

            let bindings = super::extract_match_pattern_bindings(checker, &arm.pattern, &scrutinee_ty);

            let mut arm_env = checker.inference.env.child();
            for (name, ty) in bindings {
                arm_env.bind(name, ty);
            }
            let old_env = std::mem::replace(&mut checker.inference.env, arm_env);

            if let Some(guard_id) = arm.guard {
                let guard_ty = infer_expr(checker, guard_id);
                if let Err(e) = checker.inference.ctx.unify(&guard_ty, &Type::Bool) {
                    checker.report_type_error(&e, checker.context.arena.get_expr(guard_id).span);
                }
            }

            let arm_ty = infer_expr(checker, arm.body);

            checker.inference.env = old_env;

            match &result_ty {
                Some(expected) => {
                    if let Err(e) = checker.inference.ctx.unify(expected, &arm_ty) {
                        checker.report_type_error(&e, arm.span);
                    }
                }
                None => {
                    result_ty = Some(arm_ty);
                }
            }
        }

        result_ty.unwrap_or_else(|| checker.inference.ctx.fresh_var())
    }
}

/// Infer type for a for loop.
pub fn infer_for(
    checker: &mut TypeChecker<'_>,
    binding: Name,
    iter: ExprId,
    guard: Option<ExprId>,
    body: ExprId,
    is_yield: bool,
    _span: Span,
) -> Type {
    let iter_ty = infer_expr(checker, iter);
    let resolved = checker.inference.ctx.resolve(&iter_ty);
    let elem_ty = match resolved {
        Type::List(elem) | Type::Set(elem) | Type::Range(elem) => *elem,
        Type::Str => Type::Str,
        Type::Map { key, value: _ } => *key,
        Type::Var(_) => checker.inference.ctx.fresh_var(),
        Type::Error => Type::Error,
        other => {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "`{}` is not iterable",
                    other.display(checker.context.interner)
                ),
                span: checker.context.arena.get_expr(iter).span,
                code: sigil_diagnostic::ErrorCode::E2001,
            });
            Type::Error
        }
    };

    let mut loop_env = checker.inference.env.child();
    loop_env.bind(binding, elem_ty);
    let old_env = std::mem::replace(&mut checker.inference.env, loop_env);

    if let Some(guard_id) = guard {
        let guard_ty = infer_expr(checker, guard_id);
        if let Err(e) = checker.inference.ctx.unify(&guard_ty, &Type::Bool) {
            checker.report_type_error(&e, checker.context.arena.get_expr(guard_id).span);
        }
    }

    let body_ty = infer_expr(checker, body);
    checker.inference.env = old_env;

    if is_yield {
        Type::List(Box::new(body_ty))
    } else {
        Type::Unit
    }
}

/// Infer type for a loop expression.
pub fn infer_loop(checker: &mut TypeChecker<'_>, body: ExprId) -> Type {
    let _body_ty = infer_expr(checker, body);
    checker.inference.ctx.fresh_var()
}

/// Infer type for a block expression.
pub fn infer_block(
    checker: &mut TypeChecker<'_>,
    stmts: StmtRange,
    result: Option<ExprId>,
    _span: Span,
) -> Type {
    let block_env = checker.inference.env.child();
    let old_env = std::mem::replace(&mut checker.inference.env, block_env);

    for stmt in checker.context.arena.get_stmt_range(stmts) {
        match &stmt.kind {
            sigil_ir::StmtKind::Expr(e) => {
                infer_expr(checker, *e);
            }
            sigil_ir::StmtKind::Let { pattern, ty, init, .. } => {
                checker.check_closure_self_capture(pattern, *init, stmt.span);

                let init_ty = infer_expr(checker, *init);
                let final_ty = if let Some(type_id) = ty {
                    let declared_ty = checker.type_id_to_type(*type_id);
                    if let Err(e) = checker.inference.ctx.unify(&declared_ty, &init_ty) {
                        checker.report_type_error(&e, checker.context.arena.get_expr(*init).span);
                    }
                    declared_ty
                } else {
                    init_ty
                };
                checker.bind_pattern_generalized(pattern, final_ty);
            }
        }
    }

    let result_ty = if let Some(result_id) = result {
        infer_expr(checker, result_id)
    } else {
        Type::Unit
    };

    checker.inference.env = old_env;
    result_ty
}

/// Infer type for a let binding (as expression).
pub fn infer_let(
    checker: &mut TypeChecker<'_>,
    pattern: &BindingPattern,
    ty: Option<&ParsedType>,
    init: ExprId,
    span: Span,
) -> Type {
    checker.check_closure_self_capture(pattern, init, span);

    let init_ty = infer_expr(checker, init);
    let final_ty = if let Some(parsed_ty) = ty {
        let declared_ty = checker.parsed_type_to_type(parsed_ty);
        if let Err(e) = checker.inference.ctx.unify(&declared_ty, &init_ty) {
            checker.report_type_error(&e, checker.context.arena.get_expr(init).span);
        }
        declared_ty
    } else {
        init_ty
    };
    checker.bind_pattern_generalized(pattern, final_ty);
    Type::Unit
}

/// Infer type for return expression.
pub fn infer_return(checker: &mut TypeChecker<'_>, value: Option<ExprId>) -> Type {
    if let Some(id) = value {
        infer_expr(checker, id);
    }
    Type::Never
}

/// Infer type for break expression.
pub fn infer_break(checker: &mut TypeChecker<'_>, value: Option<ExprId>) -> Type {
    if let Some(id) = value {
        infer_expr(checker, id);
    }
    Type::Never
}

/// Infer type for await expression.
pub fn infer_await(checker: &mut TypeChecker<'_>, inner: ExprId, span: Span) -> Type {
    let _ = infer_expr(checker, inner);
    checker.diagnostics.errors.push(TypeCheckError {
        message: "`.await` is not supported; use `uses Async` capability and `parallel(...)` pattern".to_string(),
        span,
        code: sigil_diagnostic::ErrorCode::E2001,
    });
    Type::Error
}

/// Infer type for try expression.
pub fn infer_try(checker: &mut TypeChecker<'_>, inner: ExprId, _span: Span) -> Type {
    let inner_ty = infer_expr(checker, inner);
    let resolved = checker.inference.ctx.resolve(&inner_ty);
    match resolved {
        Type::Result { ok, err: _ } => *ok,
        Type::Option(inner) => *inner,
        Type::Var(_) => checker.inference.ctx.fresh_var(),
        Type::Error => Type::Error,
        other => {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "the `?` operator can only be applied to `Result` or `Option`, \
                     found `{}`",
                    other.display(checker.context.interner)
                ),
                span: checker.context.arena.get_expr(inner).span,
                code: sigil_diagnostic::ErrorCode::E2001,
            });
            Type::Error
        }
    }
}

/// Infer type for assignment expression.
pub fn infer_assign(
    checker: &mut TypeChecker<'_>,
    target: ExprId,
    value: ExprId,
    _span: Span,
) -> Type {
    let target_ty = infer_expr(checker, target);
    let value_ty = infer_expr(checker, value);
    if let Err(e) = checker.inference.ctx.unify(&target_ty, &value_ty) {
        checker.report_type_error(&e, checker.context.arena.get_expr(value).span);
    }
    value_ty
}
