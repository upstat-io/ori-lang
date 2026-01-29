//! Control flow expression type inference.
//!
//! Handles if/else, match, loops, blocks, and other control flow.

use super::infer_expr;
use crate::checker::TypeChecker;
use ori_ir::{ArmRange, BindingPattern, ExprId, Name, ParsedType, Span, StmtRange};
use ori_types::Type;

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

            let bindings =
                super::extract_match_pattern_bindings(checker, &arm.pattern, &scrutinee_ty);

            let arm_ty = checker.with_infer_bindings(bindings, |checker| {
                if let Some(guard_id) = arm.guard {
                    let guard_ty = infer_expr(checker, guard_id);
                    if let Err(e) = checker.inference.ctx.unify(&guard_ty, &Type::Bool) {
                        checker
                            .report_type_error(&e, checker.context.arena.get_expr(guard_id).span);
                    }
                }

                infer_expr(checker, arm.body)
            });

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
            checker.push_error(
                format!(
                    "`{}` is not iterable; expected List, Set, Range, Str, or Map",
                    other.display(checker.context.interner)
                ),
                checker.context.arena.get_expr(iter).span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }
    };

    let body_ty = checker.with_infer_bindings(vec![(binding, elem_ty)], |checker| {
        if let Some(guard_id) = guard {
            let guard_ty = infer_expr(checker, guard_id);
            if let Err(e) = checker.inference.ctx.unify(&guard_ty, &Type::Bool) {
                checker.report_type_error(&e, checker.context.arena.get_expr(guard_id).span);
            }
        }

        infer_expr(checker, body)
    });

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
    checker.with_infer_env_scope(|checker| {
        for stmt in checker.context.arena.get_stmt_range(stmts) {
            match &stmt.kind {
                ori_ir::StmtKind::Expr(e) => {
                    infer_expr(checker, *e);
                }
                ori_ir::StmtKind::Let {
                    pattern, ty, init, ..
                } => {
                    let init_ty = super::infer_let_init(checker, pattern, *init, stmt.span);
                    let final_ty = super::check_type_annotation_id(checker, *ty, init_ty, *init);
                    checker.bind_pattern_generalized(pattern, final_ty, stmt.span);
                }
            }
        }

        if let Some(result_id) = result {
            infer_expr(checker, result_id)
        } else {
            Type::Unit
        }
    })
}

/// Infer type for a let binding (as expression).
pub fn infer_let(
    checker: &mut TypeChecker<'_>,
    pattern: &BindingPattern,
    ty: Option<&ParsedType>,
    init: ExprId,
    span: Span,
) -> Type {
    let init_ty = super::infer_let_init(checker, pattern, init, span);
    let final_ty = super::check_type_annotation(checker, ty, init_ty, init);
    checker.bind_pattern_generalized(pattern, final_ty, span);
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
    checker.push_error(
        "`.await` is not supported; use `uses Async` capability and `parallel(...)` pattern",
        span,
        ori_diagnostic::ErrorCode::E2001,
    );
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
            checker.push_error(
                format!(
                    "the `?` operator can only be applied to `Result` or `Option`, \
                     found `{}`",
                    other.display(checker.context.interner)
                ),
                checker.context.arena.get_expr(inner).span,
                ori_diagnostic::ErrorCode::E2001,
            );
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
