//! Free-function method dispatch for `range` values.

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalResult, IteratorValue, Value};

use super::super::arguments::{require_args, require_int_arg};
use super::super::length::len_to_value;
use super::super::DispatchCtx;

/// Dispatch methods on range values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_range_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Range(r) = receiver else {
        unreachable!("dispatch_range_method called with non-range receiver")
    };

    let n = ctx.names;

    if method == n.len {
        if r.is_unbounded() {
            return Err(ori_patterns::unbounded_range_length().into());
        }
        len_to_value(r.len(), "range")
    } else if method == n.contains {
        require_args("contains", 1, args.len())?;
        let val = require_int_arg("contains", &args, 0)?;
        Ok(Value::Bool(r.contains(val)))
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        Ok(Value::iterator(IteratorValue::from_range(
            r.start,
            r.end,
            r.step,
            r.inclusive,
        )))
    // Additional range methods (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_range_method_str(&r, method_str, &args)
    }
}

/// String-based dispatch for range methods not covered by Name-based dispatch.
fn dispatch_range_method_str(
    r: &ori_patterns::RangeValue,
    method: &str,
    args: &[Value],
) -> EvalResult {
    match method {
        "count" => {
            require_args("count", 0, args.len())?;
            if r.is_unbounded() {
                return Err(ori_patterns::unbounded_range_length().into());
            }
            len_to_value(r.len(), "range")
        }
        "is_empty" => {
            require_args("is_empty", 0, args.len())?;
            #[expect(clippy::len_zero, reason = "RangeValue has no is_empty()")]
            Ok(Value::Bool(r.len() == 0))
        }
        "step_by" => {
            require_args("step_by", 1, args.len())?;
            let step = require_int_arg("step_by", args, 0)?;
            if step == 0 {
                return Err(ori_patterns::wrong_arg_type("step_by", "non-zero int").into());
            }
            let new_range = ori_patterns::RangeValue {
                start: r.start,
                end: r.end,
                step,
                inclusive: r.inclusive,
            };
            Ok(Value::Range(new_range))
        }
        "to_list" => {
            require_args("to_list", 0, args.len())?;
            if r.is_unbounded() {
                return Err(ori_patterns::unbounded_range_length().into());
            }
            let items: Vec<Value> = r.iter().map(Value::int).collect();
            Ok(Value::list(items))
        }
        // Collect range into a list (also dispatched by CollectionMethodResolver)
        "collect" => {
            require_args("collect", 0, args.len())?;
            if r.is_unbounded() {
                return Err(ori_patterns::unbounded_range_length().into());
            }
            let items: Vec<Value> = r.iter().map(Value::int).collect();
            Ok(Value::list(items))
        }
        // Higher-order methods requiring closures are dispatched by
        // CollectionMethodResolver; recognizing them preserves dispatch coverage.
        "all" | "any" | "filter" | "find" | "fold" | "map" => {
            require_args(method, 1, args.len())?;
            Err(ori_patterns::wrong_arg_type(method, "function").into())
        }
        _ => Err(no_such_method(method, "range").into()),
    }
}
