//! Binary and unary operation type checking.
//!
//! Operators are desugared to trait method calls for user-defined types.
//! Primitives use fast-path direct dispatch.

use super::super::infer_expr;
use crate::checker::TypeChecker;
use crate::operators::{check_binary_operation, TypeOpResult};
use ori_ir::{BinaryOp, ExprId, Span, UnaryOp};
use ori_types::Type;

/// Maps a binary operator to its trait name and method name.
fn binary_op_to_trait(op: BinaryOp) -> Option<(&'static str, &'static str)> {
    match op {
        BinaryOp::Add => Some(("Add", "add")),
        BinaryOp::Sub => Some(("Sub", "subtract")),
        BinaryOp::Mul => Some(("Mul", "multiply")),
        BinaryOp::Div => Some(("Div", "divide")),
        BinaryOp::FloorDiv => Some(("FloorDiv", "floor_divide")),
        BinaryOp::Mod => Some(("Rem", "remainder")),
        BinaryOp::BitAnd => Some(("BitAnd", "bit_and")),
        BinaryOp::BitOr => Some(("BitOr", "bit_or")),
        BinaryOp::BitXor => Some(("BitXor", "bit_xor")),
        BinaryOp::Shl => Some(("Shl", "shift_left")),
        BinaryOp::Shr => Some(("Shr", "shift_right")),
        // Comparison and logical operators are NOT trait-based
        // They use Eq and Comparable traits directly
        _ => None,
    }
}

/// Maps a unary operator to its trait name and method name.
#[allow(dead_code)] // Reserved for future use
fn unary_op_to_trait(op: UnaryOp) -> Option<(&'static str, &'static str)> {
    match op {
        UnaryOp::Neg => Some(("Neg", "negate")),
        UnaryOp::Not => Some(("Not", "not")),
        UnaryOp::BitNot => Some(("BitNot", "bit_not")),
        UnaryOp::Try => None, // Try is special, not trait-based
    }
}

/// Infer the type of a binary operation (e.g., `a + b`, `x == y`, `p && q`).
///
/// Delegates to the type operator registry to determine valid operand combinations
/// and result types. Arithmetic, comparison, logical, and bitwise operators each
/// have specific type requirements.
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
/// First tries primitive operation checking. If that fails and the left operand
/// is a user-defined type, attempts trait-based operator dispatch.
fn check_binary_op(
    checker: &mut TypeChecker<'_>,
    op: BinaryOp,
    left: &Type,
    right: &Type,
    span: Span,
) -> Type {
    let resolved_left = checker.inference.ctx.resolve(left);
    let resolved_right = checker.inference.ctx.resolve(right);

    // First try primitive operation checking
    match check_binary_operation(
        &mut checker.inference.ctx,
        checker.context.interner,
        op,
        left,
        right,
        span,
    ) {
        TypeOpResult::Ok(ty) => return ty,
        TypeOpResult::Err(e) => {
            // For primitive types, report the error immediately
            if is_primitive_type(&resolved_left) {
                checker.push_error(e.message, span, e.code);
                return Type::Error;
            }
            // For user-defined types, try trait lookup below
        }
    }

    // For user-defined types, try trait-based operator dispatch
    if let Some((trait_name, method_name)) = binary_op_to_trait(op) {
        if let Some(result_ty) = check_operator_trait(
            checker,
            &resolved_left,
            &resolved_right,
            trait_name,
            method_name,
            span,
        ) {
            return result_ty;
        }
    }

    // No trait impl found
    let left_type = resolved_left.display(checker.context.interner);
    let right_type = resolved_right.display(checker.context.interner);
    checker.error_invalid_binary_op(span, op.as_symbol().to_string(), left_type, right_type);
    Type::Error
}

/// Check if a type is a primitive (built-in) type.
fn is_primitive_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Int
            | Type::Float
            | Type::Bool
            | Type::Str
            | Type::Char
            | Type::Byte
            | Type::Unit
            | Type::Duration
            | Type::Size
            | Type::Never
            | Type::List(_)
            | Type::Map { .. }
            | Type::Set(_)
            | Type::Option(_)
            | Type::Result { .. }
            | Type::Tuple(_)
            | Type::Range(_)
            | Type::Function { .. }
    )
}

