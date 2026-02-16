//! Method dispatch for `Iterator<T>` values.

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalResult, Value};

use super::helpers;
use super::DispatchCtx;

/// Dispatch a method call on an `Iterator<T>` value.
pub(crate) fn dispatch_iterator_method(
    receiver: Value,
    method: Name,
    args: &[Value],
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;

    if method == n.next {
        helpers::require_args("next", 0, args.len())?;
        let Value::Iterator(iter_val) = receiver else {
            unreachable!("dispatch_iterator_method called with non-iterator receiver")
        };
        let (maybe_item, new_iter) = iter_val.next();
        let option_val = match maybe_item {
            Some(v) => Value::some(v),
            None => Value::None,
        };
        Ok(Value::tuple(vec![option_val, Value::iterator(new_iter)]))
    } else {
        let method_str = ctx.interner.lookup(method);
        Err(no_such_method(method_str, "Iterator").into())
    }
}
