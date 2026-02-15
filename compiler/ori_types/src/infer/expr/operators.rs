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
                // List concatenation: [T] + [T] = [T]
                (Tag::List, Tag::List, BinaryOp::Add) => {
                    // Unify element types and return the left list type
                    let left_elem = engine.pool().list_elem(resolved_left);
                    let right_elem = engine.pool().list_elem(resolved_right);
                    let _ = engine.unify_types(left_elem, right_elem);
                    Some(engine.resolve(left_ty))
                }
                // Never propagation: right operand diverges
                (_, Tag::Never, _) => Some(Idx::NEVER),
                // Error propagation
                (_, Tag::Error, _) | (Tag::Error, _, _) => Some(Idx::ERROR),
                _ => None,
            };

            if let Some(result) = mixed_result {
                return result;
            }

            // Try trait dispatch for non-primitive, non-variable types
            if !left_tag.is_primitive() && !left_tag.is_type_variable() {
                if let Some(ret) = resolve_binary_op_via_trait(
                    engine,
                    arena,
                    resolved_left,
                    right_ty,
                    right,
                    op,
                    span,
                ) {
                    return ret;
                }
                // No trait impl found — emit error
                if let Some(trait_name) = binary_op_to_trait_name(op) {
                    engine.push_error(TypeCheckError::unsupported_operator(
                        span,
                        resolved_left,
                        op_str,
                        trait_name,
                    ));
                    return Idx::ERROR;
                }
            }

            // Default for primitives/type variables: unify left and right operands
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
                    // Try trait dispatch for user-defined types
                    if !left_tag.is_primitive() && !left_tag.is_type_variable() {
                        if let Some(ret) = resolve_binary_op_via_trait(
                            engine,
                            arena,
                            resolved_left,
                            right_ty,
                            right,
                            op,
                            span,
                        ) {
                            return ret;
                        }
                        if let Some(trait_name) = binary_op_to_trait_name(op) {
                            engine.push_error(TypeCheckError::unsupported_operator(
                                span,
                                resolved_left,
                                op_str,
                                trait_name,
                            ));
                            return Idx::ERROR;
                        }
                    }
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
                Tag::Error => Idx::ERROR,
                Tag::Var => {
                    let _ = engine.unify_types(operand_ty, Idx::INT);
                    engine.resolve(operand_ty)
                }
                _ => {
                    if !tag.is_primitive() && !tag.is_type_variable() {
                        if let Some(ret) = resolve_unary_op_via_trait(engine, resolved, op) {
                            return ret;
                        }
                        engine.push_error(TypeCheckError::unsupported_operator(
                            operand_span,
                            resolved,
                            "-",
                            "Neg",
                        ));
                        return Idx::ERROR;
                    }
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
                Tag::Error => Idx::ERROR,
                Tag::Var => {
                    let _ = engine.unify_types(operand_ty, Idx::BOOL);
                    Idx::BOOL
                }
                _ => {
                    if !tag.is_primitive() && !tag.is_type_variable() {
                        if let Some(ret) = resolve_unary_op_via_trait(engine, resolved, op) {
                            return ret;
                        }
                        engine.push_error(TypeCheckError::unsupported_operator(
                            operand_span,
                            resolved,
                            "!",
                            "Not",
                        ));
                        return Idx::ERROR;
                    }
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
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);
            match tag {
                Tag::Int | Tag::Var => {
                    engine.push_context(ContextKind::UnaryOpOperand { op: "~" });
                    let expected = Expected {
                        ty: Idx::INT,
                        origin: ExpectedOrigin::NoExpectation,
                    };
                    let _ = engine.check_type(operand_ty, &expected, operand_span);
                    engine.pop_context();
                    Idx::INT
                }
                Tag::Error => Idx::ERROR,
                Tag::Never => Idx::NEVER,
                _ => {
                    if !tag.is_primitive() && !tag.is_type_variable() {
                        if let Some(ret) = resolve_unary_op_via_trait(engine, resolved, op) {
                            return ret;
                        }
                        engine.push_error(TypeCheckError::unsupported_operator(
                            operand_span,
                            resolved,
                            "~",
                            "BitNot",
                        ));
                        return Idx::ERROR;
                    }
                    engine.push_context(ContextKind::UnaryOpOperand { op: "~" });
                    let expected = Expected {
                        ty: Idx::INT,
                        origin: ExpectedOrigin::NoExpectation,
                    };
                    let _ = engine.check_type(operand_ty, &expected, operand_span);
                    engine.pop_context();
                    Idx::INT
                }
            }
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

/// Map a binary operator to its trait method name.
///
/// Delegates to `BinaryOp::trait_method_name()` — the single source of truth in `ori_ir`.
fn binary_op_to_method_name(op: BinaryOp) -> Option<&'static str> {
    op.trait_method_name()
}

