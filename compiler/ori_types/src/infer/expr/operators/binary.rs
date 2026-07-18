//! Binary-operator inference.

use ori_ir::{BinaryOp, ExprArena, ExprId, Span};

use super::super::super::InferEngine;
use super::super::infer_expr;
use super::super::registry_bridge::is_binary_op_supported;
use super::dispatch::{
    binary_op_to_trait_name, check_cross_type_arithmetic, comparison_trait_name,
    has_comparable_trait, has_eq_trait, resolve_binary_op_via_trait,
};
use crate::{ContextKind, ErrorContext, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError};

#[derive(Clone, Copy)]
struct BinaryInputs<'a> {
    arena: &'a ExprArena,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
    span: Span,
    left_ty: Idx,
    right_ty: Idx,
}

/// Infer the type of a binary operation.
pub(crate) fn infer_binary(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    op: BinaryOp,
    left: ExprId,
    right: ExprId,
    span: Span,
) -> Idx {
    let left_ty = infer_expr(engine, arena, left);
    let right_ty = infer_expr(engine, arena, right);
    let resolved_left = engine.resolve(left_ty);
    if engine.pool().tag(resolved_left) == Tag::Never {
        return Idx::NEVER;
    }

    let inputs = BinaryInputs {
        arena,
        op,
        left,
        right,
        span,
        left_ty,
        right_ty,
    };
    match op {
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::FloorDiv
        | BinaryOp::MatMul => infer_arithmetic(engine, inputs),
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq => infer_comparison(engine, inputs),
        BinaryOp::And | BinaryOp::Or => infer_logical(engine, inputs),
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            infer_bitwise(engine, inputs)
        }
        BinaryOp::Range | BinaryOp::RangeInclusive => infer_range(engine, inputs),
        BinaryOp::Coalesce => infer_coalesce(engine, inputs),
    }
}

fn infer_arithmetic(engine: &mut InferEngine<'_>, inputs: BinaryInputs<'_>) -> Idx {
    let resolved_left = engine.resolve(inputs.left_ty);
    let resolved_right = engine.resolve(inputs.right_ty);
    let left_tag = engine.pool().tag(resolved_left);
    let right_tag = engine.pool().tag(resolved_right);
    let op_str = inputs.op.as_symbol();

    match (left_tag, right_tag) {
        (Tag::Error, _) | (_, Tag::Error) => return Idx::ERROR,
        (_, Tag::Never) => return Idx::NEVER,
        _ => {}
    }
    if let Some(result) = check_cross_type_arithmetic(left_tag, right_tag, inputs.op) {
        return result;
    }
    if left_tag == right_tag {
        if let Some(supported) = is_binary_op_supported(left_tag, inputs.op) {
            if !supported {
                push_unsupported_arithmetic(engine, inputs, resolved_left);
                return Idx::ERROR;
            }
            if left_tag == Tag::List && inputs.op == BinaryOp::Add {
                let left_element = engine.pool().list_elem(resolved_left);
                let right_element = engine.pool().list_elem(resolved_right);
                let _ = engine.unify_types(left_element, right_element);
                return engine.resolve(inputs.left_ty);
            }
            check_right_against_left(
                engine,
                inputs,
                ContextKind::BinaryOpRight { op: op_str },
                ContextKind::BinaryOpLeft { op: op_str },
            );
            return engine.resolve(inputs.left_ty);
        }
    }
    if !left_tag.is_primitive() && !left_tag.is_type_variable() {
        if let Some(result) = resolve_binary_op_via_trait(
            engine,
            inputs.arena,
            resolved_left,
            inputs.right_ty,
            inputs.right,
            inputs.op,
            inputs.span,
        ) {
            return result;
        }
        if let Some(trait_name) = binary_op_to_trait_name(inputs.op) {
            engine.push_error(TypeCheckError::unsupported_operator(
                inputs.span,
                resolved_left,
                op_str,
                trait_name,
            ));
            return Idx::ERROR;
        }
    }
    check_right_against_left(
        engine,
        inputs,
        ContextKind::BinaryOpRight { op: op_str },
        ContextKind::BinaryOpLeft { op: op_str },
    );
    engine.resolve(inputs.left_ty)
}

