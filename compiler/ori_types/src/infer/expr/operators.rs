//! Operator inference — binary, unary, cast, and assignment operators.

use ori_ir::{BinaryOp, ExprArena, ExprId, ExprKind, Span, UnaryOp};

use super::super::InferEngine;
use super::registry_bridge::{is_binary_op_supported, is_unary_op_supported};
use super::{infer_expr, resolve_and_check_parsed_type};
use crate::{ContextKind, ErrorContext, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError};

/// Infer the type of a binary operation.
#[expect(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "exhaustive BinaryOp dispatch with registry queries and trait-based type checking"
)]
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
        | BinaryOp::FloorDiv
        | BinaryOp::MatMul => {
            let resolved_left = engine.resolve(left_ty);
            let resolved_right = engine.resolve(right_ty);
            let left_tag = engine.pool().tag(resolved_left);
            let right_tag = engine.pool().tag(resolved_right);

            // Never/Error propagation (check before registry queries)
            match (left_tag, right_tag) {
                (Tag::Error, _) | (_, Tag::Error) => return Idx::ERROR,
                (_, Tag::Never) => return Idx::NEVER,
                _ => {}
            }

            // Cross-type special cases: Duration/Size mixed with Int.
            // These are cross-type rules the per-type registry can't express.
            if let Some(result) = check_cross_type_arithmetic(left_tag, right_tag, op) {
                return result;
            }

            // Same-type builtin operators: query the registry to validate.
            // For same-type pairs where both sides are the same builtin type
            // (Duration+Duration, Str+Str, List+List, etc.), the registry
            // determines whether the operator is supported.
            if left_tag == right_tag {
                if let Some(supported) = is_binary_op_supported(left_tag, op) {
                    if !supported {
                        if let Some(trait_name) = binary_op_to_trait_name(op) {
                            engine.push_error(TypeCheckError::unsupported_operator(
                                span,
                                resolved_left,
                                op_str,
                                trait_name,
                            ));
                        } else {
                            engine.push_error(TypeCheckError::bad_binary_operand(
                                arena.get_expr(left).span,
                                "arithmetic",
                                "numeric",
                                resolved_left,
                            ));
                        }
                        return Idx::ERROR;
                    }
                    // Operator supported — handle list concatenation specially
                    // (needs element type unification), others unify and return.
                    if left_tag == Tag::List && op == BinaryOp::Add {
                        let left_elem = engine.pool().list_elem(resolved_left);
                        let right_elem = engine.pool().list_elem(resolved_right);
                        let _ = engine.unify_types(left_elem, right_elem);
                        return engine.resolve(left_ty);
                    }
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
                    return engine.resolve(left_ty);
                }
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

        // Comparison: same type in, bool out. Registry validates primitive support;
        // non-primitive types must implement Eq (for ==/!=) or Comparable (for </<=/>/>=).
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq => {
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);

            // Error/Never propagation
            if left_tag == Tag::Error {
                return Idx::ERROR;
            }
            let resolved_right_top = engine.resolve(right_ty);
            let right_tag = engine.pool().tag(resolved_right_top);
            if right_tag == Tag::Error {
                return Idx::ERROR;
            }
            if right_tag == Tag::Never {
                return Idx::NEVER;
            }

            // Registry-based validation for primitive types only.
            // Only primitives (int, float, bool, str, char, byte, Duration, Size,
            // Ordering, etc.) use the registry's OpDefs as the authority for
            // comparison support. Compound types (Option, Result, List, Tuple, etc.)
            // and user types use trait dispatch for comparison — the registry's
            // OpDefs is for codegen strategy, not type checking.
            if left_tag.is_primitive() {
                if let Some(false) = is_binary_op_supported(left_tag, op) {
                    let trait_name = comparison_trait_name(op);
                    engine.push_error(TypeCheckError::unsupported_operator(
                        span,
                        resolved_left,
                        op_str,
                        trait_name,
                    ));
                    return Idx::ERROR;
                }
            }

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

        // Boolean: bool in, bool out.
        // And/Or are not tracked in OpDefs (they're language-level boolean
        // operators, not overloadable). The bool restriction is inherent.
        BinaryOp::And | BinaryOp::Or => {
            let left_span = arena.get_expr(left).span;
            let right_span = arena.get_expr(right).span;

            // Check left is bool
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);
            match left_tag {
                Tag::Bool | Tag::Error | Tag::Var | Tag::Never => {
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

        // Bitwise operations: query registry for support
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            let left_span = arena.get_expr(left).span;

            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);

            // Error/Never propagation
            if left_tag == Tag::Error {
                return Idx::ERROR;
            }
            if left_tag == Tag::Never {
                return Idx::NEVER;
            }

            // Registry-based validation for builtin types.
            if let Some(supported) = is_binary_op_supported(left_tag, op) {
                if !supported {
                    if let Some(trait_name) = binary_op_to_trait_name(op) {
                        engine.push_error(TypeCheckError::unsupported_operator(
                            span,
                            resolved_left,
                            op_str,
                            trait_name,
                        ));
                    } else {
                        engine.push_error(TypeCheckError::bad_binary_operand(
                            left_span,
                            "bitwise",
                            "int",
                            resolved_left,
                        ));
                    }
                    return Idx::ERROR;
                }
                // Supported — fall through to unification below.
            } else if left_tag == Tag::Var {
                // Unresolved variable — fall through to unification.
            } else if !left_tag.is_primitive() && !left_tag.is_type_variable() {
                // Trait dispatch for user-defined types.
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
                engine.push_error(TypeCheckError::bad_binary_operand(
                    left_span,
                    "bitwise",
                    "int",
                    resolved_left,
                ));
                return Idx::ERROR;
            }

            // Check right operand (Error/Never propagation)
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

        // Coalesce: Option<T> ?? T -> T, Option<T> ?? Option<T> -> Option<T>,
        //           Result<T,E> ?? T -> T, Result<T,E> ?? Result<T,E> -> Result<T,E>
        // Spec: operator-rules.md §Coalesce (COALESCE-UNWRAP / COALESCE-CHAIN /
        //        COALESCE-RESULT-UNWRAP / COALESCE-RESULT-CHAIN)
        BinaryOp::Coalesce => {
            let resolved_left = engine.resolve(left_ty);
            let left_tag = engine.pool().tag(resolved_left);
            match left_tag {
                Tag::Option | Tag::Result => {
                    let resolved_right = engine.resolve(right_ty);
                    let right_tag = engine.pool().tag(resolved_right);

                    // CHAIN vs UNWRAP detection:
                    // When both sides have the same wrapper tag, try to unify
                    // them. This handles polymorphic RHS (bare `None`, `Err("x")`)
                    // where the type variable hasn't resolved yet.
                    //
                    // Spec: operator-rules.md §Coalesce
                    //   CHAIN:  Option<T> ?? Option<T> -> Option<T>
                    //   UNWRAP: Option<T> ?? T -> T
                    //
                    // Edge case: `Option<Option<int>> ?? Option<int>` — both are
                    // Option, but unifying them fails (inner mismatch), so we
                    // correctly fall through to UNWRAP.
                    if right_tag == left_tag && engine.unify_types(left_ty, right_ty).is_ok() {
                        // Unification succeeded: CHAIN. Re-resolve for canonical Idx.
                        engine.resolve(left_ty)
                    } else {
                        // UNWRAP: extract inner type and unify with RHS.
                        let inner = if left_tag == Tag::Option {
                            engine.pool().option_inner(resolved_left)
                        } else {
                            engine.pool().result_ok(resolved_left)
                        };
                        if engine.unify_types(inner, right_ty).is_err() {
                            // RHS doesn't match inner type — report mismatch.
                            // Spec: only T or the same wrapper type are valid RHS.
                            let expected = engine.resolve(inner);
                            let found = engine.resolve(right_ty);
                            engine.push_error(TypeCheckError::mismatch(
                                span,
                                expected,
                                found,
                                vec![],
                                ErrorContext::default(),
                            ));
                            Idx::ERROR
                        } else {
                            engine.resolve(inner)
                        }
                    }
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
        // Negation, logical NOT, bitwise NOT: query registry for primitive support.
        UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot => {
            let resolved = engine.resolve(operand_ty);
            let tag = engine.pool().tag(resolved);

            // Error/Never propagation
            if tag == Tag::Error {
                return Idx::ERROR;
            }
            if tag == Tag::Never {
                return Idx::NEVER;
            }

            // Registry-based validation for builtin types.
            if let Some(supported) = is_unary_op_supported(tag, op) {
                if supported {
                    return resolved;
                }
                // Registry says unsupported — emit error.
                if let Some(trait_name) = op.trait_name() {
                    engine.push_error(TypeCheckError::unsupported_operator(
                        operand_span,
                        resolved,
                        op.as_symbol(),
                        trait_name,
                    ));
                } else {
                    engine.push_error(TypeCheckError::bad_unary_operand(
                        operand_span,
                        op.as_symbol(),
                        resolved,
                    ));
                }
                return Idx::ERROR;
            }

            // Unresolved type variables: infer a default type.
            if tag == Tag::Var {
                let default_ty = match op {
                    UnaryOp::Not => Idx::BOOL,
                    _ => Idx::INT, // Neg, BitNot default to int
                };
                let _ = engine.unify_types(operand_ty, default_ty);
                return if op == UnaryOp::Not {
                    Idx::BOOL
                } else {
                    engine.resolve(operand_ty)
                };
            }

            // Trait dispatch for non-primitive, non-variable types.
            if !tag.is_primitive() && !tag.is_type_variable() {
                if let Some(ret) = resolve_unary_op_via_trait(engine, resolved, op) {
                    return ret;
                }
                if let Some(trait_name) = op.trait_name() {
                    engine.push_error(TypeCheckError::unsupported_operator(
                        operand_span,
                        resolved,
                        op.as_symbol(),
                        trait_name,
                    ));
                    return Idx::ERROR;
                }
            }

            // Fallthrough: unsupported
            engine.push_error(TypeCheckError::bad_unary_operand(
                operand_span,
                op.as_symbol(),
                resolved,
            ));
            Idx::ERROR
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
    span: Span,
) -> Idx {
    // Infer the expression type (for validation, though we don't check cast validity here)
    let _expr_ty = infer_expr(engine, arena, expr);

    // Resolve the target type
    let target_ty = resolve_and_check_parsed_type(engine, arena, ty, span);

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
///
/// Delegates to `BinaryOp::trait_name()` — the single source of truth in `ori_ir`.
fn binary_op_to_trait_name(op: BinaryOp) -> Option<&'static str> {
    op.trait_name()
}

/// Map a comparison operator to the trait name for error messages.
fn comparison_trait_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Eq | BinaryOp::NotEq => "Eq",
        BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => "Comparable",
        _ => unreachable!("comparison_trait_name called with non-comparison op"),
    }
}

/// Check for cross-type arithmetic special cases.
///
/// Handles mixed-type arithmetic that the per-type registry can't express:
/// - `Duration * int`, `int * Duration`, `Duration / int`, `Duration div int`
/// - `Size * int`, `int * Size`, `Size / int`, `Size div int`
///
/// These are cross-type rules where the result type differs from at least one
/// operand type. The registry validates whether each individual type supports
/// the operator; this function handles the cross-type pairing.
///
/// Returns `Some(result_idx)` if a cross-type rule matched, `None` otherwise.
fn check_cross_type_arithmetic(left_tag: Tag, right_tag: Tag, op: BinaryOp) -> Option<Idx> {
    // Validate that the operator is supported for the non-int side via the
    // registry. This prevents accepting e.g. Duration @ int (MatMul).
    let (unit_tag, unit_idx) = match (left_tag, right_tag) {
        (Tag::Duration, Tag::Int) | (Tag::Int, Tag::Duration) => (Tag::Duration, Idx::DURATION),
        (Tag::Size, Tag::Int) | (Tag::Int, Tag::Size) => (Tag::Size, Idx::SIZE),
        _ => return None,
    };

    // Only specific operators are valid for cross-type arithmetic.
    // Duration/Size: mul (both directions), div and floor_div (unit / int only).
    let unit_is_left = left_tag == unit_tag;
    match op {
        BinaryOp::Mul => Some(unit_idx),
        BinaryOp::Div | BinaryOp::FloorDiv if unit_is_left => {
            // Duration/Size supports div: check registry.
            is_binary_op_supported(unit_tag, op)
                .filter(|&supported| supported)
                .map(|_| unit_idx)
        }
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
    span: Span,
) -> Idx {
    // Check if target is an immutable binding (let $x = ...)
    if let ExprKind::Ident(name) = arena.get_expr(target).kind {
        if engine.env().is_mutable(name) == Some(false) {
            engine.push_error(TypeCheckError::assign_to_immutable(span, name));
        }
    }

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
