//! Expression type inference for literals, identifiers, and operators.

use sigil_ir::{
    Name, Span, ExprId, BinaryOp, UnaryOp,
    ExprRange, ParamRange, ParsedType, MapEntryRange, FieldInitRange,
};
use sigil_types::Type;
use crate::checker::{TypeChecker, TypeCheckError};
use crate::operators::TypeOpResult;
use crate::registry::TypeKind;
use super::infer_expr;
use std::collections::HashMap;

/// Substitute type parameter names with their corresponding type variables.
fn substitute_type_params(ty: &Type, params: &HashMap<Name, Type>) -> Type {
    match ty {
        Type::Named(name) => {
            if let Some(replacement) = params.get(name) {
                replacement.clone()
            } else {
                ty.clone()
            }
        }
        Type::List(inner) => {
            Type::List(Box::new(substitute_type_params(inner, params)))
        }
        Type::Option(inner) => {
            Type::Option(Box::new(substitute_type_params(inner, params)))
        }
        Type::Result { ok, err } => {
            Type::Result {
                ok: Box::new(substitute_type_params(ok, params)),
                err: Box::new(substitute_type_params(err, params)),
            }
        }
        Type::Map { key, value } => {
            Type::Map {
                key: Box::new(substitute_type_params(key, params)),
                value: Box::new(substitute_type_params(value, params)),
            }
        }
        Type::Set(inner) => {
            Type::Set(Box::new(substitute_type_params(inner, params)))
        }
        Type::Tuple(elems) => {
            Type::Tuple(elems.iter().map(|e| substitute_type_params(e, params)).collect())
        }
        Type::Function { params: fn_params, ret } => {
            Type::Function {
                params: fn_params.iter().map(|p| substitute_type_params(p, params)).collect(),
                ret: Box::new(substitute_type_params(ret, params)),
            }
        }
        Type::Range(inner) => {
            Type::Range(Box::new(substitute_type_params(inner, params)))
        }
        Type::Channel(inner) => {
            Type::Channel(Box::new(substitute_type_params(inner, params)))
        }
        _ => ty.clone(),
    }
}

/// Infer type for an identifier.
pub fn infer_ident(checker: &mut TypeChecker<'_>, name: Name, span: Span) -> Type {
    if let Some(scheme) = checker.inference.env.lookup_scheme(name) {
        checker.inference.ctx.instantiate(scheme)
    } else {
        let name_str = checker.context.interner.lookup(name);
        if let Some(ty) = builtin_function_type(checker, name_str) {
            return ty;
        }

        checker.diagnostics.errors.push(TypeCheckError {
            message: format!(
                "unknown identifier `{}`",
                checker.context.interner.lookup(name)
            ),
            span,
            code: sigil_diagnostic::ErrorCode::E2003,
        });
        Type::Error
    }
}

/// Get the type for a built-in function (function_val).
fn builtin_function_type(checker: &mut TypeChecker<'_>, name: &str) -> Option<Type> {
    match name {
        "str" => {
            let param = checker.inference.ctx.fresh_var();
            Some(Type::Function {
                params: vec![param],
                ret: Box::new(Type::Str),
            })
        }
        "int" => {
            let param = checker.inference.ctx.fresh_var();
            Some(Type::Function {
                params: vec![param],
                ret: Box::new(Type::Int),
            })
        }
        "float" => {
            let param = checker.inference.ctx.fresh_var();
            Some(Type::Function {
                params: vec![param],
                ret: Box::new(Type::Float),
            })
        }
        "byte" => {
            let param = checker.inference.ctx.fresh_var();
            Some(Type::Function {
                params: vec![param],
                ret: Box::new(Type::Byte),
            })
        }
        _ => None,
    }
}

