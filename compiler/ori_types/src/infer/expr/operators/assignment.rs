//! Assignment-expression inference.

use ori_ir::{AccessStep, AccessStepRange, ExprArena, ExprId, ExprKind, Span};

use super::super::super::InferEngine;
use super::super::infer_expr;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, TypeCheckError};

/// Infer the type of an assignment expression.
pub(crate) fn infer_assign(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    target: ExprId,
    value: ExprId,
    span: Span,
) -> Idx {
    // An `AssignTarget` chain (`x[i] = v` / `x.f = v`) types its own root and
    // steps; the value type does not unify against the chain's UNIT result.
    // Mutability of the chain root is checked inside `infer_assign_target`.
    if let ExprKind::AssignTarget { root, steps } = arena.get_expr(target).kind {
        let _ = infer_assign_target(engine, arena, root, steps);
        let _value_ty = infer_expr(engine, arena, value);
        return Idx::UNIT;
    }

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

/// Infer the type of an assignment-target chain (`root` plus access steps).
///
/// The type-directed desugar (a later phase) eliminates `AssignTarget`; until
/// then this mirrors the index/field-assignment path by typing the root and
/// every index-step expression so no `Tag::Var` leaks from them. Result is
/// `Idx::UNIT`, matching the assignment-statement form (`EX-17`).
pub(crate) fn infer_assign_target(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    root: ExprId,
    steps: AccessStepRange,
) -> Idx {
    // Assigning through an immutable root binding (`let $x = ...; x[i] = v`) is rejected.
    if let ExprKind::Ident(name) = arena.get_expr(root).kind {
        if engine.env().is_mutable(name) == Some(false) {
            let span = arena.get_expr(root).span;
            engine.push_error(TypeCheckError::assign_to_immutable(span, name));
        }
    }

    let _root_ty = infer_expr(engine, arena, root);
    for step in arena.get_access_steps(steps) {
        if let AccessStep::Index(index) = step {
            let _ = infer_expr(engine, arena, *index);
        }
    }
    Idx::UNIT
}
