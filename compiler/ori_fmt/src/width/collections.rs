//! Width calculation for collection literals.
//!
//! Handles list, tuple, map, and struct literals.

use super::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{ExprId, ExprRange, FieldInitRange, MapEntryRange, Name, StringLookup};

/// Calculate width of a list literal: `[items]`.
pub(super) fn list_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    items: ExprRange,
) -> usize {
    let items_list = calc.arena.get_expr_list(items);
    if items_list.is_empty() {
        return 2; // "[]"
    }

    let items_w = calc.width_of_expr_list(items_list);
    if items_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "[" + items + "]"
    1 + items_w + 1
}

/// Calculate width of a tuple literal: `(items)`.
/// Single-element tuples need trailing comma: `(x,)`.
pub(super) fn tuple_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    items: ExprRange,
) -> usize {
    let items_list = calc.arena.get_expr_list(items);
    if items_list.is_empty() {
        return 2; // "()"
    }

    let items_w = calc.width_of_expr_list(items_list);
    if items_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "(" + items + ")" + optional trailing comma for single element
    let trailing_comma = if items_list.len() == 1 { 1 } else { 0 };
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

/// Calculate width of a range expression: `start..end` or `start..=end`.
pub(super) fn range_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    start: Option<ExprId>,
    end: Option<ExprId>,
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

    total
}
