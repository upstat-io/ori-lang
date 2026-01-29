//! Width calculation for call expressions.
//!
//! Handles function calls and method calls, both positional and named argument variants.

use super::{WidthCalculator, ALWAYS_STACKED};
use ori_ir::{CallArgRange, ExprId, ExprRange, Name, StringLookup};

/// Calculate width of a function call: `func(args)`.
pub(super) fn call_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    func: ExprId,
    args: ExprRange,
) -> usize {
    let func_w = calc.width(func);
    if func_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    let args_list = calc.arena.get_expr_list(args);
    let args_w = calc.width_of_expr_list(args_list);
    if args_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // func(arg1, arg2)
    func_w + 1 + args_w + 1
}

/// Calculate width of a function call with named arguments: `func(name: arg)`.
pub(super) fn call_named_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    func: ExprId,
    args: CallArgRange,
) -> usize {
    let func_w = calc.width(func);
    if func_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    let call_args = calc.arena.get_call_args(args);
    let args_w = calc.width_of_call_args(call_args);
    if args_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // func(name: arg1, name: arg2)
    func_w + 1 + args_w + 1
}

/// Calculate width of a method call: `receiver.method(args)`.
pub(super) fn method_call_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    receiver: ExprId,
    method: Name,
    args: ExprRange,
) -> usize {
    let receiver_w = calc.width(receiver);
    if receiver_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    let method_w = calc.interner.lookup(method).len();
    let args_list = calc.arena.get_expr_list(args);
    let args_w = calc.width_of_expr_list(args_list);
    if args_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // receiver.method(args)
    receiver_w + 1 + method_w + 1 + args_w + 1
}

/// Calculate width of a method call with named arguments: `receiver.method(name: arg)`.
pub(super) fn method_call_named_width<I: StringLookup>(
    calc: &mut WidthCalculator<'_, I>,
    receiver: ExprId,
    method: Name,
    args: CallArgRange,
) -> usize {
    let receiver_w = calc.width(receiver);
    if receiver_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    let method_w = calc.interner.lookup(method).len();
    let call_args = calc.arena.get_call_args(args);
    let args_w = calc.width_of_call_args(call_args);
    if args_w == ALWAYS_STACKED {
        return ALWAYS_STACKED;
    }

    // receiver.method(name: arg)
    receiver_w + 1 + method_w + 1 + args_w + 1
}
