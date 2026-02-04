//! Width calculation for collection literals.
//!
//! Handles list, tuple, map, and struct literals.

use super::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{
    ExprId, ExprList, FieldInitRange, ListElementRange, MapElementRange, MapEntryRange, Name,
    StringLookup, StructLitFieldRange,
};

/// Calculate width of a list literal: `[items]`.
pub(super) fn list_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    items: ExprList,
) -> usize {
    if items.is_empty() {
        return 2; // "[]"
    }

    let items_vec: Vec<_> = calc.arena.iter_expr_list(items).collect();
    let items_w = calc.width_of_expr_list(&items_vec);
    if items_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "[" + items + "]"
    1 + items_w + 1
}

/// Calculate width of a list literal with spread: `[...a, x, ...b]`.
pub(super) fn list_with_spread_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    elements: ListElementRange,
) -> usize {
    let elements_list = calc.arena.get_list_elements(elements);
    if elements_list.is_empty() {
        return 2; // "[]"
    }

    let elements_w = calc.width_of_list_elements(elements_list);
    if elements_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "[" + elements + "]"
    1 + elements_w + 1
}

/// Calculate width of a tuple literal: `(items)`.
/// Single-element tuples need trailing comma: `(x,)`.
pub(super) fn tuple_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    items: ExprList,
) -> usize {
    if items.is_empty() {
        return 2; // "()"
    }

    let items_vec: Vec<_> = calc.arena.iter_expr_list(items).collect();
    let items_w = calc.width_of_expr_list(&items_vec);
    if items_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "(" + items + ")" + optional trailing comma for single element
    let trailing_comma = usize::from(items_vec.len() == 1);
    1 + items_w + trailing_comma + 1
}

/// Calculate width of a map literal: `{entries}`.
pub(super) fn map_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    entries: MapEntryRange,
) -> usize {
    let entries_list = calc.arena.get_map_entries(entries);
    if entries_list.is_empty() {
        return 2; // "{}"
    }

    let entries_w = calc.width_of_map_entries(entries_list);
    if entries_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "{" + entries + "}"
    1 + entries_w + 1
}

/// Calculate width of a map literal with spread: `{...base, key: value}`.
pub(super) fn map_with_spread_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    elements: MapElementRange,
) -> usize {
    let elements_list = calc.arena.get_map_elements(elements);
    if elements_list.is_empty() {
        return 2; // "{}"
    }

    let elements_w = calc.width_of_map_elements(elements_list);
    if elements_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "{" + elements + "}"
    1 + elements_w + 1
}

/// Calculate width of a struct literal: `Name { fields }`.
pub(super) fn struct_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    name: Name,
    fields: FieldInitRange,
) -> usize {
    let name_w = calc.interner.lookup(name).len();
    let fields_list = calc.arena.get_field_inits(fields);

    if fields_list.is_empty() {
        // "Name {}"
        return name_w + 3;
    }

    let fields_w = calc.width_of_field_inits(fields_list);
    if fields_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "Name { " + fields + " }"
    name_w + 3 + fields_w + 2
}

/// Calculate width of a struct literal with spread: `Name { ...base, x: 10 }`.
pub(super) fn struct_with_spread_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    name: Name,
    fields: StructLitFieldRange,
) -> usize {
    let name_w = calc.interner.lookup(name).len();
    let fields_list = calc.arena.get_struct_lit_fields(fields);

    if fields_list.is_empty() {
        // "Name {}"
        return name_w + 3;
    }

    let fields_w = calc.width_of_struct_lit_fields(fields_list);
    if fields_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "Name { " + fields + " }"
    name_w + 3 + fields_w + 2
}

/// Calculate width of a range expression: `start..end` or `start..=end` or `start..end by step`.
pub(super) fn range_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    start: Option<ExprId>,
    end: Option<ExprId>,
    step: Option<ExprId>,
    inclusive: bool,
) -> usize {
    let mut total = 0;

    if let Some(start_expr) = start {
        let start_w = calc.width(start_expr);
        if start_w == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        total += start_w;
    }

    // ".." or "..="
    total += if inclusive { 3 } else { 2 };

    if let Some(end_expr) = end {
        let end_w = calc.width(end_expr);
        if end_w == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        total += end_w;
    }

    // " by " + step
    if let Some(step_expr) = step {
        let step_w = calc.width(step_expr);
        if step_w == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        total += 4 + step_w; // " by " + step
    }

    total
}
