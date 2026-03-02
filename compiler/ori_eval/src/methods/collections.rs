//! Method dispatch for collection types (str, map, range, set).
//!
//! List dispatch lives in the sibling `list` module.

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalResult, IteratorValue, Value};

use super::compare::{equals_values, hash_value, ordering_to_value};
use super::helpers::{
    debug_value, escape_debug_str, len_to_value, require_args, require_int_arg, require_str_arg,
};
use super::DispatchCtx;

/// Decode an internal map key back to a `Value`.
///
/// Map keys are stored with type prefixes (e.g., `"s:hello"`, `"i:42"`) via
/// `Value::to_map_key()`. This reverses the encoding for user-facing methods
/// like `keys()` and `entries()`.
fn decode_map_key(key: &str) -> Value {
    match key.split_once(':') {
        Some(("s", rest)) => Value::string(rest.to_string()),
        Some(("i", rest)) => rest
            .parse::<i64>()
            .map_or_else(|_| Value::string(key.to_string()), Value::int),
        Some(("f", rest)) => rest
            .parse::<f64>()
            .map_or_else(|_| Value::string(key.to_string()), Value::Float),
        Some(("b", rest)) => rest
            .parse::<bool>()
            .map_or_else(|_| Value::string(key.to_string()), Value::Bool),
        Some(("c", rest)) => {
            let mut chars = rest.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Value::Char(c),
                _ => Value::string(key.to_string()),
            }
        }
        Some(("y", rest)) => rest
            .parse::<u8>()
            .map_or_else(|_| Value::string(key.to_string()), Value::Byte),
        // Complex keys (Duration, Size, nested) — fall back to raw string
        _ => Value::string(key.to_string()),
    }
}

/// Dispatch methods on string values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "args: Vec<Value> for consistent dispatch signature across collection types"
)]
pub fn dispatch_string_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Str(mut s) = receiver else {
        unreachable!("dispatch_string_method called with non-string receiver")
    };

    let n = ctx.names;

    if method == n.len || method == n.length {
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
        s.make_mut().to_mut().push_str(other);
        Ok(Value::Str(s))
    } else if method == n.substring || method == n.slice {
        let name = if method == n.substring {
            "substring"
        } else {
            "slice"
        };
        eval_str_slice(&s, name, &args)
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
    // replace(pattern, replacement) -> str
    } else if method == n.replace {
        require_args("replace", 2, args.len())?;
        let pattern = require_str_arg("replace", &args, 0)?;
        let replacement = require_str_arg("replace", &args, 1)?;
        Ok(Value::string(s.replace(pattern, replacement)))
    // split(separator) -> [str]
    } else if method == n.split {
        require_args("split", 1, args.len())?;
        let sep = require_str_arg("split", &args, 0)?;
        let parts: Vec<Value> = if sep.is_empty() {
            // Empty separator: split into individual characters
            s.chars().map(|c| Value::string(c.to_string())).collect()
        } else {
            s.split(sep).map(|p| Value::string(p.to_string())).collect()
        };
        Ok(Value::list(parts))
    // repeat(count) -> str
    } else if method == n.repeat {
        require_args("repeat", 1, args.len())?;
        let count = require_int_arg("repeat", &args, 0)?;
        let n_usize = usize::try_from(count)
            .map_err(|_| ori_patterns::wrong_arg_type("repeat", "non-negative int"))?;
        Ok(Value::string(s.repeat(n_usize)))
    // Into trait: str -> Error (wraps string as error message)
    } else if method == n.into {
        require_args("into", 0, args.len())?;
        Ok(Value::error(s.to_string()))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "str").into())
    }
}

