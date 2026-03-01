//! Method dispatch for list values.

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalResult, IteratorValue, ListData, Value};

use super::compare::{compare_lists, compare_values, equals_values, hash_value, ordering_to_value};
use super::helpers::{debug_value, len_to_value, require_args, require_int_arg, require_list_arg};
use super::DispatchCtx;

/// Dispatch methods on list values.
pub fn dispatch_list_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::List(mut list) = receiver else {
        unreachable!("dispatch_list_method called with non-list receiver")
    };

    let n = ctx.names;

    // Note: Value clones in this function are cheap - Value uses Arc for heap types.
    // Read-only methods work through ListData's Deref<Target=[Value]>.
    if method == n.len || method == n.length {
        len_to_value(list.len(), "list")
    } else if method == n.is_empty {
        Ok(Value::Bool(list.is_empty()))
    } else if method == n.first {
        Ok(list.first().cloned().map_or(Value::None, Value::some))
    } else if method == n.last || method == n.pop {
        Ok(list.last().cloned().map_or(Value::None, Value::some))
    } else if method == n.contains {
        require_args("contains", 1, args.len())?;
        Ok(Value::Bool(list.contains(&args[0])))
    } else if method == n.push {
        require_args("push", 1, args.len())?;
        let mut args = args;
        list.push(args.swap_remove(0));
        Ok(Value::List(list))
    } else if method == n.set {
        list_set(list, args)
    } else if method == n.insert {
        list_insert(list, args)
    } else if method == n.remove {
        list_remove(list, &args)
    } else if method == n.reverse {
        require_args("reverse", 0, args.len())?;
        list.reverse();
        Ok(Value::List(list))
    } else if method == n.sort || method == n.sort_stable {
        let name = if method == n.sort {
            "sort"
        } else {
            "sort_stable"
        };
        require_args(name, 0, args.len())?;
        sort_list(list, ctx)
    } else if method == n.add || method == n.concat {
        require_args("concat", 1, args.len())?;
        let other = require_list_arg("concat", &args, 0)?;
        list.extend_from_slice(other);
        Ok(Value::List(list))
    // Zero-copy slice operations — share the same Arc backing buffer
    } else if method == n.slice {
        list_slice(&list, &args)
    } else if method == n.take {
        list_take(&list, &args)
    } else if method == n.skip || method == n.drop_ {
        let name = if method == n.skip { "skip" } else { "drop" };
        list_skip(&list, &args, name)
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_list_arg("compare", &args, 0)?;
        let ord = compare_lists(&list, other, ctx.interner)?;
        Ok(ordering_to_value(ord))
    // Eq trait - deep element-wise equality
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_list_arg("equals", &args, 0)?;
        list_equals(&list, other, ctx)
    // Hashable trait - recursive element hash
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&Value::List(list), ctx.interner)?))
    // Iterable trait - create iterator
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        Ok(Value::iterator(IteratorValue::from_list_data(&list)))
    // Clone trait - deep clone of list
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(Value::list(list[..].to_vec()))
    // Debug trait - shows list structure
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        let parts: Vec<String> = list.iter().map(debug_value).collect();
        Ok(Value::string(format!("[{}]", parts.join(", "))))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "list").into())
    }
}

// List method helpers

/// Set element at index (materializes slice if needed, then COW).
fn list_set(mut list: ListData, mut args: Vec<Value>) -> EvalResult {
    require_args("set", 2, args.len())?;
    let index = require_int_arg("set", &args, 0)?;
    let uindex = usize::try_from(index)
        .map_err(|_| ori_patterns::wrong_arg_type("set", "non-negative int"))?;
    if uindex >= list.len() {
        return Err(ori_patterns::wrong_arg_type("set", "index within bounds").into());
    }
    list.set(uindex, args.swap_remove(1));
    Ok(Value::List(list))
}

/// Insert element at index (materializes slice if needed, then COW).
fn list_insert(mut list: ListData, mut args: Vec<Value>) -> EvalResult {
    require_args("insert", 2, args.len())?;
    let index = require_int_arg("insert", &args, 0)?;
    let uindex = usize::try_from(index)
        .map_err(|_| ori_patterns::wrong_arg_type("insert", "non-negative int"))?;
    if uindex > list.len() {
        return Err(ori_patterns::wrong_arg_type("insert", "index within bounds").into());
    }
    list.insert(uindex, args.swap_remove(1));
    Ok(Value::List(list))
}

/// Remove element at index (materializes slice if needed, then COW).
fn list_remove(mut list: ListData, args: &[Value]) -> EvalResult {
    require_args("remove", 1, args.len())?;
    let index = require_int_arg("remove", args, 0)?;
    let uindex = usize::try_from(index)
        .map_err(|_| ori_patterns::wrong_arg_type("remove", "non-negative int"))?;
    if uindex >= list.len() {
        return Err(ori_patterns::wrong_arg_type("remove", "index within bounds").into());
    }
    list.remove(uindex);
    Ok(Value::List(list))
}

/// Extract a zero-copy sublist from `start` to `end` (clamped to bounds).
fn list_slice(list: &ListData, args: &[Value]) -> EvalResult {
    require_args("slice", 2, args.len())?;
    let start = require_int_arg("slice", args, 0)?;
    let end = require_int_arg("slice", args, 1)?;
    let ustart = usize::try_from(start)
        .map_err(|_| ori_patterns::wrong_arg_type("slice", "non-negative int"))?;
    let uend = usize::try_from(end)
        .map_err(|_| ori_patterns::wrong_arg_type("slice", "non-negative int"))?;
    Ok(Value::List(list.slice(ustart, uend)))
}

/// Take the first `n` elements as a zero-copy slice.
fn list_take(list: &ListData, args: &[Value]) -> EvalResult {
    require_args("take", 1, args.len())?;
    let count = require_int_arg("take", args, 0)?;
    let n = usize::try_from(count)
        .map_err(|_| ori_patterns::wrong_arg_type("take", "non-negative int"))?;
    Ok(Value::List(list.take(n)))
}

/// Skip the first `n` elements as a zero-copy slice.
fn list_skip(list: &ListData, args: &[Value], name: &str) -> EvalResult {
    require_args(name, 1, args.len())?;
    let count = require_int_arg(name, args, 0)?;
    let n = usize::try_from(count)
        .map_err(|_| ori_patterns::wrong_arg_type(name, "non-negative int"))?;
    Ok(Value::List(list.skip(n)))
}

/// Sort a list by comparing element values, propagating comparison errors.
///
/// Takes ownership of the `ListData` to enable COW: if the list has
/// a single reference (RC==1), the sort happens in-place with no allocation.
fn sort_list(mut list: ListData, ctx: &DispatchCtx<'_>) -> EvalResult {
    let interner = ctx.interner;
    let mut err: Option<ori_patterns::EvalError> = None;
    list.sort_by(|a, b| {
        if err.is_some() {
            return std::cmp::Ordering::Equal;
        }
        match compare_values(a, b, interner) {
            Ok(ord) => ord,
            Err(e) => {
                err = Some(e);
                std::cmp::Ordering::Equal
            }
        }
    });
    if let Some(e) = err {
        return Err(e.into());
    }
    Ok(Value::List(list))
}

/// Deep element-wise equality comparison between two lists.
fn list_equals(items: &[Value], other: &[Value], ctx: &DispatchCtx<'_>) -> EvalResult {
    if items.len() != other.len() {
        return Ok(Value::Bool(false));
    }
    for (a, b) in items.iter().zip(other.iter()) {
        if !equals_values(a, b, ctx.interner)? {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}
