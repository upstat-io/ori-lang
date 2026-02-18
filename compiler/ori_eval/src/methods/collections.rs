//! Method dispatch for collection types (list, str, map, range, set).

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalResult, IteratorValue, Value};

use super::compare::{compare_lists, equals_values, hash_value, ordering_to_value};
use super::helpers::{
    debug_value, escape_debug_str, len_to_value, require_args, require_int_arg, require_list_arg,
    require_str_arg,
};
use super::DispatchCtx;

/// Dispatch methods on list values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_list_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::List(items) = receiver else {
        unreachable!("dispatch_list_method called with non-list receiver")
    };

    let n = ctx.names;

    // Note: Value clones in this function are cheap - Value uses Arc for heap types.
    if method == n.len {
        len_to_value(items.len(), "list")
    } else if method == n.is_empty {
        Ok(Value::Bool(items.is_empty()))
    } else if method == n.first {
        Ok(items.first().cloned().map_or(Value::None, Value::some))
    } else if method == n.last {
        Ok(items.last().cloned().map_or(Value::None, Value::some))
    } else if method == n.contains {
        require_args("contains", 1, args.len())?;
        Ok(Value::Bool(items.contains(&args[0])))
    } else if method == n.add || method == n.concat {
        require_args("concat", 1, args.len())?;
        let other = require_list_arg("concat", &args, 0)?;
        let mut result = (*items).clone();
        result.extend_from_slice(other);
        Ok(Value::list(result))
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_list_arg("compare", &args, 0)?;
        let ord = compare_lists(&items, other, ctx.interner)?;
        Ok(ordering_to_value(ord))
    // Eq trait - deep element-wise equality
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_list_arg("equals", &args, 0)?;
        if items.len() != other.len() {
            return Ok(Value::Bool(false));
        }
        for (a, b) in items.iter().zip(other.iter()) {
            if !equals_values(a, b, ctx.interner)? {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    // Hashable trait - recursive element hash
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&Value::List(items), ctx.interner)?))
    // Iterable trait - create iterator
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        Ok(Value::iterator(IteratorValue::from_list(items)))
    // Clone trait - deep clone of list
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(Value::list((*items).clone()))
    // Debug trait - shows list structure
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        let parts: Vec<String> = items.iter().map(debug_value).collect();
        Ok(Value::string(format!("[{}]", parts.join(", "))))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "list").into())
    }
}

/// Dispatch methods on string values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_string_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Str(s) = receiver else {
        unreachable!("dispatch_string_method called with non-string receiver")
    };

    let n = ctx.names;

    if method == n.len {
        len_to_value(s.len(), "string")
    } else if method == n.is_empty {
        Ok(Value::Bool(s.is_empty()))
    } else if method == n.to_uppercase {
        Ok(Value::string(s.to_uppercase()))
    } else if method == n.to_lowercase {
        Ok(Value::string(s.to_lowercase()))
    } else if method == n.trim {
        Ok(Value::string(s.trim().to_string()))
    } else if method == n.contains {
        require_args("contains", 1, args.len())?;
        let needle = require_str_arg("contains", &args, 0)?;
        Ok(Value::Bool(s.contains(needle)))
    } else if method == n.starts_with {
        require_args("starts_with", 1, args.len())?;
        let prefix = require_str_arg("starts_with", &args, 0)?;
        Ok(Value::Bool(s.starts_with(prefix)))
    } else if method == n.ends_with {
        require_args("ends_with", 1, args.len())?;
        let suffix = require_str_arg("ends_with", &args, 0)?;
        Ok(Value::Bool(s.ends_with(suffix)))
    } else if method == n.add || method == n.concat {
        require_args("concat", 1, args.len())?;
        let other = require_str_arg("concat", &args, 0)?;
        let result = format!("{}{}", &**s, other);
        Ok(Value::string(result))
    // Comparable trait - lexicographic (Unicode codepoint)
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_str_arg("compare", &args, 0)?;
        Ok(ordering_to_value((**s).cmp(other)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_str_arg("equals", &args, 0)?;
        Ok(Value::Bool(&**s == other))
    // Iterable trait - create character iterator
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        Ok(Value::iterator(IteratorValue::from_string(s)))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(Value::string(s.to_string()))
    // Printable trait - returns the string itself
    } else if method == n.to_str {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(s.to_string()))
    // escape - returns string with special characters escaped
    } else if method == n.escape {
        require_args("escape", 0, args.len())?;
        Ok(Value::string(escape_debug_str(&s)))
    // Debug trait - shows escaped string with quotes
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(format!("\"{}\"", escape_debug_str(&s))))
    // Hashable trait
    } else if method == n.hash {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        require_args("hash", 0, args.len())?;
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        Ok(Value::int(hasher.finish().cast_signed()))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "str").into())
    }
}

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
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "range").into())
    }
}

/// Dispatch methods on map values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_map_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Map(ref map) = receiver else {
        unreachable!("dispatch_map_method called with non-map receiver")
    };

    let n = ctx.names;

    if method == n.len {
        len_to_value(map.len(), "map")
    } else if method == n.is_empty {
        Ok(Value::Bool(map.is_empty()))
    } else if method == n.contains_key {
        require_args("contains_key", 1, args.len())?;
        let key = require_str_arg("contains_key", &args, 0)?;
        Ok(Value::Bool(map.contains_key(key)))
    } else if method == n.keys {
        // Clone Cow<str> keys to create Value::Str - required for owned return
        let keys: Vec<Value> = map.keys().map(|k| Value::string(k.clone())).collect();
        Ok(Value::list(keys))
    } else if method == n.values {
        // Clone values for return list. Cheap: Value uses Arc for heap types.
        let values: Vec<Value> = map.values().cloned().collect();
        Ok(Value::list(values))
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        Ok(Value::iterator(IteratorValue::from_map(map)))
    // Eq trait - deep value equality (order-independent)
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let eq = equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - order-independent XOR of entry hashes
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&receiver, ctx.interner)?))
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    // Debug trait - shows map structure
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&receiver)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "map").into())
    }
}

/// Dispatch methods on set values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_set_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Set(ref items) = receiver else {
        unreachable!("dispatch_set_method called with non-set receiver")
    };

    let n = ctx.names;

    if method == n.iter {
        require_args("iter", 0, args.len())?;
        // from_value always succeeds for Value::Set (returns Some)
        match IteratorValue::from_value(&receiver) {
            Some(iter) => Ok(Value::iterator(iter)),
            None => unreachable!("Set is always iterable"),
        }
    } else if method == n.len {
        require_args("len", 0, args.len())?;
        len_to_value(items.len(), "set")
    // Eq trait - deep value equality
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let eq = equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - order-independent XOR of element hashes
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&receiver, ctx.interner)?))
    // Debug trait - shows set structure
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&receiver)))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "Set").into())
    }
}
