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
            // Provide helpful error with valid indices
            let message = if elems.is_empty() {
                "unit type `()` has no fields".to_string()
            } else if elems.len() == 1 {
                format!("tuple has no field `{field_name}` (valid index: 0)")
            } else {
                format!(
                    "tuple has no field `{field_name}` (valid indices: 0..{})",
                    elems.len() - 1
                )
            };
            checker.push_error(message, receiver_span, ori_diagnostic::ErrorCode::E2001);
            Type::Error
        }

        Type::Applied {
            name: type_name,
            args,
        } => handle_struct_field_access(checker, type_name, field, Some(&args), receiver_span),

        Type::ModuleNamespace { .. } => {
            // Look up the field in the module namespace using the helper method
            if let Some(item_type) = resolved_ty.get_namespace_item(field) {
                return item_type.clone();
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
            let type_str = resolved_ty.display(checker.context.interner);
            // Provide type-specific suggestions
            let suggestion = match &resolved_ty {
                Type::Int | Type::Float | Type::Bool | Type::Char | Type::Byte => {
                    " (primitives do not have fields)"
                }
                Type::Function { .. } => " (functions must be called, not accessed with `.`)",
                Type::Option(_) => " (use `.unwrap()` or pattern matching to access inner value)",
                Type::Result { .. } => " (use `.unwrap()` or pattern matching to access Ok value)",
                Type::List(_) => " (use indexing `[i]` or methods like `.first()`, `.last()`)",
                Type::Map { .. } => " (use indexing `[key]` or methods like `.get()`)",
                _ => "",
            };
            checker.push_error(
                format!("type `{type_str}` does not support field access{suggestion}"),
                receiver_span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }
    }
}

/// Infer the type of an index access expression (e.g., `list[0]`, `map["key"]`).
///
/// Validates that the receiver is indexable and the index type matches:
/// - `List<T>` indexed by `int` returns `T`
/// - `Map<K, V>` indexed by `K` returns `Option<V>`
/// - `str` indexed by `int` returns `str`
///
/// Reports an error if the receiver is not indexable or the index type mismatches.
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
            let type_str = other.display(checker.context.interner);
            // Provide type-specific suggestions
            let suggestion = match &other {
                Type::Named(_) | Type::Applied { .. } => {
                    " (structs use field access with `.field`)"
                }
                Type::Tuple(_) => " (tuples use field access with `.0`, `.1`, etc.)",
                Type::Option(_) => " (use `.unwrap()` or pattern matching to access inner value)",
                Type::Result { .. } => " (use `.unwrap()` or pattern matching to access Ok value)",
                _ => "",
            };
            checker.push_error(
                format!("type `{type_str}` is not indexable{suggestion}"),
                span,
                ori_diagnostic::ErrorCode::E2001,
            );
            Type::Error
        }
    }
}
