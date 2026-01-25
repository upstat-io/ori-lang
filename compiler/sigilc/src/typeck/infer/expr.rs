//! Expression type inference for literals, identifiers, and operators.

use crate::ir::{
    Name, Span, ExprId, BinaryOp, UnaryOp,
    ExprRange, ParamRange, ParsedType, MapEntryRange, FieldInitRange,
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
        // Check for built-in type conversion functions (function_val)
        let name_str = checker.interner.lookup(name);
        if let Some(ty) = builtin_function_type(checker, name_str) {
            return ty;
        }

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

/// Get the type for a built-in function (function_val).
///
/// Returns `Some(Type)` for recognized built-in functions, `None` otherwise.
fn builtin_function_type(checker: &mut TypeChecker<'_>, name: &str) -> Option<Type> {
    // Type conversion functions: polymorphic (T) -> Target
    // The actual type checking is done at runtime; here we just
    // provide a function type that accepts any argument.
    match name {
        "str" => {
            let param = checker.ctx.fresh_var();
            Some(Type::Function {
                params: vec![param],
                ret: Box::new(Type::Str),
            })
        }
        "int" => {
            let param = checker.ctx.fresh_var();
            Some(Type::Function {
                params: vec![param],
                ret: Box::new(Type::Int),
            })
        }
        "float" => {
            let param = checker.ctx.fresh_var();
            Some(Type::Function {
                params: vec![param],
                ret: Box::new(Type::Float),
            })
        }
        "byte" => {
            let param = checker.ctx.fresh_var();
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
/// Delegates to the `TypeOperatorRegistry` for type checking.
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
                // Valid numeric types and deferred type variables
                Type::Int | Type::Float | Type::Var(_) => resolved,
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
                checker.report_type_error(&e, span);
            }
            Type::Bool
        }
        UnaryOp::BitNot => {
            if let Err(e) = checker.ctx.unify(operand, &Type::Int) {
                checker.report_type_error(&e, span);
            }
            Type::Int
        }
        UnaryOp::Try => {
            // ?expr: Result<T, E> -> T (propagates E)
            let ok_ty = checker.ctx.fresh_var();
            let err_ty = checker.ctx.fresh_var();
            let result_ty = checker.ctx.make_result(ok_ty.clone(), err_ty);
            if let Err(e) = checker.ctx.unify(operand, &result_ty) {
                checker.report_type_error(&e, span);
            }
            checker.ctx.resolve(&ok_ty)
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
    let params_slice = checker.arena.get_params(params);
    let param_types: Vec<Type> = params_slice
        .iter()
        .map(|p| {
            match &p.ty {
                Some(parsed_ty) => checker.parsed_type_to_type(parsed_ty),
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
        Some(parsed_ty) => {
            let declared_ty = checker.parsed_type_to_type(parsed_ty);
            // Unify declared with inferred
            if let Err(e) = checker.ctx.unify(&declared_ty, &body_ty) {
                checker.report_type_error(&e, checker.arena.get_expr(body).span);
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
        let elem = checker.ctx.fresh_var();
        checker.ctx.make_list(elem)
    } else {
        // Infer element types and unify
        let first_ty = infer_expr(checker, element_ids[0]);
        for id in &element_ids[1..] {
            let elem_ty = infer_expr(checker, *id);
            if let Err(e) = checker.ctx.unify(&first_ty, &elem_ty) {
                checker.report_type_error(&e, checker.arena.get_expr(*id).span);
            }
        }
        checker.ctx.make_list(first_ty)
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
        checker.ctx.make_tuple(types)
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
        let key = checker.ctx.fresh_var();
        let value = checker.ctx.fresh_var();
        checker.ctx.make_map(key, value)
    } else {
        let first_key_ty = infer_expr(checker, map_entries[0].key);
        let first_val_ty = infer_expr(checker, map_entries[0].value);
        for entry in &map_entries[1..] {
            let key_ty = infer_expr(checker, entry.key);
            let val_ty = infer_expr(checker, entry.value);
            if let Err(e) = checker.ctx.unify(&first_key_ty, &key_ty) {
                checker.report_type_error(&e, entry.span);
            }
            if let Err(e) = checker.ctx.unify(&first_val_ty, &val_ty) {
                checker.report_type_error(&e, entry.span);
            }
        }
        checker.ctx.make_map(first_key_ty, first_val_ty)
    }
}

/// Infer type for a struct literal.
pub fn infer_struct(
    checker: &mut TypeChecker<'_>,
    name: Name,
    fields: FieldInitRange,
) -> Type {
    use std::collections::HashSet;
    use crate::typeck::type_registry::TypeKind;

    // Look up the struct type in the registry
    let type_entry = if let Some(entry) = checker.type_registry.get_by_name(name) { entry.clone() } else {
        // Unknown struct type
        let field_inits = checker.arena.get_field_inits(fields);
        let span = if let Some(first) = field_inits.first() {
            first.span
        } else {
            crate::ir::Span::new(0, 0)
        };

        checker.errors.push(TypeCheckError {
            message: format!(
                "unknown struct type `{}`",
                checker.interner.lookup(name)
            ),
            span,
            code: crate::diagnostic::ErrorCode::E2003,
        });

        // Still infer field types for better error reporting
        for init in field_inits {
            if let Some(value_id) = init.value {
                infer_expr(checker, value_id);
            }
        }
        return Type::Error;
    };

    // Verify it's actually a struct type
    let expected_fields = if let TypeKind::Struct { fields } = &type_entry.kind { fields.clone() } else {
        let field_inits = checker.arena.get_field_inits(fields);
        let span = if let Some(first) = field_inits.first() {
            first.span
        } else {
            crate::ir::Span::new(0, 0)
        };

        checker.errors.push(TypeCheckError {
            message: format!(
                "`{}` is not a struct type",
                checker.interner.lookup(name)
            ),
            span,
            code: crate::diagnostic::ErrorCode::E2001,
        });
        return Type::Error;
    };

    // Build a map of expected field names to types
    let expected_map: std::collections::HashMap<Name, Type> = expected_fields
        .iter()
        .cloned()
        .collect();

    // Check provided fields
    let field_inits = checker.arena.get_field_inits(fields);
    let mut provided_fields: HashSet<Name> = HashSet::new();

    for init in field_inits {
        // Check for duplicate fields
        if !provided_fields.insert(init.name) {
            checker.errors.push(TypeCheckError {
                message: format!(
                    "field `{}` specified more than once",
                    checker.interner.lookup(init.name)
                ),
                span: init.span,
                code: crate::diagnostic::ErrorCode::E2001,
            });
            continue;
        }

        // Check if field exists in struct
        if let Some(expected_ty) = expected_map.get(&init.name) {
            // Infer the value type and unify with expected
            if let Some(value_id) = init.value {
                let actual_ty = infer_expr(checker, value_id);
                if let Err(e) = checker.ctx.unify(&actual_ty, expected_ty) {
                    checker.report_type_error(&e, init.span);
                }
            } else {
                // Shorthand syntax: { field } means { field: field }
                // Look up the variable with the same name as the field
                let var_ty = infer_ident(checker, init.name, init.span);
                if let Err(e) = checker.ctx.unify(&var_ty, expected_ty) {
                    checker.report_type_error(&e, init.span);
                }
            }
        } else {
            // Unknown field
            checker.errors.push(TypeCheckError {
                message: format!(
                    "struct `{}` has no field `{}`",
                    checker.interner.lookup(name),
                    checker.interner.lookup(init.name)
                ),
                span: init.span,
                code: crate::diagnostic::ErrorCode::E2001,
            });

            // Still infer the value for error recovery
            if let Some(value_id) = init.value {
                infer_expr(checker, value_id);
            }
        }
    }

    // Check for missing fields
    for (field_name, _) in &expected_fields {
        if !provided_fields.contains(field_name) {
            let span = if let Some(last) = field_inits.last() {
                last.span
            } else {
                crate::ir::Span::new(0, 0)
            };

            checker.errors.push(TypeCheckError {
                message: format!(
                    "missing field `{}` in struct `{}`",
                    checker.interner.lookup(*field_name),
                    checker.interner.lookup(name)
                ),
                span,
                code: crate::diagnostic::ErrorCode::E2001,
            });
        }
    }

    // Return the struct type
    Type::Named(name)
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
                checker.report_type_error(&e, checker.arena.get_expr(end_id).span);
            }
        }
    }
    checker.ctx.make_range(elem_ty)
}

/// Infer type for field access.
pub fn infer_field(
    checker: &mut TypeChecker<'_>,
    receiver: ExprId,
    field: Name,
) -> Type {
    use crate::typeck::type_registry::TypeKind;

    let receiver_ty = infer_expr(checker, receiver);
    let resolved_ty = checker.ctx.resolve(&receiver_ty);
    // Resolve through any type aliases
    let resolved_ty = checker.resolve_through_aliases(&resolved_ty);
    let receiver_span = checker.arena.get_expr(receiver).span;

    match resolved_ty {
        // Struct field access: Point.x -> int
        Type::Named(type_name) => {
            // Look up the struct type in the registry
            if let Some(entry) = checker.type_registry.get_by_name(type_name) {
                let entry = entry.clone();
                match &entry.kind {
                    TypeKind::Struct { fields } => {
                        // Find the field
                        for (field_name, field_ty) in fields {
                            if *field_name == field {
                                return field_ty.clone();
                            }
                        }

                        // Field not found
                        checker.errors.push(TypeCheckError {
                            message: format!(
                                "struct `{}` has no field `{}`",
                                checker.interner.lookup(type_name),
                                checker.interner.lookup(field)
                            ),
                            span: receiver_span,
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                    TypeKind::Enum { .. } => {
                        checker.errors.push(TypeCheckError {
                            message: format!(
                                "cannot access field `{}` on enum type `{}`",
                                checker.interner.lookup(field),
                                checker.interner.lookup(type_name)
                            ),
                            span: receiver_span,
                            code: crate::diagnostic::ErrorCode::E2001,
                        });
                        Type::Error
                    }
                    TypeKind::Alias { .. } => {
                        // This shouldn't happen - aliases resolved above
                        Type::Error
                    }
                }
            } else {
                checker.errors.push(TypeCheckError {
                    message: format!(
                        "unknown type `{}`",
                        checker.interner.lookup(type_name)
                    ),
                    span: receiver_span,
                    code: crate::diagnostic::ErrorCode::E2003,
                });
                Type::Error
            }
        }

        // Tuple field access: tuple.0, tuple.1, etc.
        Type::Tuple(elems) => {
            let field_name = checker.interner.lookup(field);
            if let Ok(index) = field_name.parse::<usize>() {
                if index < elems.len() {
                    return elems[index].clone();
                }
            }
            checker.errors.push(TypeCheckError {
                message: format!(
                    "tuple has no field `{field_name}`"
                ),
                span: receiver_span,
                code: crate::diagnostic::ErrorCode::E2001,
            });
            Type::Error
        }

        // Type variable - defer checking
        Type::Var(_) => checker.ctx.fresh_var(),

        // Error recovery
        Type::Error => Type::Error,

        // Any other type
        _ => {
            checker.errors.push(TypeCheckError {
                message: format!(
                    "type `{resolved_ty:?}` does not support field access"
                ),
                span: receiver_span,
                code: crate::diagnostic::ErrorCode::E2001,
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

    match checker.ctx.resolve(&receiver_ty) {
        // List indexing: list[int] -> T (panics on out-of-bounds)
        Type::List(elem_ty) => {
            if let Err(e) = checker.ctx.unify(&index_ty, &Type::Int) {
                checker.report_type_error(&e, checker.arena.get_expr(index).span);
            }
            (*elem_ty).clone()
        }
        // Map indexing: map[K] -> Option<V> (None if key missing)
        Type::Map { key, value } => {
            if let Err(e) = checker.ctx.unify(&index_ty, &key) {
                checker.report_type_error(&e, checker.arena.get_expr(index).span);
            }
            Type::Option(value)
        }
        // String indexing: str[int] -> str (single codepoint)
        Type::Str => {
            if let Err(e) = checker.ctx.unify(&index_ty, &Type::Int) {
                checker.report_type_error(&e, checker.arena.get_expr(index).span);
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
    let err_ty = checker.ctx.fresh_var();
    checker.ctx.make_result(ok_ty, err_ty)
}

/// Infer type for Err variant constructor.
pub fn infer_err(checker: &mut TypeChecker<'_>, inner: Option<ExprId>) -> Type {
    let err_ty = if let Some(id) = inner {
        infer_expr(checker, id)
    } else {
        Type::Unit
    };
    let ok_ty = checker.ctx.fresh_var();
    checker.ctx.make_result(ok_ty, err_ty)
}

/// Infer type for Some variant constructor.
pub fn infer_some(checker: &mut TypeChecker<'_>, inner: ExprId) -> Type {
    let inner_ty = infer_expr(checker, inner);
    checker.ctx.make_option(inner_ty)
}

/// Infer type for None variant constructor.
pub fn infer_none(checker: &mut TypeChecker<'_>) -> Type {
    let inner = checker.ctx.fresh_var();
    checker.ctx.make_option(inner)
}
