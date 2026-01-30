//! Width calculation for wrapper expressions.
//!
//! Handles expressions that wrap an inner value with a prefix/suffix:
//! - Result constructors: `Ok(inner)`, `Err(inner)`
//! - Option constructor: `Some(inner)`
//! - Postfix operators: `inner?`, `inner.await`

use super::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{ExprId, StringLookup};

/// Helper for optional-inner wrapper width calculation.
///
/// Used by `ok_width` and `err_width` which follow the same pattern:
/// - `prefix_len`: length of prefix including open paren (e.g., 3 for `Ok(`)
/// - `empty_len`: length when inner is None (e.g., 4 for `Ok()`)
fn optional_wrapper_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    inner: Option<ExprId>,
    prefix_len: usize,
    empty_len: usize,
) -> usize {
    match inner {
        Some(expr) => {
            let inner_w = calc.width(expr);
            if inner_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            // prefix + inner + ")"
            prefix_len + inner_w + 1
        }
        None => empty_len,
    }
}

/// Calculate width of `Ok(inner)` or `Ok()`.
pub(super) fn ok_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    inner: Option<ExprId>,
) -> usize {
    optional_wrapper_width(calc, inner, 3, 4) // "Ok(" = 3, "Ok()" = 4
}

/// Calculate width of `Err(inner)` or `Err()`.
pub(super) fn err_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    inner: Option<ExprId>,
) -> usize {
    optional_wrapper_width(calc, inner, 4, 5) // "Err(" = 4, "Err()" = 5
}

/// Calculate width of `Some(inner)`.
pub(super) fn some_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    inner: ExprId,
) -> usize {
    let inner_w = calc.width(inner);
    if inner_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }
    // "Some(" + inner + ")"
    5 + inner_w + 1
}

/// Calculate width of `inner?` (try/propagate).
pub(super) fn try_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    inner: ExprId,
) -> usize {
    let inner_w = calc.width(inner);
    if inner_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }
    // inner + "?"
    inner_w + 1
}

/// Calculate width of `inner.await`.
pub(super) fn await_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    inner: ExprId,
) -> usize {
    let inner_w = calc.width(inner);
    if inner_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }
    // inner + ".await"
    inner_w + 6
}

/// Calculate width of `loop(body)`.
pub(super) fn loop_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    body: ExprId,
) -> usize {
    let body_w = calc.width(body);
    if body_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }
    // "loop(" + body + ")"
    5 + body_w + 1
}
