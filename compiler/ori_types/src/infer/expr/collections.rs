//! Collection inference — list, tuple, map, and range literals.

use ori_ir::{ExprArena, ExprId, Span};

use super::super::InferEngine;
use super::infer_expr;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, SequenceKind, Tag, TypeCheckError};

/// Infer the type of a list literal.
pub(crate) fn infer_list(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::ExprRange,
    _span: Span,
) -> Idx {
    let elem_ids: Vec<_> = arena.get_expr_list(elements).to_vec();

    if elem_ids.is_empty() {
        return engine.infer_empty_list();
    }

    // Infer first element
    let first_ty = infer_expr(engine, arena, elem_ids[0]);
    let first_span = arena.get_expr(elem_ids[0]).span;

    // Check remaining elements
    for (i, &elem_id) in elem_ids.iter().skip(1).enumerate() {
        let expected = Expected {
            ty: first_ty,
            origin: ExpectedOrigin::PreviousInSequence {
                previous_span: first_span,
                current_index: i + 1,
                sequence_kind: SequenceKind::ListLiteral,
            },
        };
        let elem_ty = infer_expr(engine, arena, elem_id);
        let _ = engine.check_type(elem_ty, &expected, arena.get_expr(elem_id).span);
    }

    let resolved_elem = engine.resolve(first_ty);
    engine.infer_list(resolved_elem)
}

/// Infer the type of a list literal with spread syntax.
pub(crate) fn infer_list_spread(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::ListElementRange,
    _span: Span,
) -> Idx {
    use ori_ir::ListElement;

    let elems = arena.get_list_elements(elements);
    if elems.is_empty() {
        return engine.infer_empty_list();
    }

    // Unified element type — start with a fresh variable
    let elem_ty = engine.fresh_var();

    for element in elems {
        match element {
            ListElement::Expr {
                expr,
                span: el_span,
            } => {
                let ty = infer_expr(engine, arena, *expr);
                if engine.unify_types(ty, elem_ty).is_err() {
                    engine.push_error(TypeCheckError::mismatch(
                        *el_span,
                        elem_ty,
                        ty,
                        vec![],
                        crate::ErrorContext::new(ContextKind::ListElement { index: 0 }),
                    ));
                }
            }
            ListElement::Spread {
                expr,
                span: sp_span,
            } => {
                let spread_ty = infer_expr(engine, arena, *expr);
                let resolved = engine.resolve(spread_ty);
                if engine.pool().tag(resolved) == Tag::List {
                    let inner = engine.pool().list_elem(resolved);
                    if engine.unify_types(inner, elem_ty).is_err() {
                        engine.push_error(TypeCheckError::mismatch(
                            *sp_span,
                            elem_ty,
                            inner,
                            vec![],
                            crate::ErrorContext::new(ContextKind::ListElement { index: 0 }),
                        ));
                    }
                } else if resolved != Idx::ERROR {
                    // Spread target must be a list
                    let expected_list = engine.infer_list(elem_ty);
                    engine.push_error(TypeCheckError::mismatch(
                        *sp_span,
                        expected_list,
                        resolved,
                        vec![],
                        crate::ErrorContext::new(ContextKind::PatternMatch {
                            pattern_kind: "list spread",
                        }),
                    ));
                }
            }
        }
    }

    let resolved_elem = engine.resolve(elem_ty);
    engine.infer_list(resolved_elem)
}

/// Infer the type of a tuple literal.
pub(crate) fn infer_tuple(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::ExprRange,
    _span: Span,
) -> Idx {
    let elem_ids: Vec<_> = arena.get_expr_list(elements).to_vec();
    let elem_types: Vec<_> = elem_ids
        .iter()
        .map(|&id| infer_expr(engine, arena, id))
        .collect();
    engine.infer_tuple(&elem_types)
}

/// Infer the type of a map literal.
pub(crate) fn infer_map_literal(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    entries: ori_ir::MapEntryRange,
    _span: Span,
) -> Idx {
    let entries_slice = arena.get_map_entries(entries);

    if entries_slice.is_empty() {
        return engine.infer_empty_map();
    }

    // Infer first entry
    let first_entry = &entries_slice[0];
    let first_key_ty = infer_expr(engine, arena, first_entry.key);
    let first_val_ty = infer_expr(engine, arena, first_entry.value);

    // Check remaining entries
    for entry in entries_slice.iter().skip(1) {
        let key_ty = infer_expr(engine, arena, entry.key);
        let val_ty = infer_expr(engine, arena, entry.value);
        let _ = engine.unify_types(key_ty, first_key_ty);
        let _ = engine.unify_types(val_ty, first_val_ty);
    }

    let resolved_key = engine.resolve(first_key_ty);
    let resolved_val = engine.resolve(first_val_ty);
    engine.infer_map(resolved_key, resolved_val)
}

/// Infer the type of a map literal with spread syntax.
pub(crate) fn infer_map_spread(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    elements: ori_ir::MapElementRange,
    _span: Span,
) -> Idx {
    use ori_ir::MapElement;

    let elems = arena.get_map_elements(elements);
    if elems.is_empty() {
        return engine.infer_empty_map();
    }

    // Unified key and value types — start with fresh variables
    let key_ty = engine.fresh_var();
    let val_ty = engine.fresh_var();

    for element in elems {
        match element {
            MapElement::Entry(entry) => {
                let k = infer_expr(engine, arena, entry.key);
                let v = infer_expr(engine, arena, entry.value);
                let _ = engine.unify_types(k, key_ty);
                let _ = engine.unify_types(v, val_ty);
            }
            MapElement::Spread {
                expr,
                span: sp_span,
            } => {
                let spread_ty = infer_expr(engine, arena, *expr);
                let resolved = engine.resolve(spread_ty);
                if engine.pool().tag(resolved) == Tag::Map {
                    let k = engine.pool().map_key(resolved);
                    let v = engine.pool().map_value(resolved);
                    let _ = engine.unify_types(k, key_ty);
                    let _ = engine.unify_types(v, val_ty);
                } else if resolved != Idx::ERROR {
                    // Spread target must be a map
                    let expected_map = engine.infer_map(key_ty, val_ty);
                    engine.push_error(TypeCheckError::mismatch(
                        *sp_span,
                        expected_map,
                        resolved,
                        vec![],
                        crate::ErrorContext::new(ContextKind::PatternMatch {
                            pattern_kind: "map spread",
                        }),
                    ));
                }
            }
        }
    }

    let resolved_key = engine.resolve(key_ty);
    let resolved_val = engine.resolve(val_ty);
    engine.infer_map(resolved_key, resolved_val)
}

/// Infer the type of a range expression.
pub(crate) fn infer_range(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    start: ExprId,
    end: ExprId,
    step: ExprId,
    _inclusive: bool,
    _span: Span,
) -> Idx {
    // Determine element type from provided bounds
    let elem_ty = if start.is_present() {
        infer_expr(engine, arena, start)
    } else if end.is_present() {
        infer_expr(engine, arena, end)
    } else {
        Idx::INT // Default to int for open ranges
    };

    // Unify all provided bounds
    if start.is_present() {
        let ty = infer_expr(engine, arena, start);
        let _ = engine.unify_types(ty, elem_ty);
    }
    if end.is_present() {
        let ty = infer_expr(engine, arena, end);
        let _ = engine.unify_types(ty, elem_ty);
    }
    if step.is_present() {
        let ty = infer_expr(engine, arena, step);
        let _ = engine.unify_types(ty, elem_ty);
    }

    let resolved = engine.resolve(elem_ty);
    engine.pool_mut().range(resolved)
}
