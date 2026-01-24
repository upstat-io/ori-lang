//! Expression type inference for literals, identifiers, and operators.

use crate::ir::{
    Name, Span, ExprId, BinaryOp, UnaryOp,
    ExprRange, ParamRange, TypeId, MapEntryRange, FieldInitRange,
};
use crate::types::Type;
use super::super::checker::{TypeChecker, TypeCheckError};
use super::super::operators::TypeOpResult;
use super::infer_expr;

/// Infer type for an identifier.
pub fn infer_ident(checker: &mut TypeChecker<'_>, name: Name, span: Span) -> Type {
    if let Some(scheme) = checker.env.lookup_scheme(name) {
        // Instantiate the scheme to get fresh type variables
        // This is key for let-polymorphism: each use of a polymorphic
        // variable gets its own fresh type variables
        checker.ctx.instantiate(scheme)
    } else {
        checker.errors.push(TypeCheckError {
            message: format!(
                "unknown identifier `{}`",
                checker.interner.lookup(name)
            ),
            span,
            code: crate::diagnostic::ErrorCode::E2003,
        });
        Type::Error
    }
}

/// Infer type for a function reference.
pub fn infer_function_ref(checker: &mut TypeChecker<'_>, name: Name, span: Span) -> Type {
    // Look up function type and instantiate for polymorphism
    if let Some(scheme) = checker.env.lookup_scheme(name) {
        checker.ctx.instantiate(scheme)
    } else {
        checker.errors.push(TypeCheckError {
            message: format!(
                "unknown function `@{}`",
                checker.interner.lookup(name)
            ),
            span,
            code: crate::diagnostic::ErrorCode::E2003,
        });
        Type::Error
    }
}

/// Infer type for a binary operation.
pub fn infer_binary(
    checker: &mut TypeChecker<'_>,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
    span: Span,
) -> Type {
    let left_ty = infer_expr(checker, left);
    let right_ty = infer_expr(checker, right);
    check_binary_op(checker, op, &left_ty, &right_ty, span)
}

/// Check a binary operation.
///
/// Delegates to the TypeOperatorRegistry for type checking.
fn check_binary_op(
    checker: &mut TypeChecker<'_>,
    op: BinaryOp,
    left: &Type,
    right: &Type,
    span: Span,
) -> Type {
    match checker.type_operator_registry.check(
        &mut checker.ctx,
        checker.interner,
        op,
        left,
        right,
        span,
    ) {
        TypeOpResult::Ok(ty) => ty,
        TypeOpResult::Err(e) => {
            checker.errors.push(TypeCheckError {
                message: e.message,
                span,
                code: e.code,
            });
            Type::Error
        }
    }
}

/// Infer type for a unary operation.
pub fn infer_unary(
    checker: &mut TypeChecker<'_>,
    op: UnaryOp,
    operand: ExprId,
    span: Span,
) -> Type {
    let operand_ty = infer_expr(checker, operand);
    check_unary_op(checker, op, &operand_ty, span)
}

