//! Assignment-expression inference.

use ori_ir::{AccessStep, AccessStepRange, ExprArena, ExprId, ExprKind, Span};

use super::super::super::InferEngine;
use super::super::infer_expr;
use super::super::structs::infer_struct_field;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError};

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
    // The type-directed desugar (`ori_canon`) consumes the per-level types
    // recorded here to synthesize the pure-reassignment form.
    if let ExprKind::AssignTarget { root, steps } = arena.get_expr(target).kind {
        let _ = infer_assign_target(engine, arena, target, root, steps);
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
/// Types the root and every index-step expression so no `Tag::Var` leaks from
/// them, walks the chain computing the resolved receiver-read type at every
/// level, and records the per-level types into the
/// [`InferEngine`]'s assign-desugar accumulator keyed by the `AssignTarget`
/// node's `ExprId` (`target`). `ori_canon` consumes the recorded plan to
/// synthesize the pure-reassignment form (`root = root.updated(...)` /
/// `{ ...root, f: v }`) — the type-directed desugar of `EX-17`. Result is
/// `Idx::UNIT`, matching the assignment-statement form.
///
/// Each `level_types[k]` is the resolved type of reading `root` plus the
/// first `k` steps, with map reads UNWRAPPED to the value type (the receiver
/// the next level steps into, and the element type `updated`'s `value`
/// parameter accepts). The final entry is the value-position element type.
pub(crate) fn infer_assign_target(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    target: ExprId,
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

    let root_ty = infer_expr(engine, arena, root);

    // Copy the steps out so the immutable arena borrow does not overlap the
    // mutable `engine` use in the per-step type computations below.
    let step_list: Vec<AccessStep> = arena.get_access_steps(steps).to_vec();

    let mut level_types = Vec::with_capacity(step_list.len() + 1);
    level_types.push(engine.resolve(root_ty));

    let mut receiver_ty = root_ty;
    for step in step_list {
        let next_ty = match step {
            AccessStep::Index(index) => {
                let index_ty = infer_expr(engine, arena, index);
                let span = arena.get_expr(index).span;
                step_index_read_type(engine, receiver_ty, index_ty, span)
            }
            AccessStep::Field(field) => {
                let span = arena.get_expr(root).span;
                step_field_read_type(engine, receiver_ty, field, span)
            }
        };
        let resolved = engine.resolve(next_ty);
        level_types.push(resolved);
        receiver_ty = resolved;
    }

    engine.record_assign_desugar(target, level_types);
    Idx::UNIT
}

/// Resolve the value-position element type for an index step in an
/// assignment-target chain.
///
/// `[T]` -> `T`, `{K: V}` -> `V` (UNWRAPPED — the read `Option<V>` is the
/// surface read type, but `updated`'s `value` parameter and any next-level
/// receiver step into `V`), `str` -> `str`. Unifies the index expression's
/// type against the receiver's key type. Unknown/error/var receivers yield a
/// fresh var (deferred) or `Idx::ERROR`.
fn step_index_read_type(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    index_ty: Idx,
    _span: Span,
) -> Idx {
    let resolved = engine.resolve(receiver_ty);
    match engine.pool().tag(resolved) {
        Tag::List => {
            let elem_ty = engine.pool().list_elem(resolved);
            let _ = engine.unify_types(index_ty, Idx::INT);
            elem_ty
        }
        Tag::Map => {
            let key_ty = engine.pool().map_key(resolved);
            let value_ty = engine.pool().map_value(resolved);
            let _ = engine.unify_types(index_ty, key_ty);
            value_ty
        }
        Tag::Str => {
            let _ = engine.unify_types(index_ty, Idx::INT);
            Idx::STR
        }
        Tag::Var => engine.fresh_var(),
        _ => Idx::ERROR,
    }
}

/// Resolve the type of a field step in an assignment-target chain.
///
/// Reuses struct/tuple field resolution. Unknown/error/var receivers yield a
/// fresh var (deferred) or `Idx::ERROR`.
fn step_field_read_type(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    field: ori_ir::Name,
    span: Span,
) -> Idx {
    let resolved = engine.resolve(receiver_ty);
    match engine.pool().tag(resolved) {
        Tag::Tuple => {
            let Some(field_str) = engine.lookup_name(field) else {
                return Idx::ERROR;
            };
            if let Ok(index) = field_str.parse::<usize>() {
                let elems = engine.pool().tuple_elems(resolved);
                elems.get(index).copied().unwrap_or(Idx::ERROR)
            } else {
                Idx::ERROR
            }
        }
        Tag::Named => {
            let type_name = engine.pool().named_name(resolved);
            infer_struct_field(engine, type_name, None, field, span)
        }
        Tag::Applied => {
            let type_name = engine.pool().applied_name(resolved);
            let type_args = engine.pool().applied_args(resolved);
            infer_struct_field(engine, type_name, Some(type_args), field, span)
        }
        Tag::Var => engine.fresh_var(),
        _ => Idx::ERROR,
    }
}