/// `substring`/`slice` — extract char-indexed substring.
fn eval_str_slice(s: &str, name: &str, args: &[Value]) -> EvalResult {
    require_args(name, 2, args.len())?;
    let start = require_int_arg(name, args, 0)?;
    let end = require_int_arg(name, args, 1)?;
    let ustart = usize::try_from(start)
        .map_err(|_| ori_patterns::wrong_arg_type(name, "non-negative int"))?;
    let uend =
        usize::try_from(end).map_err(|_| ori_patterns::wrong_arg_type(name, "non-negative int"))?;
    let chars: Vec<char> = s.chars().collect();
    let uend = uend.min(chars.len());
    let ustart = ustart.min(uend);
    let result: String = chars[ustart..uend].iter().collect();
    Ok(Value::string(result))
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
pub fn dispatch_map_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Map(mut map) = receiver else {
        unreachable!("dispatch_map_method called with non-map receiver")
    };

    let n = ctx.names;

    if method == n.len || method == n.length {
        len_to_value(map.len(), "map")
    } else if method == n.is_empty {
        Ok(Value::Bool(map.is_empty()))
    } else if method == n.contains_key || method == n.contains {
        require_args("contains_key", 1, args.len())?;
        let key = args[0]
            .to_map_key()
            .map_err(|_| ori_patterns::wrong_arg_type("contains_key", "hashable value"))?;
        Ok(Value::Bool(map.contains_key(&key)))
    } else if method == n.get {
        require_args("get", 1, args.len())?;
        let key = args[0]
            .to_map_key()
            .map_err(|_| ori_patterns::wrong_arg_type("get", "hashable value"))?;
        match map.get(&key) {
            Some(v) => Ok(Value::some(v.clone())),
            None => Ok(Value::None),
        }
    } else if method == n.insert {
        require_args("insert", 2, args.len())?;
        let mut args = args;
        let value = args.swap_remove(1);
        let key = args[0]
            .to_map_key()
            .map_err(|_| ori_patterns::wrong_arg_type("insert", "hashable value"))?;
        map.make_mut().insert(key, value);
        Ok(Value::Map(map))
    } else if method == n.remove {
        require_args("remove", 1, args.len())?;
        let key = args[0]
            .to_map_key()
            .map_err(|_| ori_patterns::wrong_arg_type("remove", "hashable value"))?;
        map.make_mut().remove(&key);
        Ok(Value::Map(map))
    } else if method == n.keys {
        let keys: Vec<Value> = map.keys().map(|k| decode_map_key(k)).collect();
        Ok(Value::list(keys))
    } else if method == n.values {
        let values: Vec<Value> = map.values().cloned().collect();
        Ok(Value::list(values))
    } else if method == n.entries {
        require_args("entries", 0, args.len())?;
        let pairs: Vec<Value> = map
            .iter()
            .map(|(k, v)| Value::tuple(vec![decode_map_key(k), v.clone()]))
            .collect();
        Ok(Value::list(pairs))
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        Ok(Value::iterator(IteratorValue::from_map(&map)))
    // Eq trait - deep value equality (order-independent)
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let receiver = Value::Map(map);
        let eq = equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - order-independent XOR of entry hashes
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&Value::Map(map), ctx.interner)?))
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(Value::Map(map))
    // Debug trait - shows map structure
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&Value::Map(map))))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "map").into())
    }
}

/// Dispatch methods on set values.
pub fn dispatch_set_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Set(mut items) = receiver else {
        unreachable!("dispatch_set_method called with non-set receiver")
    };

    let n = ctx.names;

    if method == n.iter {
        require_args("iter", 0, args.len())?;
        let receiver = Value::Set(items);
        match IteratorValue::from_value(&receiver) {
            Some(iter) => Ok(Value::iterator(iter)),
            None => unreachable!("Set is always iterable"),
        }
    } else if method == n.len || method == n.length {
        require_args("len", 0, args.len())?;
        len_to_value(items.len(), "set")
    } else if method == n.is_empty {
        require_args("is_empty", 0, args.len())?;
        Ok(Value::Bool(items.is_empty()))
    } else if method == n.contains {
        require_args("contains", 1, args.len())?;
        let key = args[0]
            .to_map_key()
            .map_err(|_| ori_patterns::wrong_arg_type("contains", "hashable value"))?;
        Ok(Value::Bool(items.contains_key(&key)))
    } else if method == n.insert {
        require_args("insert", 1, args.len())?;
        let mut args = args;
        let elem = args.swap_remove(0);
        let key = elem
            .to_map_key()
            .map_err(|_| ori_patterns::wrong_arg_type("insert", "hashable value"))?;
        items.make_mut().insert(key, elem);
        Ok(Value::Set(items))
    } else if method == n.remove {
        require_args("remove", 1, args.len())?;
        let key = args[0]
            .to_map_key()
            .map_err(|_| ori_patterns::wrong_arg_type("remove", "hashable value"))?;
        items.make_mut().remove(&key);
        Ok(Value::Set(items))
    } else if method == n.union {
        require_args("union", 1, args.len())?;
        let Value::Set(ref other) = args[0] else {
            return Err(ori_patterns::wrong_arg_type("union", "Set").into());
        };
        let m = items.make_mut();
        for (k, v) in other.iter() {
            m.entry(k.clone()).or_insert_with(|| v.clone());
        }
        Ok(Value::Set(items))
    } else if method == n.intersection {
        require_args("intersection", 1, args.len())?;
        let Value::Set(ref other) = args[0] else {
            return Err(ori_patterns::wrong_arg_type("intersection", "Set").into());
        };
        items
            .make_mut()
            .retain(|k, _| other.contains_key(k.as_str()));
        Ok(Value::Set(items))
    } else if method == n.difference {
        require_args("difference", 1, args.len())?;
        let Value::Set(ref other) = args[0] else {
            return Err(ori_patterns::wrong_arg_type("difference", "Set").into());
        };
        items
            .make_mut()
            .retain(|k, _| !other.contains_key(k.as_str()));
        Ok(Value::Set(items))
    } else if method == n.to_list || method == n.into {
        require_args("to_list", 0, args.len())?;
        Ok(Value::list(items.values().cloned().collect()))
    // Clone trait - deep clone of set
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(Value::Set(items))
    // Eq trait - deep value equality
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let receiver = Value::Set(items);
        let eq = equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - order-independent XOR of element hashes
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(hash_value(&Value::Set(items), ctx.interner)?))
    // Debug trait - shows set structure
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(debug_value(&Value::Set(items))))
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "Set").into())
    }
}
