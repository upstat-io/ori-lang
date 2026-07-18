//! Validation and concrete evaluation for fixed-list capacity annotations.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, ParsedType, Span, UnaryOp};

use crate::const_eval::{is_int_const_binary_op, EvaluatedConstExpr, GenericConstExpr};
use crate::{ConstValue, Idx, InvalidFixedListCapacityReason, TypeCheckError};

use super::super::InferEngine;

/// Validate every fixed-list capacity nested in a user-written annotation.
pub(crate) fn validate_fixed_list_capacities(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    parsed: &ParsedType,
) {
    match parsed {
        ParsedType::FixedList { elem, capacity } => {
            validate_fixed_list_capacity(engine, arena, *capacity);
            validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(*elem));
        }
        ParsedType::List(elem) => {
            validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(*elem));
        }
        ParsedType::Map { key, value } => {
            validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(*key));
            validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(*value));
        }
        ParsedType::Tuple(elements) | ParsedType::TraitBounds(elements) => {
            for &element in arena.get_parsed_type_list(*elements) {
                validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(element));
            }
        }
        ParsedType::Function { params, ret } => {
            for &param in arena.get_parsed_type_list(*params) {
                validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(param));
            }
            validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(*ret));
        }
        ParsedType::Named { type_args, .. } => {
            for &arg in arena.get_parsed_type_list(*type_args) {
                validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(arg));
            }
        }
        ParsedType::AssociatedType { base, .. } => {
            validate_fixed_list_capacities(engine, arena, arena.get_parsed_type(*base));
        }
        ParsedType::Primitive(_)
        | ParsedType::Infer
        | ParsedType::SelfType
        | ParsedType::ConstExpr(_) => {}
    }
}

fn validate_fixed_list_capacity(engine: &mut InferEngine<'_>, arena: &ExprArena, capacity: ExprId) {
    let mut names = Vec::new();
    if !validate_capacity_names(engine, arena, capacity, &mut names) {
        return;
    }

    match evaluate_capacity_expr(engine, arena, capacity) {
        Ok(EvaluatedConstExpr::Concrete(ConstValue::Int(value))) if value <= 0 => {
            let span = arena.get_expr(capacity).span;
            engine.push_error(TypeCheckError::non_positive_fixed_list_capacity(
                span, value,
            ));
        }
        Ok(EvaluatedConstExpr::Concrete(ConstValue::Bool(_))) => {
            let span = arena.get_expr(capacity).span;
            engine.push_error(TypeCheckError::invalid_fixed_list_capacity_expression(
                span,
                InvalidFixedListCapacityReason::NonInteger,
            ));
        }
        Ok(EvaluatedConstExpr::Concrete(ConstValue::Int(_)) | EvaluatedConstExpr::Symbolic) => {}
        Err(reason) => {
            let span = arena.get_expr(capacity).span;
            engine.push_error(TypeCheckError::invalid_fixed_list_capacity_expression(
                span, reason,
            ));
        }
    }
}

/// Validate every name in an admitted capacity-expression shape. Both sides
/// are visited even after an error so one annotation reports all bad names.
fn validate_capacity_names(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
    names: &mut Vec<Name>,
) -> bool {
    match arena.get_expr(expr).kind {
        ExprKind::Ident(name) | ExprKind::Const(name) => {
            if names.contains(&name) {
                true
            } else {
                names.push(name);
                validate_capacity_name(engine, arena.get_expr(expr).span, name)
            }
        }
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        } => validate_capacity_names(engine, arena, operand, names),
        ExprKind::Binary { op, left, right } if is_int_const_binary_op(op) => {
            let left_valid = validate_capacity_names(engine, arena, left, names);
            let right_valid = validate_capacity_names(engine, arena, right, names);
            left_valid && right_valid
        }
        _ => true,
    }
}

fn validate_capacity_name(engine: &mut InferEngine<'_>, span: Span, name: Name) -> bool {
    if let Some(value) = engine.const_value(name) {
        return match value {
            ConstValue::Int(_) => true,
            ConstValue::Bool(_) => {
                engine.push_error(TypeCheckError::invalid_fixed_list_capacity_expression(
                    span,
                    InvalidFixedListCapacityReason::NonInteger,
                ));
                false
            }
        };
    }

    if let Some(param_ty) = engine.const_param_type(name) {
        if engine.resolve(param_ty) == Idx::INT {
            return true;
        }
        engine.push_error(TypeCheckError::invalid_fixed_list_capacity_expression(
            span,
            InvalidFixedListCapacityReason::NonInteger,
        ));
        return false;
    }

    if engine.has_lexical_binding(name) {
        engine.push_error(TypeCheckError::undeclared_fixed_list_capacity_const(
            span, name,
        ));
        return false;
    }

    if let Some(module_ty) = engine.module_const_type(name) {
        let resolved = engine.resolve(module_ty);
        let reason = if resolved == Idx::INT || engine.pool().tag(resolved).is_type_variable() {
            InvalidFixedListCapacityReason::UnsupportedExpression
        } else {
            InvalidFixedListCapacityReason::NonInteger
        };
        engine.push_error(TypeCheckError::invalid_fixed_list_capacity_expression(
            span, reason,
        ));
        return false;
    }

    engine.push_error(TypeCheckError::undeclared_fixed_list_capacity_const(
        span, name,
    ));
    false
}

fn evaluate_capacity_expr(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
) -> Result<EvaluatedConstExpr, InvalidFixedListCapacityReason> {
    GenericConstExpr::from_arena(arena, expr)?.evaluate(&mut |name| engine.const_value(name))
}

/// Evaluate the concrete generic-const shapes admitted by type inference.
pub(super) fn generic_const_value(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
) -> Option<ConstValue> {
    let expr = GenericConstExpr::from_arena(arena, expr).ok()?;
    match expr.evaluate(&mut |name| engine.const_value(name)).ok()? {
        EvaluatedConstExpr::Concrete(value) => Some(value),
        EvaluatedConstExpr::Symbolic => None,
    }
}