/// Check a unary operation.
fn check_unary_op(
    checker: &mut TypeChecker<'_>,
    op: UnaryOp,
    operand: &Type,
    span: Span,
) -> Type {
    match op {
        UnaryOp::Neg => {
            let resolved = checker.ctx.resolve(operand);
            match resolved {
                Type::Int | Type::Float => resolved,
                Type::Var(_) => resolved, // Defer checking for type variables
                _ => {
                    checker.errors.push(TypeCheckError {
                        message: format!(
                            "cannot negate `{}`: negation requires a numeric type (int or float)",
                            operand.display(checker.interner)
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2001,
                    });
                    Type::Error
                }
            }
        }
        UnaryOp::Not => {
            if let Err(e) = checker.ctx.unify(operand, &Type::Bool) {
                checker.report_type_error(e, span);
            }
            Type::Bool
        }
        UnaryOp::BitNot => {
            if let Err(e) = checker.ctx.unify(operand, &Type::Int) {
                checker.report_type_error(e, span);
            }
            Type::Int
        }
        UnaryOp::Try => {
            // ?expr: Result<T, E> -> T (propagates E)
            let ok_ty = checker.ctx.fresh_var();
            let err_ty = checker.ctx.fresh_var();
            let result_ty = Type::Result {
                ok: Box::new(ok_ty.clone()),
                err: Box::new(err_ty),
            };
            if let Err(e) = checker.ctx.unify(operand, &result_ty) {
                checker.report_type_error(e, span);
            }
            checker.ctx.resolve(&ok_ty)
        }
    }
}

/// Infer type for a lambda expression.
pub fn infer_lambda(
    checker: &mut TypeChecker<'_>,
    params: ParamRange,
    ret_ty: Option<TypeId>,
    body: ExprId,
    _span: Span,
) -> Type {
    let params_slice = checker.arena.get_params(params);
    let param_types: Vec<Type> = params_slice
        .iter()
        .map(|p| {
            match p.ty {
                Some(type_id) => checker.type_id_to_type(type_id),
                None => checker.ctx.fresh_var(),
            }
        })
        .collect();

    // Create scope for lambda
    let mut lambda_env = checker.env.child();
    for (param, ty) in params_slice.iter().zip(param_types.iter()) {
        lambda_env.bind(param.name, ty.clone());
    }

    let old_env = std::mem::replace(&mut checker.env, lambda_env);
    let body_ty = infer_expr(checker, body);
    checker.env = old_env;

    // Use declared return type if present, otherwise inferred
    let final_ret_ty = match ret_ty {
        Some(type_id) => {
            let declared_ty = checker.type_id_to_type(type_id);
            // Unify declared with inferred
            if let Err(e) = checker.ctx.unify(&declared_ty, &body_ty) {
                checker.report_type_error(e, checker.arena.get_expr(body).span);
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

/// Infer type for a list literal.
pub fn infer_list(checker: &mut TypeChecker<'_>, elements: ExprRange) -> Type {
    let element_ids = checker.arena.get_expr_list(elements);

    if element_ids.is_empty() {
        // Empty list: element type is unknown
        Type::List(Box::new(checker.ctx.fresh_var()))
    } else {
        // Infer element types and unify
        let first_ty = infer_expr(checker, element_ids[0]);
        for id in &element_ids[1..] {
            let elem_ty = infer_expr(checker, *id);
            if let Err(e) = checker.ctx.unify(&first_ty, &elem_ty) {
                checker.report_type_error(e, checker.arena.get_expr(*id).span);
            }
        }
        Type::List(Box::new(first_ty))
    }
}

/// Infer type for a tuple literal.
pub fn infer_tuple(checker: &mut TypeChecker<'_>, elements: ExprRange) -> Type {
    let element_ids = checker.arena.get_expr_list(elements);
    if element_ids.is_empty() {
        // Empty tuple is unit type
        Type::Unit
    } else {
        let types: Vec<Type> = element_ids.iter()
            .map(|id| infer_expr(checker, *id))
            .collect();
        Type::Tuple(types)
    }
}

/// Infer type for a map literal.
pub fn infer_map(
    checker: &mut TypeChecker<'_>,
    entries: MapEntryRange,
    _span: Span,
) -> Type {
    let map_entries = checker.arena.get_map_entries(entries);
    if map_entries.is_empty() {
        // Empty map: key and value types are unknown
        Type::Map {
            key: Box::new(checker.ctx.fresh_var()),
            value: Box::new(checker.ctx.fresh_var()),
        }
    } else {
        let first_key_ty = infer_expr(checker, map_entries[0].key);
        let first_val_ty = infer_expr(checker, map_entries[0].value);
        for entry in &map_entries[1..] {
            let key_ty = infer_expr(checker, entry.key);
            let val_ty = infer_expr(checker, entry.value);
            if let Err(e) = checker.ctx.unify(&first_key_ty, &key_ty) {
                checker.report_type_error(e, entry.span);
            }
            if let Err(e) = checker.ctx.unify(&first_val_ty, &val_ty) {
                checker.report_type_error(e, entry.span);
            }
        }
        Type::Map {
            key: Box::new(first_key_ty),
            value: Box::new(first_val_ty),
        }
    }
}

/// Infer type for a struct literal.
pub fn infer_struct(
    checker: &mut TypeChecker<'_>,
    _name: Name,
    fields: FieldInitRange,
) -> Type {
    let field_inits = checker.arena.get_field_inits(fields);
    for init in field_inits {
        if let Some(value_id) = init.value {
            infer_expr(checker, value_id);
        }
    }
    // TODO: return proper struct type
    checker.ctx.fresh_var()
}

/// Infer type for a range expression.
pub fn infer_range(
    checker: &mut TypeChecker<'_>,
    start: Option<ExprId>,
    end: Option<ExprId>,
    _inclusive: bool,
    _span: Span,
) -> Type {
    let elem_ty = if let Some(start_id) = start {
        infer_expr(checker, start_id)
    } else if let Some(end_id) = end {
        infer_expr(checker, end_id)
    } else {
        Type::Int // unbounded range defaults to int
    };

    if let Some(_start_id) = start {
        if let Some(end_id) = end {
            let end_ty = infer_expr(checker, end_id);
            if let Err(e) = checker.ctx.unify(&elem_ty, &end_ty) {
                checker.report_type_error(e, checker.arena.get_expr(end_id).span);
            }
        }
    }
    // TODO: Range<T> type
    Type::List(Box::new(elem_ty))
}

/// Infer type for field access.
pub fn infer_field(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    _field: Name,
) -> Type {
    let _receiver_ty = infer_expr(checker, receiver);
    // TODO: implement proper field access type checking
    checker.ctx.fresh_var()
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

    match checker.ctx.resolve(&receiver_ty) {
        // List indexing: list[int] -> T (panics on out-of-bounds)
        Type::List(elem_ty) => {
            if let Err(e) = checker.ctx.unify(&index_ty, &Type::Int) {
                checker.report_type_error(e, checker.arena.get_expr(index).span);
            }
            (*elem_ty).clone()
        }
        // Map indexing: map[K] -> Option<V> (None if key missing)
        Type::Map { key, value } => {
            if let Err(e) = checker.ctx.unify(&index_ty, &key) {
                checker.report_type_error(e, checker.arena.get_expr(index).span);
            }
            Type::Option(value)
        }
        // String indexing: str[int] -> str (single codepoint)
        Type::Str => {
            if let Err(e) = checker.ctx.unify(&index_ty, &Type::Int) {
                checker.report_type_error(e, checker.arena.get_expr(index).span);
            }
            Type::Str
        }
        // Type variable - defer checking
        Type::Var(_) => checker.ctx.fresh_var(),
        // Error recovery
        Type::Error => Type::Error,
        // Other types - not indexable
        other => {
            checker.errors.push(TypeCheckError {
                message: format!(
                    "type `{}` is not indexable",
                    other.display(checker.interner)
                ),
                span,
                code: crate::diagnostic::ErrorCode::E2001,
            });
            Type::Error
        }
    }
}

/// Infer type for Ok variant constructor.
pub fn infer_ok(checker: &mut TypeChecker<'_>, inner: Option<ExprId>) -> Type {
    let ok_ty = if let Some(id) = inner {
        infer_expr(checker, id)
    } else {
        Type::Unit
    };
    Type::Result {
        ok: Box::new(ok_ty),
        err: Box::new(checker.ctx.fresh_var()),
    }
}

/// Infer type for Err variant constructor.
pub fn infer_err(checker: &mut TypeChecker<'_>, inner: Option<ExprId>) -> Type {
    let err_ty = if let Some(id) = inner {
        infer_expr(checker, id)
    } else {
        Type::Unit
    };
    Type::Result {
        ok: Box::new(checker.ctx.fresh_var()),
        err: Box::new(err_ty),
    }
}

/// Infer type for Some variant constructor.
pub fn infer_some(checker: &mut TypeChecker<'_>, inner: ExprId) -> Type {
    let inner_ty = infer_expr(checker, inner);
    Type::Option(Box::new(inner_ty))
}

/// Infer type for None variant constructor.
pub fn infer_none(checker: &mut TypeChecker<'_>) -> Type {
    Type::Option(Box::new(checker.ctx.fresh_var()))
}