/// Check if a type implements an operator trait and return the Output type.
fn check_operator_trait(
    checker: &mut TypeChecker<'_>,
    left_ty: &Type,
    right_ty: &Type,
    trait_name: &str,
    method_name: &str,
    span: Span,
) -> Option<Type> {
    let method_name_interned = checker.context.interner.intern(method_name);

    // Look up the method in the trait registry
    if let Some(method_lookup) = checker
        .registries
        .traits
        .lookup_method(left_ty, method_name_interned)
    {
        // The method should have 2 params: self and rhs
        if method_lookup.params.len() >= 2 {
            let rhs_param = &method_lookup.params[1];

            // Check that the right operand type matches
            if let Err(_e) = checker.inference.ctx.unify(rhs_param, right_ty) {
                let expected = rhs_param.display(checker.context.interner);
                let found = right_ty.display(checker.context.interner);
                checker.error_operator_type_mismatch(span, trait_name, expected, found);
                return Some(Type::Error);
            }

            return Some(method_lookup.return_ty.clone());
        }
    }

    None
}

/// Infer the type of a unary operation (e.g., `-x`, `!p`, `~n`, `result?`).
///
/// Validates operand types and returns result:
/// - `Neg` (`-`): requires `int` or `float`, returns same type
/// - `Not` (`!`): requires `bool`, returns `bool`
/// - `BitNot` (`~`): requires `int`, returns `int`
/// - `Try` (`?`): requires `Result<T, E>`, returns `T` (propagates error)
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
///
/// First tries primitive operation checking. If that fails and the operand
/// is a user-defined type, attempts trait-based operator dispatch.
fn check_unary_op(checker: &mut TypeChecker<'_>, op: UnaryOp, operand: &Type, span: Span) -> Type {
    let resolved = checker.inference.ctx.resolve(operand);

    match op {
        UnaryOp::Neg => {
            match &resolved {
                Type::Int | Type::Float | Type::Duration | Type::Var(_) => resolved,
                Type::Size => {
                    checker.error_invalid_unary_op(span, "-", "Size");
                    Type::Error
                }
                _ if is_primitive_type(&resolved) => {
                    let operand_type = operand.display(checker.context.interner);
                    checker.error_invalid_unary_op(span, "-", operand_type);
                    Type::Error
                }
                // User-defined type: try trait lookup
                _ => {
                    if let Some(result_ty) =
                        check_unary_operator_trait(checker, &resolved, "Neg", "neg", span)
                    {
                        result_ty
                    } else {
                        let operand_type = operand.display(checker.context.interner);
                        checker.error_invalid_unary_op(span, "-", operand_type);
                        Type::Error
                    }
                }
            }
        }
        UnaryOp::Not => {
            // First try bool
            if checker.inference.ctx.unify(operand, &Type::Bool).is_ok() {
                return Type::Bool;
            }

            // For non-bool, try Not trait on user types
            if !is_primitive_type(&resolved) {
                if let Some(result_ty) =
                    check_unary_operator_trait(checker, &resolved, "Not", "not", span)
                {
                    return result_ty;
                }
            }

            let operand_type = operand.display(checker.context.interner);
            checker.error_invalid_unary_op(span, "!", operand_type);
            Type::Error
        }
        UnaryOp::BitNot => {
            // First try int
            if checker.inference.ctx.unify(operand, &Type::Int).is_ok() {
                return Type::Int;
            }

            // For non-int, try BitNot trait on user types
            if !is_primitive_type(&resolved) {
                if let Some(result_ty) =
                    check_unary_operator_trait(checker, &resolved, "BitNot", "bit_not", span)
                {
                    return result_ty;
                }
            }

            let operand_type = operand.display(checker.context.interner);
            checker.error_invalid_unary_op(span, "~", operand_type);
            Type::Error
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

/// Check if a type implements a unary operator trait and return the Output type.
fn check_unary_operator_trait(
    checker: &mut TypeChecker<'_>,
    operand_ty: &Type,
    _trait_name: &str,
    method_name: &str,
    _span: Span,
) -> Option<Type> {
    let method_name_interned = checker.context.interner.intern(method_name);

    // Look up the method in the trait registry
    if let Some(method_lookup) = checker
        .registries
        .traits
        .lookup_method(operand_ty, method_name_interned)
    {
        // Unary operator method should have 1 param (self)
        if !method_lookup.params.is_empty() {
            return Some(method_lookup.return_ty.clone());
        }
    }

    None
}
