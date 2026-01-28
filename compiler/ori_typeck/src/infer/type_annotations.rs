//! Type annotation checking for let bindings.

use super::infer_expr;
use crate::checker::TypeChecker;
use ori_ir::{ExprId, ParsedType, Span, TypeId};
use ori_types::Type;

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
    value: ExprId,
    span: Span,
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
    ty: Option<&ParsedType>,
    binding_ty: Type,
    value: ExprId,
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
    ty: Option<TypeId>,
    binding_ty: Type,
    value: ExprId,
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