fn push_unsupported_arithmetic(
    engine: &mut InferEngine<'_>,
    inputs: BinaryInputs<'_>,
    resolved_left: Idx,
) {
    let op = inputs.op.as_symbol();
    if let Some(trait_name) = binary_op_to_trait_name(inputs.op) {
        engine.push_error(TypeCheckError::unsupported_operator(
            inputs.span,
            resolved_left,
            op,
            trait_name,
        ));
    } else {
        engine.push_error(TypeCheckError::bad_binary_operand(
            inputs.arena.get_expr(inputs.left).span,
            "arithmetic",
            "numeric",
            resolved_left,
        ));
    }
}

fn infer_comparison(engine: &mut InferEngine<'_>, inputs: BinaryInputs<'_>) -> Idx {
    let resolved_left = engine.resolve(inputs.left_ty);
    let left_tag = engine.pool().tag(resolved_left);
    if left_tag == Tag::Error {
        return Idx::ERROR;
    }
    let resolved_right = engine.resolve(inputs.right_ty);
    match engine.pool().tag(resolved_right) {
        Tag::Error => return Idx::ERROR,
        Tag::Never => return Idx::NEVER,
        _ => {}
    }

    if left_tag.is_primitive() && is_binary_op_supported(left_tag, inputs.op) == Some(false) {
        engine.push_error(TypeCheckError::unsupported_operator(
            inputs.span,
            resolved_left,
            inputs.op.as_symbol(),
            comparison_trait_name(inputs.op),
        ));
        return Idx::ERROR;
    }
    if matches!(left_tag, Tag::Named | Tag::Applied) {
        let unsatisfied = match inputs.op {
            BinaryOp::Eq | BinaryOp::NotEq => !has_eq_trait(engine, resolved_left),
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
                !has_comparable_trait(engine, resolved_left)
            }
            _ => unreachable!("comparison helper called with non-comparison operator"),
        };
        if unsatisfied {
            engine.push_error(TypeCheckError::unsupported_operator(
                inputs.span,
                resolved_left,
                inputs.op.as_symbol(),
                comparison_trait_name(inputs.op),
            ));
            return Idx::ERROR;
        }
        if resolve_binary_op_via_trait(
            engine,
            inputs.arena,
            resolved_left,
            inputs.right_ty,
            inputs.right,
            inputs.op,
            inputs.span,
        ) == Some(Idx::ERROR)
        {
            return Idx::ERROR;
        }
    }
    check_right_against_left(
        engine,
        inputs,
        ContextKind::ComparisonRight,
        ContextKind::ComparisonLeft,
    );
    Idx::BOOL
}

fn infer_logical(engine: &mut InferEngine<'_>, inputs: BinaryInputs<'_>) -> Idx {
    check_logical_operand(engine, inputs.arena, inputs.left, inputs.left_ty);
    check_logical_operand(engine, inputs.arena, inputs.right, inputs.right_ty);
    Idx::BOOL
}

fn check_logical_operand(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expression: ExprId,
    ty: Idx,
) {
    let span = arena.get_expr(expression).span;
    let resolved = engine.resolve(ty);
    let tag = engine.pool().tag(resolved);
    match tag {
        Tag::Bool | Tag::Error | Tag::Var | Tag::Never => {
            if tag != Tag::Never {
                let expected = Expected {
                    ty: Idx::BOOL,
                    origin: ExpectedOrigin::NoExpectation,
                };
                let _ = engine.check_type(ty, &expected, span);
            }
        }
        _ => engine.push_error(TypeCheckError::bad_binary_operand(
            span, "logical", "bool", resolved,
        )),
    }
}

