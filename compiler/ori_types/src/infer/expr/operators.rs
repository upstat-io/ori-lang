//! Operator inference — binary, unary, cast, and assignment operators.

use ori_ir::{BinaryOp, ExprArena, ExprId, Span, UnaryOp};

use super::super::InferEngine;
use super::{infer_expr, resolve_parsed_type};
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError};

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
    let op_str = op.as_symbol();

    // Never propagation: if the left operand is Never (e.g. panic()), the right
    // operand is unreachable and the whole expression is Never.
    let resolved_left_top = engine.resolve(left_ty);
    if engine.pool().tag(resolved_left_top) == Tag::Never {
        return Idx::NEVER;
    }

    match op {
        // Arithmetic: same type in, same type out (with Duration/Size mixed support)
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::FloorDiv => {
            let resolved_left = engine.resolve(left_ty);
            let resolved_right = engine.resolve(right_ty);
            let left_tag = engine.pool().tag(resolved_left);
            let right_tag = engine.pool().tag(resolved_right);

            // Special case: Duration/Size * Int, Int * Duration/Size, Duration/Size / Int
            let mixed_result = match (left_tag, right_tag, op) {
                // Duration + Duration, Duration * int, Duration / int, int * Duration = Duration
                (Tag::Duration, Tag::Duration, _)
                | (Tag::Duration, Tag::Int, BinaryOp::Mul | BinaryOp::Div | BinaryOp::FloorDiv)
                | (Tag::Int, Tag::Duration, BinaryOp::Mul) => Some(Idx::DURATION),
                // Size + Size, Size * int, Size / int, int * Size = Size
                (Tag::Size, Tag::Size, _)
                | (Tag::Size, Tag::Int, BinaryOp::Mul | BinaryOp::Div | BinaryOp::FloorDiv)
                | (Tag::Int, Tag::Size, BinaryOp::Mul) => Some(Idx::SIZE),
                // String concatenation
                (Tag::Str, Tag::Str, BinaryOp::Add) => Some(Idx::STR),
                // Never propagation: right operand diverges
                (_, Tag::Never, _) => Some(Idx::NEVER),
                // Error propagation
                (_, Tag::Error, _) | (Tag::Error, _, _) => Some(Idx::ERROR),
                _ => None,
            };

            if let Some(result) = mixed_result {
                return result;
            }

            // Default: unify left and right operands
            engine.push_context(ContextKind::BinaryOpRight { op: op_str });
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::BinaryOpLeft { op: op_str },
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            // Result type is the left operand type (after unification)
            engine.resolve(left_ty)
        }

        // Comparison: same type in, bool out
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq => {
            // Unify left and right operands
            engine.push_context(ContextKind::ComparisonRight);
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::ComparisonLeft,
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            Idx::BOOL
        }

        // Boolean: bool in, bool out
        BinaryOp::And | BinaryOp::Or => {
            let left_span = arena.get_expr(left).span;
            let right_span = arena.get_expr(right).span;

            // Check left is bool — produce operator-specific message on failure
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);
            match left_tag {
                Tag::Bool | Tag::Error | Tag::Var | Tag::Never => {
                    // Bool is correct, Error/Never propagate silently, Var defers
                    if left_tag != Tag::Never {
                        let bool_expected = Expected {
                            ty: Idx::BOOL,
                            origin: ExpectedOrigin::NoExpectation,
                        };
                        let _ = engine.check_type(left_ty, &bool_expected, left_span);
                    }
                }
                _ => {
                    engine.push_error(TypeCheckError::bad_binary_operand(
                        left_span,
                        "logical",
                        "bool",
                        resolved_left,
                    ));
                }
            }

            // Check right is bool (Never accepted: e.g. `false && panic()`)
            let resolved_right = engine.resolve(right_ty);
            let right_tag = engine.pool().tag(resolved_right);
            match right_tag {
                Tag::Bool | Tag::Error | Tag::Var | Tag::Never => {
                    if right_tag != Tag::Never {
                        let bool_expected = Expected {
                            ty: Idx::BOOL,
                            origin: ExpectedOrigin::NoExpectation,
                        };
                        let _ = engine.check_type(right_ty, &bool_expected, right_span);
                    }
                }
                _ => {
                    engine.push_error(TypeCheckError::bad_binary_operand(
                        right_span,
                        "logical",
                        "bool",
                        resolved_right,
                    ));
                }
            }

            Idx::BOOL
        }

        // Bitwise operations: int operands only
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            let left_span = arena.get_expr(left).span;

            // Check left operand is int (skip Error/Never to prevent cascading)
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);
            match left_tag {
                Tag::Int | Tag::Var => {}
                Tag::Error => return Idx::ERROR,
                Tag::Never => return Idx::NEVER,
                _ => {
                    engine.push_error(TypeCheckError::bad_binary_operand(
                        left_span,
                        "bitwise",
                        "int",
                        resolved_left,
                    ));
                    return Idx::ERROR;
                }
            }

            // Check right operand (also skip Error/Never)
            let resolved_right = engine.resolve(right_ty);
            match engine.pool().tag(resolved_right) {
                Tag::Error => return Idx::ERROR,
                Tag::Never => return Idx::NEVER,
                _ => {}
            }

            // Unify left and right as int
            engine.push_context(ContextKind::BinaryOpRight { op: op_str });
            let expected = Expected {
                ty: Idx::INT,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::BinaryOpLeft { op: op_str },
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);
            engine.pop_context();

            Idx::INT
        }

        // Range creation
        BinaryOp::Range | BinaryOp::RangeInclusive => {
            // Both operands should be the same type (typically int)
            let left_span = arena.get_expr(left).span;
            let expected = Expected {
                ty: left_ty,
                origin: ExpectedOrigin::Context {
                    span: left_span,
                    kind: ContextKind::RangeStart,
                },
            };
            let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);

            // Return Range<T>
            let elem_ty = engine.resolve(left_ty);
            engine.pool_mut().range(elem_ty)
        }

        // Coalesce: Option<T> ?? T -> T  or  Result<T, E> ?? T -> T
        BinaryOp::Coalesce => {
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);
            match left_tag {
                Tag::Option => {
                    let inner = engine.pool().option_inner(resolved_left);
                    let _ = engine.unify_types(inner, right_ty);
                    engine.resolve(inner)
                }
                Tag::Result => {
                    let ok_ty = engine.pool().result_ok(resolved_left);
                    let _ = engine.unify_types(ok_ty, right_ty);
                    engine.resolve(ok_ty)
                }
                // Unresolved variable — defer via fresh var
                Tag::Var => engine.fresh_var(),
                Tag::Error => Idx::ERROR,
                // Never is the bottom type — expression diverges before coalesce
                Tag::Never => Idx::NEVER,
                _ => {
                    engine.push_error(TypeCheckError::coalesce_requires_option(span));
                    Idx::ERROR
                }
            }
        }
    }
}