/// Infer type for a function reference.
pub fn infer_function_ref(checker: &mut TypeChecker<'_>, name: Name, span: Span) -> Type {
    if let Some(scheme) = checker.inference.env.lookup_scheme(name) {
        checker.inference.ctx.instantiate(scheme)
    } else {
        checker.diagnostics.errors.push(TypeCheckError {
            message: format!(
                "unknown function `@{}`",
                checker.context.interner.lookup(name)
            ),
            span,
            code: sigil_diagnostic::ErrorCode::E2003,
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
fn check_binary_op(
    checker: &mut TypeChecker<'_>,
    op: BinaryOp,
    left: &Type,
    right: &Type,
    span: Span,
) -> Type {
    match checker.registries.type_op.check(
        &mut checker.inference.ctx,
        checker.context.interner,
        op,
        left,
        right,
        span,
    ) {
        TypeOpResult::Ok(ty) => ty,
        TypeOpResult::Err(e) => {
            checker.diagnostics.errors.push(TypeCheckError {
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
            let resolved = checker.inference.ctx.resolve(operand);
            match resolved {
                Type::Int | Type::Float | Type::Var(_) => resolved,
                _ => {
                    checker.diagnostics.errors.push(TypeCheckError {
                        message: format!(
                            "cannot negate `{}`: negation requires a numeric type (int or float)",
                            operand.display(checker.context.interner)
                        ),
                        span,
                        code: sigil_diagnostic::ErrorCode::E2001,
                    });
                    Type::Error
                }
            }
        }
        UnaryOp::Not => {
            if let Err(e) = checker.inference.ctx.unify(operand, &Type::Bool) {
                checker.report_type_error(&e, span);
            }
            Type::Bool
        }
        UnaryOp::BitNot => {
            if let Err(e) = checker.inference.ctx.unify(operand, &Type::Int) {
                checker.report_type_error(&e, span);
            }
            Type::Int
        }
        UnaryOp::Try => {
            let ok_ty = checker.inference.ctx.fresh_var();
            let err_ty = checker.inference.ctx.fresh_var();
            let result_ty = checker.inference.ctx.make_result(ok_ty.clone(), err_ty);
            if let Err(e) = checker.inference.ctx.unify(operand, &result_ty) {
                checker.report_type_error(&e, span);
            }
            checker.inference.ctx.resolve(&ok_ty)
        }
    }
}

/// Infer type for a lambda expression.
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
        .map(|p| {
            match &p.ty {
                Some(parsed_ty) => checker.parsed_type_to_type(parsed_ty),
                None => checker.inference.ctx.fresh_var(),
            }
        })
        .collect();

    let mut lambda_env = checker.inference.env.child();
    for (param, ty) in params_slice.iter().zip(param_types.iter()) {
        lambda_env.bind(param.name, ty.clone());
    }

    let old_env = std::mem::replace(&mut checker.inference.env, lambda_env);
    let body_ty = infer_expr(checker, body);
    checker.inference.env = old_env;

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

/// Infer type for a list literal.
pub fn infer_list(checker: &mut TypeChecker<'_>, elements: ExprRange) -> Type {
    let element_ids = checker.context.arena.get_expr_list(elements);

    if element_ids.is_empty() {
        let elem = checker.inference.ctx.fresh_var();
        checker.inference.ctx.make_list(elem)
    } else {
        let first_ty = infer_expr(checker, element_ids[0]);
        for id in &element_ids[1..] {
            let elem_ty = infer_expr(checker, *id);
            if let Err(e) = checker.inference.ctx.unify(&first_ty, &elem_ty) {
                checker.report_type_error(&e, checker.context.arena.get_expr(*id).span);
            }
        }
        checker.inference.ctx.make_list(first_ty)
    }
}

/// Infer type for a tuple literal.
pub fn infer_tuple(checker: &mut TypeChecker<'_>, elements: ExprRange) -> Type {
    let element_ids = checker.context.arena.get_expr_list(elements);
    if element_ids.is_empty() {
        Type::Unit
    } else {
        let types: Vec<Type> = element_ids.iter()
            .map(|id| infer_expr(checker, *id))
            .collect();
        checker.inference.ctx.make_tuple(types)
    }
}

/// Infer type for a map literal.
pub fn infer_map(
    checker: &mut TypeChecker<'_>,
    entries: MapEntryRange,
    _span: Span,
) -> Type {
    let map_entries = checker.context.arena.get_map_entries(entries);
    if map_entries.is_empty() {
        let key = checker.inference.ctx.fresh_var();
        let value = checker.inference.ctx.fresh_var();
        checker.inference.ctx.make_map(key, value)
    } else {
        let first_key_ty = infer_expr(checker, map_entries[0].key);
        let first_val_ty = infer_expr(checker, map_entries[0].value);
        for entry in &map_entries[1..] {
            let key_ty = infer_expr(checker, entry.key);
            let val_ty = infer_expr(checker, entry.value);
            if let Err(e) = checker.inference.ctx.unify(&first_key_ty, &key_ty) {
                checker.report_type_error(&e, entry.span);
            }
            if let Err(e) = checker.inference.ctx.unify(&first_val_ty, &val_ty) {
                checker.report_type_error(&e, entry.span);
            }
        }
        checker.inference.ctx.make_map(first_key_ty, first_val_ty)
    }
}

/// Infer type for a struct literal.
pub fn infer_struct(
    checker: &mut TypeChecker<'_>,
    name: Name,
    fields: FieldInitRange,
) -> Type {
    use std::collections::HashSet;

    let type_entry = if let Some(entry) = checker.registries.types.get_by_name(name) { entry.clone() } else {
        let field_inits = checker.context.arena.get_field_inits(fields);
        let span = if let Some(first) = field_inits.first() {
            first.span
        } else {
            sigil_ir::Span::new(0, 0)
        };

        checker.diagnostics.errors.push(TypeCheckError {
            message: format!(
                "unknown struct type `{}`",
                checker.context.interner.lookup(name)
            ),
            span,
            code: sigil_diagnostic::ErrorCode::E2003,
        });

        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(checker, value_id);
            }
        }
        return Type::Error;
    };

    let expected_fields = if let TypeKind::Struct { fields } = &type_entry.kind { fields.clone() } else {
        let field_inits = checker.context.arena.get_field_inits(fields);
        let span = if let Some(first) = field_inits.first() {
            first.span
        } else {
            sigil_ir::Span::new(0, 0)
        };

        checker.diagnostics.errors.push(TypeCheckError {
            message: format!(
                "`{}` is not a struct type",
                checker.context.interner.lookup(name)
            ),
            span,
            code: sigil_diagnostic::ErrorCode::E2001,
        });
        return Type::Error;
    };

    let (expected_fields, type_args) = if type_entry.type_params.is_empty() {
        (expected_fields, Vec::new())
    } else {
        let type_args: Vec<Type> = type_entry
            .type_params
            .iter()
            .map(|_| checker.inference.ctx.fresh_var())
            .collect();

        let type_param_vars: HashMap<Name, Type> = type_entry
            .type_params
            .iter()
            .zip(type_args.iter())
            .map(|(&param_name, type_var)| (param_name, type_var.clone()))
            .collect();

        let substituted_fields = expected_fields
            .into_iter()
            .map(|(field_name, field_ty)| {
                let substituted_ty = substitute_type_params(&field_ty, &type_param_vars);
                (field_name, substituted_ty)
            })
            .collect();

        (substituted_fields, type_args)
    };

    let expected_map: std::collections::HashMap<Name, Type> = expected_fields
        .iter()
        .cloned()
        .collect();

    let field_inits = checker.context.arena.get_field_inits(fields);
    let mut provided_fields: HashSet<Name> = HashSet::new();

    for init in field_inits {
        if !provided_fields.insert(init.name) {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "field `{}` specified more than once",
                    checker.context.interner.lookup(init.name)
                ),
                span: init.span,
                code: sigil_diagnostic::ErrorCode::E2001,
            });
            continue;
        }

        if let Some(expected_ty) = expected_map.get(&init.name) {
            if let Some(value_id) = init.value {
                let actual_ty = infer_expr(checker, value_id);
                if let Err(e) = checker.inference.ctx.unify(&actual_ty, expected_ty) {
                    checker.report_type_error(&e, init.span);
                }
            } else {
                let var_ty = infer_ident(checker, init.name, init.span);
                if let Err(e) = checker.inference.ctx.unify(&var_ty, expected_ty) {
                    checker.report_type_error(&e, init.span);
                }
            }
        } else {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "struct `{}` has no field `{}`",
                    checker.context.interner.lookup(name),
                    checker.context.interner.lookup(init.name)
                ),
                span: init.span,
                code: sigil_diagnostic::ErrorCode::E2001,
            });

            if let Some(value_id) = init.value {
                infer_expr(checker, value_id);
            }
        }
    }

    for (field_name, _) in &expected_fields {
        if !provided_fields.contains(field_name) {
            let span = if let Some(last) = field_inits.last() {
                last.span
            } else {
                sigil_ir::Span::new(0, 0)
            };

            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "missing field `{}` in struct `{}`",
                    checker.context.interner.lookup(*field_name),
                    checker.context.interner.lookup(name)
                ),
                span,
                code: sigil_diagnostic::ErrorCode::E2001,
            });
        }
    }

    if type_args.is_empty() {
        Type::Named(name)
    } else {
        Type::Applied { name, args: type_args }
    }
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
        Type::Int
    };

    if start.is_some() {
        if let Some(end_id) = end {
            let end_ty = infer_expr(checker, end_id);
            if let Err(e) = checker.inference.ctx.unify(&elem_ty, &end_ty) {
                checker.report_type_error(&e, checker.context.arena.get_expr(end_id).span);
            }
        }
    }
    checker.inference.ctx.make_range(elem_ty)
}