fn infer_bitwise(engine: &mut InferEngine<'_>, inputs: BinaryInputs<'_>) -> Idx {
    let left_span = inputs.arena.get_expr(inputs.left).span;
    let resolved_left = engine.resolve(inputs.left_ty);
    let left_tag = engine.pool().tag(resolved_left);
    match left_tag {
        Tag::Error => return Idx::ERROR,
        Tag::Never => return Idx::NEVER,
        _ => {}
    }

    if let Some(supported) = is_binary_op_supported(left_tag, inputs.op) {
        if !supported {
            push_unsupported_bitwise(engine, inputs, resolved_left);
            return Idx::ERROR;
        }
    } else if left_tag == Tag::Var {
    } else if !left_tag.is_primitive() && !left_tag.is_type_variable() {
        if let Some(result) = resolve_binary_op_via_trait(
            engine,
            inputs.arena,
            resolved_left,
            inputs.right_ty,
            inputs.right,
            inputs.op,
            inputs.span,
        ) {
            return result;
        }
        if let Some(trait_name) = binary_op_to_trait_name(inputs.op) {
            engine.push_error(TypeCheckError::unsupported_operator(
                inputs.span,
                resolved_left,
                inputs.op.as_symbol(),
                trait_name,
            ));
            return Idx::ERROR;
        }
        engine.push_error(TypeCheckError::bad_binary_operand(
            left_span,
            "bitwise",
            "int",
            resolved_left,
        ));
        return Idx::ERROR;
    }

    let resolved_right = engine.resolve(inputs.right_ty);
    match engine.pool().tag(resolved_right) {
        Tag::Error => return Idx::ERROR,
        Tag::Never => return Idx::NEVER,
        _ => {}
    }
    engine.push_context(ContextKind::BinaryOpRight {
        op: inputs.op.as_symbol(),
    });
    let expected = Expected {
        ty: Idx::INT,
        origin: ExpectedOrigin::Context {
            span: left_span,
            kind: ContextKind::BinaryOpLeft {
                op: inputs.op.as_symbol(),
            },
        },
    };
    let _ = engine.check_type(
        inputs.right_ty,
        &expected,
        inputs.arena.get_expr(inputs.right).span,
    );
    engine.pop_context();
    Idx::INT
}

fn push_unsupported_bitwise(
    engine: &mut InferEngine<'_>,
    inputs: BinaryInputs<'_>,
    resolved_left: Idx,
) {
    if let Some(trait_name) = binary_op_to_trait_name(inputs.op) {
        engine.push_error(TypeCheckError::unsupported_operator(
            inputs.span,
            resolved_left,
            inputs.op.as_symbol(),
            trait_name,
        ));
    } else {
        engine.push_error(TypeCheckError::bad_binary_operand(
            inputs.arena.get_expr(inputs.left).span,
            "bitwise",
            "int",
            resolved_left,
        ));
    }
}

fn infer_range(engine: &mut InferEngine<'_>, inputs: BinaryInputs<'_>) -> Idx {
    let left_span = inputs.arena.get_expr(inputs.left).span;
    let expected = Expected {
        ty: inputs.left_ty,
        origin: ExpectedOrigin::Context {
            span: left_span,
            kind: ContextKind::RangeStart,
        },
    };
    let _ = engine.check_type(
        inputs.right_ty,
        &expected,
        inputs.arena.get_expr(inputs.right).span,
    );
    let element = engine.resolve(inputs.left_ty);
    engine.pool_mut().range(element)
}

fn infer_coalesce(engine: &mut InferEngine<'_>, inputs: BinaryInputs<'_>) -> Idx {
    let resolved_left = engine.resolve(inputs.left_ty);
    let left_tag = engine.pool().tag(resolved_left);
    match left_tag {
        Tag::Option | Tag::Result => {
            let resolved_right = engine.resolve(inputs.right_ty);
            let right_tag = engine.pool().tag(resolved_right);
            if right_tag == left_tag && engine.unify_types(inputs.left_ty, inputs.right_ty).is_ok()
            {
                return engine.resolve(inputs.left_ty);
            }
            let inner = if left_tag == Tag::Option {
                engine.pool().option_inner(resolved_left)
            } else {
                engine.pool().result_ok(resolved_left)
            };
            if engine.unify_types(inner, inputs.right_ty).is_ok() {
                engine.resolve(inner)
            } else {
                let expected = engine.resolve(inner);
                let found = engine.resolve(inputs.right_ty);
                engine.push_error(TypeCheckError::mismatch(
                    inputs.span,
                    expected,
                    found,
                    vec![],
                    ErrorContext::default(),
                ));
                Idx::ERROR
            }
        }
        Tag::Var => engine.fresh_var(),
        Tag::Error => Idx::ERROR,
        Tag::Never => Idx::NEVER,
        _ => {
            engine.push_error(TypeCheckError::coalesce_requires_option(inputs.span));
            Idx::ERROR
        }
    }
}

fn check_right_against_left(
    engine: &mut InferEngine<'_>,
    inputs: BinaryInputs<'_>,
    right_context: ContextKind,
    left_context: ContextKind,
) {
    engine.push_context(right_context);
    let expected = Expected {
        ty: inputs.left_ty,
        origin: ExpectedOrigin::Context {
            span: inputs.arena.get_expr(inputs.left).span,
            kind: left_context,
        },
    };
    let _ = engine.check_type(
        inputs.right_ty,
        &expected,
        inputs.arena.get_expr(inputs.right).span,
    );
    engine.pop_context();
}