/// Infer the type of a unary operation.
pub(crate) fn infer_unary(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    op: UnaryOp,
    operand: ExprId,
    span: Span,
) -> Idx {
    let operand_ty = infer_expr(engine, arena, operand);
    let operand_span = arena.get_expr(operand).span;

    match op {
        // Negation: numeric/duration/size -> same type
        UnaryOp::Neg => {
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);
            match tag {
                Tag::Int | Tag::Float | Tag::Duration => resolved,
                // Propagate errors and defer type variables
                Tag::Error => Idx::ERROR,
                Tag::Var => {
                    // Type variable not yet resolved — unify with int as default
                    let _ = engine.unify_types(operand_ty, Idx::INT);
                    engine.resolve(operand_ty)
                }
                _ => {
                    engine.push_error(TypeCheckError::bad_unary_operand(
                        operand_span,
                        "-",
                        resolved,
                    ));
                    Idx::ERROR
                }
            }
        }

        // Logical not: bool -> bool
        UnaryOp::Not => {
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);
            match tag {
                Tag::Bool => Idx::BOOL,
                // Propagate errors and defer type variables
                Tag::Error => Idx::ERROR,
                Tag::Var => {
                    let _ = engine.unify_types(operand_ty, Idx::BOOL);
                    Idx::BOOL
                }
                _ => {
                    engine.push_error(TypeCheckError::bad_unary_operand(
                        operand_span,
                        "!",
                        resolved,
                    ));
                    Idx::ERROR
                }
            }
        }

        // Bitwise not: int -> int
        UnaryOp::BitNot => {
            engine.push_context(ContextKind::UnaryOpOperand { op: "~" });
            let expected = Expected {
                ty: Idx::INT,
                origin: ExpectedOrigin::NoExpectation,
            };
            let _ = engine.check_type(operand_ty, &expected, operand_span);
            engine.pop_context();
            Idx::INT
        }

        // Try operator: Option<T> -> T or Result<T, E> -> T
        UnaryOp::Try => {
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);

            match tag {
                Tag::Option => engine.pool().option_inner(resolved),
                Tag::Result => engine.pool().result_ok(resolved),
                Tag::Error => Idx::ERROR,
                _ => {
                    engine.push_error(TypeCheckError::try_requires_option_or_result(
                        span, resolved,
                    ));
                    Idx::ERROR
                }
            }
        }
    }
}

/// Infer the type of a cast expression.
pub(crate) fn infer_cast(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
    ty: &ori_ir::ParsedType,
    fallible: bool,
    _span: Span,
) -> Idx {
    // Infer the expression type (for validation, though we don't check cast validity here)
    let _expr_ty = infer_expr(engine, arena, expr);

    // Resolve the target type
    let target_ty = resolve_parsed_type(engine, arena, ty);

    // Fallible casts return Option<T>, infallible return T directly
    if fallible {
        engine.pool_mut().option(target_ty)
    } else {
        target_ty
    }
}

/// Infer the type of an assignment expression.
pub(crate) fn infer_assign(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    target: ExprId,
    value: ExprId,
    _span: Span,
) -> Idx {
    let target_ty = infer_expr(engine, arena, target);
    let value_ty = infer_expr(engine, arena, value);

    let expected = Expected {
        ty: target_ty,
        origin: ExpectedOrigin::Context {
            span: arena.get_expr(target).span,
            kind: ContextKind::Assignment,
        },
    };
    let _ = engine.check_type(value_ty, &expected, arena.get_expr(value).span);

    Idx::UNIT
}
