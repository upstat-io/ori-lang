//! Field and index access type inference.

use super::super::infer_expr;
use super::structs::handle_struct_field_access;
use crate::checker::TypeChecker;
use ori_ir::{ExprId, Name, Span};
use ori_types::Type;

/// Infer type for field access.
pub fn infer_field(checker: &mut TypeChecker<'_>, receiver: ExprId, field: Name) -> Type {
    let receiver_ty = infer_expr(checker, receiver);
    let resolved_ty = checker.inference.ctx.resolve(&receiver_ty);
    let resolved_ty = checker.resolve_through_aliases(&resolved_ty);
    let receiver_span = checker.context.arena.get_expr(receiver).span;

    match resolved_ty {
        Type::Named(type_name) => {
            handle_struct_field_access(checker, type_name, field, None, receiver_span)
        }

        Type::Tuple(elems) => {
            let field_name = checker.context.interner.lookup(field);
            if let Ok(index) = field_name.parse::<usize>() {
                if index < elems.len() {
                    return elems[index].clone();
                }
            }
            checker.push_error(
                format!("tuple has no field `{field_name}`"),
                receiver_span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }

        Type::Applied {
            name: type_name,
            args,
        } => handle_struct_field_access(checker, type_name, field, Some(&args), receiver_span),

        Type::ModuleNamespace { items } => {
            // Look up the field in the module namespace
            for (item_name, item_type) in items {
                if item_name == field {
                    return item_type.clone();
                }
            }
            // Field not found in module namespace
            let field_name = checker.context.interner.lookup(field);
            checker.push_error(
                format!("module has no exported item `{field_name}`"),
                receiver_span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }

        Type::Var(_) => checker.inference.ctx.fresh_var(),
        Type::Error => Type::Error,

        _ => {
            checker.push_error(
                format!("type `{resolved_ty:?}` does not support field access"),
                receiver_span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }
    }
}

/// Infer type for index access.
pub fn infer_index(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    index: ExprId,
    span: Span,
) -> Type {
    let receiver_ty = infer_expr(checker, receiver);
    let index_ty = infer_expr(checker, index);

    match checker.inference.ctx.resolve(&receiver_ty) {
        Type::List(elem_ty) => {
            if let Err(e) = checker.inference.ctx.unify(&index_ty, &Type::Int) {
                checker.report_type_error(&e, checker.context.arena.get_expr(index).span);
            }
            (*elem_ty).clone()
        }
        Type::Map { key, value } => {
            if let Err(e) = checker.inference.ctx.unify(&index_ty, &key) {
                checker.report_type_error(&e, checker.context.arena.get_expr(index).span);
            }
            Type::Option(value)
        }
        Type::Str => {
            if let Err(e) = checker.inference.ctx.unify(&index_ty, &Type::Int) {
                checker.report_type_error(&e, checker.context.arena.get_expr(index).span);
            }
            Type::Str
        }
        Type::Var(_) => checker.inference.ctx.fresh_var(),
        Type::Error => Type::Error,
        other => {
            checker.push_error(
                format!(
                    "type `{}` is not indexable",
                    other.display(checker.context.interner)
                ),
                span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }
    }
}
