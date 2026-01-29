//! Lambda expression type inference.

use super::super::infer_expr;
use crate::checker::TypeChecker;
use ori_ir::{ExprId, ParamRange, ParsedType, Span};
use ori_types::Type;

/// Infer the type of a lambda expression (e.g., `x -> x + 1`, `(a, b) -> a + b`).
///
/// Returns a `Function` type with parameter and return types:
/// - Typed parameters use declared types; untyped get fresh type variables
/// - Return type inferred from body, or validated against declared return type
///
/// Binds parameter names in scope for body inference.
pub fn infer_lambda(
    checker: &mut TypeChecker<'_>,
    params: ParamRange,
    ret_ty: Option<&ParsedType>,
    body: ExprId,
    _span: Span,
) -> Type {
    let params_slice = checker.context.arena.get_params(params);
    let param_types: Vec<Type> = params_slice
        .iter()
        .map(|p| match &p.ty {
            Some(parsed_ty) => checker.parsed_type_to_type(parsed_ty),
            None => checker.inference.ctx.fresh_var(),
        })
        .collect();

    let bindings: Vec<_> = params_slice
        .iter()
        .zip(param_types.iter())
        .map(|(param, ty)| (param.name, ty.clone()))
        .collect();

    let body_ty = checker.with_infer_bindings(bindings, |checker| infer_expr(checker, body));

    let final_ret_ty = match ret_ty {
        Some(parsed_ty) => {
            let declared_ty = checker.parsed_type_to_type(parsed_ty);
            if let Err(e) = checker.inference.ctx.unify(&declared_ty, &body_ty) {
                checker.report_type_error(&e, checker.context.arena.get_expr(body).span);
            }
            declared_ty
        }
        None => body_ty,
    };

    Type::Function {
        params: param_types,
        ret: Box::new(final_ret_ty),
    }
}
