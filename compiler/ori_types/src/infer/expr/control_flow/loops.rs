//! Loop inference — for, loop, while, break, continue.

use ori_ir::{BindingPatternId, ExprArena, ExprId, Name, Span};

use super::super::super::{InferEngine, LoopContext, LoopForm};
use super::super::{bind_pattern, infer_expr, pattern_is_irrefutable};
use crate::type_error::{NonCollectingLoopKind, VoidLoopKind};
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError};

/// Extract the element type bound by a `for` loop from the iterator type.
///
/// Known iterables (list, range, iterator, map, set, option, str) yield their
/// element type: a map yields `(key, value)` tuples, a str yields `char`. An
/// unknown iterator tag falls back to a fresh type variable so downstream
/// unification can still catch concrete mismatches. `Range<float>` is rejected
/// as non-iterable (spec 8.13).
fn for_loop_elem_ty(engine: &mut InferEngine<'_>, iter_ty: Idx, span: Span) -> Idx {
    let resolved_iter = engine.resolve(iter_ty);
    match engine.pool().tag(resolved_iter) {
        Tag::List => engine.pool().list_elem(resolved_iter),
        Tag::Range => {
            let elem = engine.pool().range_elem(resolved_iter);
            if elem == Idx::FLOAT {
                engine.push_error(TypeCheckError::range_float_not_iterable(
                    span,
                    "for i in 0..10 do i.to_float() / 10.0",
                ));
            }
            elem
        }
        Tag::Iterator | Tag::DoubleEndedIterator => engine.pool().iterator_elem(resolved_iter),
        Tag::Map => {
            // Iterating over a map yields (key, value) tuples
            let key_ty = engine.pool().map_key(resolved_iter);
            let value_ty = engine.pool().map_value(resolved_iter);
            engine.pool_mut().tuple(&[key_ty, value_ty])
        }
        // Sets store elements similarly to lists (single type parameter)
        Tag::Set => engine.pool().set_elem(resolved_iter),
        // Option<T> iterates as 0-or-1 element of type T
        Tag::Option => engine.pool().option_inner(resolved_iter),
        Tag::Str => Idx::CHAR,
        // Not a known iterable - still allow iteration with a fresh element type;
        // the type checker catches concrete type mismatches later.
        _ => engine.fresh_var(),
    }
}

/// Infer the type of a for loop.
///
/// For loops in Ori can be used in two forms:
/// - `for x in iter do body` - returns unit, iterates for side effects
/// - `for x in iter yield body` - returns a list, collects body results
///
/// The iterator must be iterable (list, range, etc.), and the binding
/// receives each element type.
// TODO(typeck): Refactor with a ForLoopParams struct when implementing
#[expect(clippy::too_many_arguments, reason = "matches ExprKind::For structure")]
pub(crate) fn infer_for(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    label: Name,
    pattern: BindingPatternId,
    iter: ExprId,
    guard: ExprId,
    body: ExprId,
    is_yield: bool,
    span: Span,
) -> Idx {
    // Enter scope for loop binding
    engine.enter_scope();

    // Infer iterator type
    engine.push_context(ContextKind::ForIterator);
    let iter_ty = infer_expr(engine, arena, iter);
    engine.pop_context();

    // Extract element type from iterator
    let elem_ty = for_loop_elem_ty(engine, iter_ty, span);

    // Bind the loop pattern (supports name, tuple, wildcard, struct, list)
    engine.push_context(ContextKind::ForBinding);
    let pat = arena.get_binding_pattern(pattern);
    if let Err(reason) = pattern_is_irrefutable(engine, pat, elem_ty) {
        let err = TypeCheckError::refutable_pattern(span, reason);
        engine.push_error(err);
    }
    bind_pattern(engine, arena, pat, elem_ty);
    engine.pop_context();

    // Check guard if present (must be bool)
    if guard.is_present() {
        let guard_ty = infer_expr(engine, arena, guard);
        let expected = Expected {
            ty: Idx::BOOL,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(guard).span,
                kind: ContextKind::LoopCondition,
            },
        };
        let _ = engine.check_type(guard_ty, &expected, arena.get_expr(guard).span);
    }

    // Push a loop context so `break` / `continue` inside the body resolve to
    // this `for`. Value rules differ by form (Spec: Clause 16):
    // - `for...yield` permits `break value` (adds a final element) and
    //   `continue value` (substitutes the element).
    // - `for...do` is void-typed: `break value` is E0860, `continue value`
    //   is E0861.
    let break_ty = engine.fresh_var();
    engine.push_loop_context(LoopContext {
        label,
        break_ty,
        break_value_allowed: is_yield,
        continue_value_allowed: is_yield,
        form: if is_yield {
            LoopForm::ForYield
        } else {
            LoopForm::ForDo
        },
    });

    // Infer body type
    engine.push_context(ContextKind::LoopBody);
    let body_ty = infer_expr(engine, arena, body);
    engine.pop_context();

    engine.pop_loop_context();

    // Exit loop scope
    engine.exit_scope();

    // Return type depends on do vs yield
    if is_yield {
        // yield: collect results into a list
        let resolved_body = engine.resolve(body_ty);
        engine.pool_mut().list(resolved_body)
    } else {
        // do: iterate for side effects, return unit
        Idx::UNIT
    }
}

