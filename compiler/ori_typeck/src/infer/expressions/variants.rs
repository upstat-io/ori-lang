//! Result/Option variant constructor type inference.

use super::super::infer_expr;
use crate::checker::TypeChecker;
use ori_ir::ExprId;
use ori_types::Type;

/// Common implementation for Ok/Err variant type inference.
fn infer_result_variant(checker: &mut TypeChecker<'_>, inner: Option<ExprId>, is_ok: bool) -> Type {
    let inner_ty = inner.map_or(Type::Unit, |id| infer_expr(checker, id));
    let fresh = checker.inference.ctx.fresh_var();
    if is_ok {
        checker.inference.ctx.make_result(inner_ty, fresh)
    } else {
        checker.inference.ctx.make_result(fresh, inner_ty)
    }
}

/// Infer type for Ok variant constructor.
pub fn infer_ok(checker: &mut TypeChecker<'_>, inner: Option<ExprId>) -> Type {
    infer_result_variant(checker, inner, true)
}

/// Infer type for Err variant constructor.
pub fn infer_err(checker: &mut TypeChecker<'_>, inner: Option<ExprId>) -> Type {
    infer_result_variant(checker, inner, false)
}

/// Infer type for Some variant constructor.
pub fn infer_some(checker: &mut TypeChecker<'_>, inner: ExprId) -> Type {
    let inner_ty = infer_expr(checker, inner);
    checker.inference.ctx.make_option(inner_ty)
}

/// Infer type for None variant constructor.
pub fn infer_none(checker: &mut TypeChecker<'_>) -> Type {
    let inner = checker.inference.ctx.fresh_var();
    checker.inference.ctx.make_option(inner)
}
