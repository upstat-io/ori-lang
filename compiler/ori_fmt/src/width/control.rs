//! Width calculation for control flow expressions.
//!
//! Handles:
//! - Jump statements: `return`, `break`, `continue`
//! - Conditionals: `if cond then expr else expr`
//! - Loops: `for binding in iter do/yield body`
//! - Blocks: `{ stmts; result }`

use super::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{ExprId, ExprKind, Name, StmtRange, StringLookup};

/// Check if an expression needs parentheses when used as a receiver.
fn receiver_needs_parens<I: StringLookup>(calc: &WidthCalculator<'_, I>, receiver: ExprId) -> bool {
    let expr = calc.arena.get_expr(receiver);
    matches!(
        expr.kind,
        ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::If { .. }
            | ExprKind::Lambda { .. }
            | ExprKind::Let { .. }
            | ExprKind::Range { .. }
    )
}

/// Calculate width of `return` or `return value`.
pub(super) fn return_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    value: Option<ExprId>,
) -> usize {
    match value {
        Some(expr) => {
            let val_w = calc.width(expr);
            if val_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            // "return " + val
            7 + val_w
        }
        None => 6, // "return"
    }
}

/// Calculate width of `break` or `break value`.
pub(super) fn break_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    value: Option<ExprId>,
) -> usize {
    match value {
        Some(expr) => {
            let val_w = calc.width(expr);
            if val_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            // "break " + val
            6 + val_w
        }
        None => 5, // "break"
    }
}

/// Calculate width of `if cond then expr [else expr]`.
pub(super) fn if_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    cond: ExprId,
    then_branch: ExprId,
    else_branch: Option<ExprId>,
) -> usize {
    let cond_w = calc.width(cond);
    let then_w = calc.width(then_branch);
    if cond_w == ALWAYS_STACKED || then_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "if " + cond + " then " + then
    let mut total = 3 + cond_w + 6 + then_w;

    if let Some(else_expr) = else_branch {
        let else_w = calc.width(else_expr);
        if else_w == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        // " else " + else
        total += 6 + else_w;
    }

    total
}

/// Calculate width of `for binding in iter [if guard] do/yield body`.
pub(super) fn for_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    binding: Name,
    iter: ExprId,
    guard: Option<ExprId>,
    body: ExprId,
    is_yield: bool,
) -> usize {
    let binding_w = calc.interner.lookup(binding).len();
    let iter_w = calc.width(iter);
    let body_w = calc.width(body);
    if iter_w == ALWAYS_STACKED || body_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // "for " + binding + " in " + iter
    let mut total = 4 + binding_w + 4 + iter_w;

    if let Some(guard_expr) = guard {
        let guard_w = calc.width(guard_expr);
        if guard_w == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        // " if " + guard
        total += 4 + guard_w;
    }

    // " do " or " yield " + body
    if is_yield {
        total += 7 + body_w; // " yield "
    } else {
        total += 4 + body_w; // " do "
    }

    total
}

/// Calculate width of `{ stmts; result }` block.
pub(super) fn block_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    stmts: StmtRange,
    result: Option<ExprId>,
) -> usize {
    // Always stacked if has statements
    if !stmts.is_empty() {
        return ALWAYS_STACKED;
    }

    match result {
        Some(result_expr) => {
            let result_w = calc.width(result_expr);
            if result_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            // "{ " + result + " }"
            2 + result_w + 2
        }
        None => 2, // "{}"
    }
}

/// Calculate width of `target = value` assignment.
pub(super) fn assign_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    target: ExprId,
    value: ExprId,
) -> usize {
    let target_w = calc.width(target);
    let value_w = calc.width(value);
    if target_w == ALWAYS_STACKED || value_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }
    // target + " = " + value
    target_w + 3 + value_w
}

/// Calculate width of `receiver.field` access.
/// Adds 2 for parentheses if receiver needs them for precedence.
pub(super) fn field_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    receiver: ExprId,
    field: Name,
) -> usize {
    let receiver_w = calc.width(receiver);
    if receiver_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }
    let paren_w = if receiver_needs_parens(calc, receiver) { 2 } else { 0 };
    let field_w = calc.interner.lookup(field).len();
    paren_w + receiver_w + 1 + field_w
}

/// Calculate width of `receiver[index]` access.
/// Adds 2 for parentheses if receiver needs them for precedence.
pub(super) fn index_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    receiver: ExprId,
    index: ExprId,
) -> usize {
    let receiver_w = calc.width(receiver);
    let index_w = calc.width(index);
    if receiver_w == ALWAYS_STACKED || index_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }
    let paren_w = if receiver_needs_parens(calc, receiver) { 2 } else { 0 };
    // (receiver)[index] or receiver[index]
    paren_w + receiver_w + 1 + index_w + 1
}

/// Calculate width of `with Cap = provider in body`.
pub(super) fn with_capability_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    capability: Name,
    provider: ExprId,
    body: ExprId,
) -> usize {
    let cap_w = calc.interner.lookup(capability).len();
    let provider_w = calc.width(provider);
    let body_w = calc.width(body);
    if provider_w == ALWAYS_STACKED || body_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }
    // "with " + Cap + " = " + provider + " in " + body
    5 + cap_w + 3 + provider_w + 4 + body_w
}