/// Infer the type of an infinite loop.
///
/// `loop { body }` runs the body repeatedly until a `break` is encountered.
/// The loop type is determined by break expressions within the body:
/// - If breaks have values, the loop returns that type
/// - If no breaks, the loop returns `never` (runs forever)
pub(crate) fn infer_loop(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    label: Name,
    body: ExprId,
    _span: Span,
) -> Idx {
    // Create a fresh type variable for the loop's result (determined by break values)
    let break_ty = engine.fresh_var();
    // `loop` permits `break value` (the loop evaluates to it) but forbids
    // `continue value` (loops do not accumulate values — spec E0861).
    engine.push_loop_context(LoopContext {
        label,
        break_ty,
        break_value_allowed: true,
        continue_value_allowed: false,
        form: LoopForm::Loop,
    });

    // Enter scope for loop
    engine.enter_scope();

    // Infer body type (break expressions unify their value with break_ty)
    engine.push_context(ContextKind::LoopBody);
    let _body_ty = infer_expr(engine, arena, body);
    engine.pop_context();

    // Exit loop scope
    engine.exit_scope();
    engine.pop_loop_context();

    // Resolve the break type — if no break was encountered, the variable
    // stays unresolved (infinite loop returns Never). If breaks exist,
    // it unifies to their value type.
    let resolved = engine.resolve(break_ty);
    if engine.pool().tag(resolved) == Tag::Var {
        // No break was encountered — this is an infinite loop (returns Never).
        // Note: `break` without a value unifies break_ty with Unit, so
        // Tag::Var here means truly no break exists in the loop body.
        Idx::NEVER
    } else {
        resolved
    }
}

/// Infer the type of a while loop.
///
/// `while cond do body` runs the body repeatedly while `cond` holds.
/// The condition must be `bool`; the body is checked under loop context
/// (so `break` / `continue` are valid). A while loop always returns `void`
/// (Spec: Clause 16 — `break value` is forbidden inside `while`).
pub(crate) fn infer_while(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    label: Name,
    cond: ExprId,
    body: ExprId,
    _span: Span,
) -> Idx {
    // Push the loop context BEFORE inferring the condition: the spec desugar
    // `loop { if !cond then break; body }` (Spec: Clause 16.2.4) places the
    // condition INSIDE the while's own loop, so condition-position `break` /
    // `continue` target this while, not the enclosing loop. `while...do` is
    // void-typed: `break value` is E0860 and `continue value` is E0861.
    let break_ty = engine.fresh_var();
    engine.push_loop_context(LoopContext {
        label,
        break_ty,
        break_value_allowed: false,
        continue_value_allowed: false,
        form: LoopForm::While,
    });

    // Condition must be bool (inferred under the while's own loop context).
    let cond_ty = infer_expr(engine, arena, cond);
    let expected = Expected {
        ty: Idx::BOOL,
        origin: ExpectedOrigin::Context {
            span: arena.get_expr(cond).span,
            kind: ContextKind::LoopCondition,
        },
    };
    let _ = engine.check_type(cond_ty, &expected, arena.get_expr(cond).span);

    engine.enter_scope();
    engine.push_context(ContextKind::LoopBody);
    let _body_ty = infer_expr(engine, arena, body);
    engine.pop_context();
    engine.exit_scope();

    engine.pop_loop_context();

    Idx::UNIT
}

