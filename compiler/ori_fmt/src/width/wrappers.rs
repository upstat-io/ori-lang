//! Width calculation for wrapper expressions.
//!
//! Handles expressions that wrap an inner value with a prefix/suffix:
//! - Result constructors: `Ok(inner)`, `Err(inner)`
//! - Option constructor: `Some(inner)`
//! - Postfix operators: `inner?`, `inner.await`, `inner as type`

use super::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{ExprId, ParsedType, StringLookup};

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

/// Calculate width of `expr as type` or `expr as? type`.
pub(super) fn cast_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    expr: ExprId,
    ty: &ParsedType,
    fallible: bool,
) -> usize {
    let expr_w = calc.width(expr);
    if expr_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // Estimate type width from parsed type
    let type_w = estimate_type_width(ty, calc.interner);

    // " as " = 4, " as? " = 5
    let op_w = if fallible { 5 } else { 4 };
    expr_w + op_w + type_w
}

/// Estimate width of a parsed type for formatting purposes.
fn estimate_type_width<I: StringLookup>(ty: &ParsedType, interner: &I) -> usize {
    match ty {
        // Primitives and simple lists have similar average widths
        ParsedType::Primitive(_) | ParsedType::List(_) => 6,
        ParsedType::Named { name, type_args } => {
            let name_w = interner.lookup(*name).len();
            if type_args.is_empty() {
                name_w
            } else {
                // "Name<...>" - estimate args as 5 chars each
                name_w + 2 + (type_args.len() * 5)
            }
        }
        ParsedType::FixedList { .. } => 14, // "[int, max 100]" estimate
        ParsedType::Map { .. } => 12,       // "{str: int}" estimate
        // Tuples and associated types have similar estimated widths
        ParsedType::Tuple(_) | ParsedType::AssociatedType { .. } => 10,
        ParsedType::Function { .. } => 15, // "(int) -> str" estimate
        ParsedType::Infer => 1,            // "_"
        ParsedType::SelfType => 4,         // "Self"
    }
}