/// Map a binary operator to its trait name (for error messages).
fn binary_op_to_trait_name(op: BinaryOp) -> Option<&'static str> {
    match op {
        BinaryOp::Add => Some("Add"),
        BinaryOp::Sub => Some("Sub"),
        BinaryOp::Mul => Some("Mul"),
        BinaryOp::Div => Some("Div"),
        BinaryOp::FloorDiv => Some("FloorDiv"),
        BinaryOp::Mod => Some("Rem"),
        BinaryOp::BitAnd => Some("BitAnd"),
        BinaryOp::BitOr => Some("BitOr"),
        BinaryOp::BitXor => Some("BitXor"),
        BinaryOp::Shl => Some("Shl"),
        BinaryOp::Shr => Some("Shr"),
        _ => None,
    }
}

/// Try to resolve a binary operator via trait dispatch.
///
/// Looks up the operator's method name in the `TraitRegistry` for the left
/// operand's type. If found, checks the right operand against the method's
/// parameter type and returns the method's return type.
fn resolve_binary_op_via_trait(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver_ty: Idx,
    right_ty: Idx,
    right: ExprId,
    op: BinaryOp,
    span: Span,
) -> Option<Idx> {
    let method_name = binary_op_to_method_name(op)?;
    let op_str = op.as_symbol();
    let name = engine.intern_name(method_name)?;

    // Scoped borrow: extract signature and self-ness, then release the registry borrow.
    let (sig_ty, has_self) = {
        let trait_registry = engine.trait_registry()?;
        let lookup = trait_registry.lookup_method(receiver_ty, name)?;
        (lookup.method().signature, lookup.method().has_self)
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Some(Idx::ERROR);
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    // Skip `self` parameter for instance methods
    let skip = usize::from(has_self);
    let method_params = &params[skip..];

    // Binary operators expect exactly one non-self parameter
    if method_params.len() != 1 {
        return Some(Idx::ERROR);
    }

    // Check right operand against the method's parameter type
    let expected = Expected {
        ty: method_params[0],
        origin: ExpectedOrigin::Context {
            span,
            kind: ContextKind::BinaryOpRight { op: op_str },
        },
    };
    let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);

    Some(ret)
}

/// Try to resolve a unary operator via trait dispatch.
///
/// Looks up the operator's method name in the `TraitRegistry` for the
/// operand's type. If found, returns the method's return type.
///
/// Uses `UnaryOp::trait_method_name()` as the single source of truth for
/// the operator→method mapping.
fn resolve_unary_op_via_trait(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    op: UnaryOp,
) -> Option<Idx> {
    let method_name = op.trait_method_name()?;
    let name = engine.intern_name(method_name)?;

    let sig_ty = {
        let trait_registry = engine.trait_registry()?;
        let lookup = trait_registry.lookup_method(receiver_ty, name)?;
        lookup.method().signature
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Some(Idx::ERROR);
    }

    Some(engine.pool().function_return(resolved_sig))
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