/// Infer the type of a break expression.
///
/// A labeled `break:name` resolves its value-permission and break-type
/// unification against the LABELED target loop (per Spec: Clause 16),
/// not the innermost loop. An unlabeled `break` targets the innermost loop.
pub(crate) fn infer_break(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    label: Name,
    value: ExprId,
    _span: Span,
) -> Idx {
    let target = engine.resolve_loop_context(label);

    if value.is_present() {
        // Infer the value first so any errors inside it still surface.
        let value_ty = infer_expr(engine, arena, value);
        let value_span = arena.get_expr(value).span;

        match target {
            Some(ctx) if ctx.break_value_allowed => {
                let _ = engine.unify_types(value_ty, ctx.break_ty);
            }
            Some(ctx) => {
                // `break value` targeting a void-typed loop (`while...do` /
                // `for...do`). The form is the TARGET loop's form, so a
                // labeled break naming an outer void loop reports that loop.
                let kind = match ctx.form {
                    LoopForm::ForDo => VoidLoopKind::ForDo,
                    // While is the only other void form that pushes a context;
                    // Loop / ForYield carry break_value_allowed = true.
                    _ => VoidLoopKind::While,
                };
                engine.push_error(TypeCheckError::break_value_in_void_loop(value_span, kind));
            }
            None => {
                // `break value` with no resolvable target: either outside any
                // loop, or a labeled break whose label names no enclosing loop
                // (the label-resolution error is owned elsewhere). A value here
                // has no break target — surface a void-loop-style rejection.
                engine.push_error(TypeCheckError::break_value_in_void_loop(
                    value_span,
                    VoidLoopKind::While,
                ));
            }
        }
    } else if let Some(ctx) = target {
        // Bare `break` unifies the target loop's break type with unit when the
        // loop permits a value (so a later `break value` and this `break`
        // agree), matching the prior behavior for `loop`.
        if ctx.break_value_allowed {
            let _ = engine.unify_types(Idx::UNIT, ctx.break_ty);
        }
    }

    // Break itself is a diverging expression (control transfers to loop exit)
    Idx::NEVER
}

/// Infer the type of a continue expression.
///
/// `continue value` is permitted only in `for...yield` (it substitutes the
/// element contributed for the current iteration). In `loop`, `while`, and
/// `for...do` it is E0861 — those loops do not accumulate values.
///
/// A labeled `continue:name value` resolves against the LABELED target loop's
/// form (per Spec: Clause 16), not the innermost loop. An unlabeled
/// `continue` targets the innermost loop.
pub(crate) fn infer_continue(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    label: Name,
    value: ExprId,
    _span: Span,
) -> Idx {
    if value.is_present() {
        // Infer the value first so any errors inside it still surface.
        let value_ty = infer_expr(engine, arena, value);
        let value_span = arena.get_expr(value).span;

        match engine.resolve_loop_context(label) {
            Some(ctx) if ctx.continue_value_allowed => {
                // `for...yield`: the value substitutes the element; unify with
                // the loop's element/break type so a uniform body type holds.
                let _ = engine.unify_types(value_ty, ctx.break_ty);
            }
            Some(ctx) => {
                let kind = match ctx.form {
                    LoopForm::Loop => NonCollectingLoopKind::Loop,
                    LoopForm::ForDo => NonCollectingLoopKind::ForDo,
                    // While is the only other non-collecting form; ForYield
                    // carries continue_value_allowed = true.
                    _ => NonCollectingLoopKind::While,
                };
                engine.push_error(TypeCheckError::continue_value_in_non_collecting_loop(
                    value_span, kind,
                ));
            }
            None => {
                // `continue value` with no resolvable target: outside any loop,
                // or a labeled continue whose label names no enclosing loop.
                engine.push_error(TypeCheckError::continue_value_in_non_collecting_loop(
                    value_span,
                    NonCollectingLoopKind::Loop,
                ));
            }
        }
    }

    Idx::NEVER
}
