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
        let elem_ty = infer_assign_target(engine, arena, target, root, steps);
        let value_ty = infer_expr(engine, arena, value);
        // The assigned value must be assignable to the value-position element
        // type of the chain (`xs[i] = v` requires `v: elem(xs)`). An `Idx::ERROR`
        // element (e.g. str index-assign, already diagnosed) absorbs silently.
        let expected = Expected {
            ty: elem_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(target).span,
                kind: ContextKind::Assignment,
            },
        };
        let _ = engine.check_type(value_ty, &expected, arena.get_expr(value).span);
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
/// `{ ...root, f: v }`) — the type-directed desugar of `EX-17`. Returns the
/// value-position element type (the last level type) so `infer_assign` can
/// check the assigned value against it; the assignment expression itself is
/// `Idx::UNIT`.
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
    // Root-binding validation: the chain root must be a mutable local.
    //   - `let $x = ...` (immutable): rejected (E2039).
    //   - parameter / non-mutable-tracked binding: rejected (E2051) — params are
    //     not mutable roots for index/field assignment.
    //   - `let x = ...` (mutable, `Some(true)`): allowed.
    //   - unknown name (`None`, not bound): left to `infer_expr(root)` to report.
    if let ExprKind::Ident(name) = arena.get_expr(root).kind {
        let span = arena.get_expr(root).span;
        match engine.env().is_mutable(name) {
            Some(false) => {
                engine.push_error(TypeCheckError::assign_to_immutable(span, name));
            }
            None if engine.env().lookup(name).is_some() => {
                engine.push_error(TypeCheckError::assign_through_parameter(span, name));
            }
            _ => {}
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

    // The final level type is the value-position element type (`updated`'s
    // `value` parameter / `{ ...s, f: v }`'s field type) the assigned value is
    // checked against by `infer_assign`.
    let elem_ty = level_types.last().copied().unwrap_or(Idx::UNIT);
    engine.record_assign_desugar(target, level_types);
    elem_ty
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
    span: Span,
) -> Idx {
    let resolved = engine.resolve(receiver_ty);
    match engine.pool().tag(resolved) {
        Tag::List => {
            let elem_ty = engine.pool().list_elem(resolved);
            check_index_key(engine, index_ty, Idx::INT, span);
            elem_ty
        }
        Tag::Map => {
            let key_ty = engine.pool().map_key(resolved);
            let value_ty = engine.pool().map_value(resolved);
            check_index_key(engine, index_ty, key_ty, span);
            value_ty
        }
        // `str` supports index reads (`s[i]` → single-codepoint `str`) but NOT
        // index assignment — it is immutable through indexing (no `IndexSet`).
        Tag::Str => {
            engine.push_error(TypeCheckError::index_assign_not_supported(span, resolved));
            Idx::ERROR
        }
        Tag::Var => engine.fresh_var(),
        _ => Idx::ERROR,
    }
}

/// Check an index expression's type against the receiver's key type, emitting a
/// type mismatch (E2001) on failure (`m[5] = v` where `m: {str: V}`).
fn check_index_key(engine: &mut InferEngine<'_>, index_ty: Idx, key_ty: Idx, span: Span) {
    let expected = Expected {
        ty: key_ty,
        origin: ExpectedOrigin::Context {
            span,
            kind: ContextKind::IndexKey,
        },
    };
    let _ = engine.check_type(index_ty, &expected, span);
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
