//! Collection type inference: lists, tuples, maps, and ranges.

use super::super::infer_expr;
use crate::checker::TypeChecker;
use ori_ir::{ExprId, ExprRange, MapEntryRange, Span};
use ori_types::Type;

/// Infer type for a list literal.
pub fn infer_list(checker: &mut TypeChecker<'_>, elements: ExprRange) -> Type {
    let element_ids = checker.context.arena.get_expr_list(elements);

    if element_ids.is_empty() {
        let elem = checker.inference.ctx.fresh_var();
        checker.inference.ctx.make_list(elem)
    } else {
        let first_ty = infer_expr(checker, element_ids[0]);
        for id in &element_ids[1..] {
            let elem_ty = infer_expr(checker, *id);
            if let Err(e) = checker.inference.ctx.unify(&first_ty, &elem_ty) {
                checker.report_type_error(&e, checker.context.arena.get_expr(*id).span);
            }
        }
        checker.inference.ctx.make_list(first_ty)
    }
}

/// Infer type for a tuple literal.
pub fn infer_tuple(checker: &mut TypeChecker<'_>, elements: ExprRange) -> Type {
    let element_ids = checker.context.arena.get_expr_list(elements);
    if element_ids.is_empty() {
        Type::Unit
    } else {
        let types: Vec<Type> = element_ids
            .iter()
            .map(|id| infer_expr(checker, *id))
            .collect();
        checker.inference.ctx.make_tuple(types)
    }
}

/// Infer the type of a map literal (e.g., `{"a": 1, "b": 2}`).
///
/// Returns `Map<K, V>` where K and V are inferred from entries:
/// - Empty map: fresh type variables for key and value
/// - Non-empty: first entry sets types, subsequent entries unified against them
///
/// Reports errors if key or value types are inconsistent across entries.
pub fn infer_map(checker: &mut TypeChecker<'_>, entries: MapEntryRange, _span: Span) -> Type {
    let map_entries = checker.context.arena.get_map_entries(entries);
    if map_entries.is_empty() {
        let key = checker.inference.ctx.fresh_var();
        let value = checker.inference.ctx.fresh_var();
        checker.inference.ctx.make_map(key, value)
    } else {
        let first_key_ty = infer_expr(checker, map_entries[0].key);
        let first_val_ty = infer_expr(checker, map_entries[0].value);
        for entry in &map_entries[1..] {
            let key_ty = infer_expr(checker, entry.key);
            let val_ty = infer_expr(checker, entry.value);
            if let Err(e) = checker.inference.ctx.unify(&first_key_ty, &key_ty) {
                checker.report_type_error(&e, entry.span);
            }
            if let Err(e) = checker.inference.ctx.unify(&first_val_ty, &val_ty) {
                checker.report_type_error(&e, entry.span);
            }
        }
        checker.inference.ctx.make_map(first_key_ty, first_val_ty)
    }
}

/// Infer the type of a range expression (e.g., `0..10`, `1..=5`).
///
/// Returns `Range<T>` where T is inferred from bounds:
/// - If start provided, infers from start
/// - If only end provided, infers from end
/// - If neither, defaults to `Range<int>`
///
/// Unifies start and end types if both are present.
/// Step expression, if present, must be an integer.
pub fn infer_range(
    checker: &mut TypeChecker<'_>,
    start: Option<ExprId>,
    end: Option<ExprId>,
    step: Option<ExprId>,
) -> Type {
    let elem_ty = if let Some(start_id) = start {
        infer_expr(checker, start_id)
    } else if let Some(end_id) = end {
        infer_expr(checker, end_id)
    } else {
        Type::Int
    };

    if start.is_some() {
        if let Some(end_id) = end {
            let end_ty = infer_expr(checker, end_id);
            if let Err(e) = checker.inference.ctx.unify(&elem_ty, &end_ty) {
                checker.report_type_error(&e, checker.context.arena.get_expr(end_id).span);
            }
        }
    }

    // Step must be an integer (matches range element type)
    if let Some(step_id) = step {
        let step_ty = infer_expr(checker, step_id);
        if let Err(e) = checker.inference.ctx.unify(&elem_ty, &step_ty) {
            checker.report_type_error(&e, checker.context.arena.get_expr(step_id).span);
        }
    }

    checker.inference.ctx.make_range(elem_ty)
}
