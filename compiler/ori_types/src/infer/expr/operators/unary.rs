//! Unary-operator inference.

use ori_ir::{ExprArena, ExprId, Span, UnaryOp};

use super::super::super::InferEngine;
use super::super::infer_expr;
use super::super::registry_bridge::is_unary_op_supported;
use crate::{Idx, Tag, TypeCheckError};

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