/// Infer type for field access.
pub fn infer_field(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    field: Name,
) -> Type {
    let receiver_ty = infer_expr(checker, receiver);
    let resolved_ty = checker.inference.ctx.resolve(&receiver_ty);
    let resolved_ty = checker.resolve_through_aliases(&resolved_ty);
    let receiver_span = checker.context.arena.get_expr(receiver).span;

    match resolved_ty {
        Type::Named(type_name) => {
            if let Some(entry) = checker.registries.types.get_by_name(type_name) {
                let entry = entry.clone();
                match &entry.kind {
                    TypeKind::Struct { fields } => {
                        for (field_name, field_ty) in fields {
                            if *field_name == field {
                                return field_ty.clone();
                            }
                        }

                        checker.diagnostics.errors.push(TypeCheckError {
                            message: format!(
                                "struct `{}` has no field `{}`",
                                checker.context.interner.lookup(type_name),
                                checker.context.interner.lookup(field)
                            ),
                            span: receiver_span,
                            code: sigil_diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                    TypeKind::Enum { .. } => {
                        checker.diagnostics.errors.push(TypeCheckError {
                            message: format!(
                                "cannot access field `{}` on enum type `{}`",
                                checker.context.interner.lookup(field),
                                checker.context.interner.lookup(type_name)
                            ),
                            span: receiver_span,
                            code: sigil_diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                    TypeKind::Alias { .. } => {
                        Type::Error
                    }
                }
            } else {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!(
                        "unknown type `{}`",
                        checker.context.interner.lookup(type_name)
                    ),
                    span: receiver_span,
                    code: sigil_diagnostic::ErrorCode::E2003,
                });
                Type::Error
            }
        }

        Type::Tuple(elems) => {
            let field_name = checker.context.interner.lookup(field);
            if let Ok(index) = field_name.parse::<usize>() {
                if index < elems.len() {
                    return elems[index].clone();
                }
            }
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!("tuple has no field `{field_name}`"),
                span: receiver_span,
                code: sigil_diagnostic::ErrorCode::E2001,
            });
            Type::Error
        }

        Type::Applied { name: type_name, args } => {
            if let Some(entry) = checker.registries.types.get_by_name(type_name) {
                let entry = entry.clone();
                match &entry.kind {
                    TypeKind::Struct { fields } => {
                        let type_param_map: HashMap<Name, Type> = entry
                            .type_params
                            .iter()
                            .zip(args.iter())
                            .map(|(&param_name, arg)| (param_name, arg.clone()))
                            .collect();

                        for (field_name, field_ty) in fields {
                            if *field_name == field {
                                return substitute_type_params(field_ty, &type_param_map);
                            }
                        }

                        checker.diagnostics.errors.push(TypeCheckError {
                            message: format!(
                                "struct `{}` has no field `{}`",
                                checker.context.interner.lookup(type_name),
                                checker.context.interner.lookup(field)
                            ),
                            span: receiver_span,
                            code: sigil_diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                    TypeKind::Enum { .. } => {
                        checker.diagnostics.errors.push(TypeCheckError {
                            message: format!(
                                "cannot access field `{}` on enum type `{}`",
                                checker.context.interner.lookup(field),
                                checker.context.interner.lookup(type_name)
                            ),
                            span: receiver_span,
                            code: sigil_diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                    TypeKind::Alias { .. } => {
                        Type::Error
                    }
                }
            } else {
                checker.diagnostics.errors.push(TypeCheckError {
                    message: format!(
                        "unknown type `{}`",
                        checker.context.interner.lookup(type_name)
                    ),
                    span: receiver_span,
                    code: sigil_diagnostic::ErrorCode::E2003,
                });
                Type::Error
            }
        }

        Type::Var(_) => checker.inference.ctx.fresh_var(),
        Type::Error => Type::Error,

        _ => {
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!("type `{resolved_ty:?}` does not support field access"),
                span: receiver_span,
                code: sigil_diagnostic::ErrorCode::E2001,
            });
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
            checker.diagnostics.errors.push(TypeCheckError {
                message: format!(
                    "type `{}` is not indexable",
                    other.display(checker.context.interner)
                ),
                span,
                code: sigil_diagnostic::ErrorCode::E2001,
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
    let err_ty = checker.inference.ctx.fresh_var();
    checker.inference.ctx.make_result(ok_ty, err_ty)
}

/// Infer type for Err variant constructor.
pub fn infer_err(checker: &mut TypeChecker<'_>, inner: Option<ExprId>) -> Type {
    let err_ty = if let Some(id) = inner {
        infer_expr(checker, id)
    } else {
        Type::Unit
    };
    let ok_ty = checker.inference.ctx.fresh_var();
    checker.inference.ctx.make_result(ok_ty, err_ty)
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
