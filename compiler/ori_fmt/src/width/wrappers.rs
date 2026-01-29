//! Width calculation for wrapper expressions.
//!
//! Handles expressions that wrap an inner value with a prefix/suffix:
//! - Result constructors: `Ok(inner)`, `Err(inner)`
//! - Option constructor: `Some(inner)`
//! - Postfix operators: `inner?`, `inner.await`

use super::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{ExprId, StringLookup};

/// Calculate width of `Ok(inner)` or `Ok()`.
pub(super) fn ok_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    inner: Option<ExprId>,
) -> usize {
    match inner {
        Some(expr) => {
            let inner_w = calc.width(expr);
            if inner_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            // "Ok(" + inner + ")"
            3 + inner_w + 1
        }
        None => 4, // "Ok()"
    }
}

/// Calculate width of `Err(inner)` or `Err()`.
pub(super) fn err_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    inner: Option<ExprId>,
) -> usize {
    match inner {
        Some(expr) => {
            let inner_w = calc.width(expr);
            if inner_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            // "Err(" + inner + ")"
            4 + inner_w + 1
        }
        None => 5, // "Err()"
    }
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
