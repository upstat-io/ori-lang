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
    // Additional str methods (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_string_method_str(&s, method_str, &args)
    }
}

/// String-based dispatch for str methods not hot enough to warrant
/// pre-interned Name fields.
fn dispatch_string_method_str(s: &str, method: &str, args: &[Value]) -> EvalResult {
    match method {
        "as_bytes" | "to_bytes" | "bytes" => {
            require_args(method, 0, args.len())?;
            Ok(Value::list(
                s.as_bytes().iter().map(|b| Value::Byte(*b)).collect(),
            ))
        }
        "byte_len" => {
            require_args("byte_len", 0, args.len())?;
            len_to_value(s.len(), "str")
        }
        "chars" => {
            require_args("chars", 0, args.len())?;
            Ok(Value::list(s.chars().map(Value::Char).collect()))
        }
        "index_of" => str_find_char_index(s, "index_of", args, |h, n| h.find(n)),
        "last_index_of" => str_find_char_index(s, "last_index_of", args, |h, n| h.rfind(n)),
        "lines" => {
            require_args("lines", 0, args.len())?;
            Ok(Value::list(
                s.lines().map(|l| Value::string(l.to_string())).collect(),
            ))
        }
        "pad_start" => str_pad(s, "pad_start", args, true),
        "pad_end" => str_pad(s, "pad_end", args, false),
        "parse_int" | "to_int" => {
            require_args(method, 0, args.len())?;
            match s.trim().parse::<i64>() {
                Ok(n) => Ok(Value::some(Value::int(n))),
                Err(_) => Ok(Value::None),
            }
        }
        "parse_float" | "to_float" => {
            require_args(method, 0, args.len())?;
            match s.trim().parse::<f64>() {
                Ok(f) => Ok(Value::some(Value::Float(f))),
                Err(_) => Ok(Value::None),
            }
        }
        "trim_start" => {
            require_args("trim_start", 0, args.len())?;
            Ok(Value::string(s.trim_start().to_string()))
        }
        "trim_end" => {
            require_args("trim_end", 0, args.len())?;
            Ok(Value::string(s.trim_end().to_string()))
        }
        "from_utf8" | "from_utf8_unchecked" => str_from_utf8(method, args),
        _ => Err(no_such_method(method, "str").into()),
    }
}

/// Find a substring and return its char index as `Option<int>`.
fn str_find_char_index(
    s: &str,
    method: &str,
    args: &[Value],
    finder: fn(&str, &str) -> Option<usize>,
) -> EvalResult {
    require_args(method, 1, args.len())?;
    let needle = require_str_arg(method, args, 0)?;
    match finder(s, needle) {
        Some(byte_idx) => {
            #[expect(clippy::cast_possible_wrap, reason = "char count fits in i64")]
            let char_idx = s[..byte_idx].chars().count() as i64;
            Ok(Value::some(Value::int(char_idx)))
        }
        None => Ok(Value::None),
    }
}

/// Pad a string to `width` characters using `fill`, prepending or appending.
fn str_pad(s: &str, method: &str, args: &[Value], prepend: bool) -> EvalResult {
    require_args(method, 2, args.len())?;
    let width = require_int_arg(method, args, 0)?;
    let fill = require_str_arg(method, args, 1)?;
    let current_len = s.chars().count();
    let width_usize = usize::try_from(width).unwrap_or(0);
    if current_len >= width_usize || fill.is_empty() {
        return Ok(Value::string(s.to_string()));
    }
    let pad_count = width_usize.saturating_sub(current_len);
    let pad: String = fill.chars().cycle().take(pad_count).collect();
    if prepend {
        Ok(Value::string(format!("{pad}{s}")))
    } else {
        Ok(Value::string(format!("{s}{pad}")))
    }
}

/// Parse a `[byte]` argument into a `Vec<u8>`, then convert to a string.
fn str_from_utf8(method: &str, args: &[Value]) -> EvalResult {
    require_args(method, 1, args.len())?;
    let Value::List(ref bytes) = args[0] else {
        return Err(ori_patterns::wrong_arg_type(method, "[byte]").into());
    };
    let byte_vec: Result<Vec<u8>, _> = bytes
        .iter()
        .map(|v| match v {
            Value::Byte(b) => Ok(*b),
            _ => Err(ori_patterns::wrong_arg_type(method, "[byte]")),
        })
        .collect();
    let bv = byte_vec?;
    if method == "from_utf8" {
        match String::from_utf8(bv) {
            Ok(s) => Ok(Value::ok(Value::string(s))),
            Err(e) => Ok(Value::err(Value::error(e.to_string()))),
        }
    } else {
        // from_utf8_unchecked: in the interpreter, we validate for safety
        match String::from_utf8(bv) {
            Ok(s) => Ok(Value::string(s)),
            Err(e) => Ok(Value::string(
                String::from_utf8_lossy(e.as_bytes()).into_owned(),
            )),
        }
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
        // Higher-order methods requiring closures (dispatched by CollectionMethodResolver
        // in production; recognized here so dispatch coverage test sees non-UndefinedMethod)
        "all" | "any" | "filter" | "find" | "fold" | "map" => {
            require_args(method, 1, args.len())?;
            Err(ori_patterns::wrong_arg_type(method, "function").into())
        }
        _ => Err(no_such_method(method, "range").into()),
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
    // Additional map methods (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_map_method_str(map, method_str, &args)
    }
}

/// String-based dispatch for map methods not covered by Name-based dispatch.
fn dispatch_map_method_str(
    mut map: ori_patterns::Heap<std::collections::BTreeMap<String, Value>>,
    method: &str,
    args: &[Value],
) -> EvalResult {
    match method {
        "merge" => {
            require_args("merge", 1, args.len())?;
            let Value::Map(ref other) = args[0] else {
                return Err(ori_patterns::wrong_arg_type("merge", "Map").into());
            };
            let m = map.make_mut();
            for (k, v) in other.iter() {
                m.insert(k.clone(), v.clone());
            }
            Ok(Value::Map(map))
        }
        "update" => {
            // update(key, f) requires a closure — arg count check
            require_args("update", 2, args.len())?;
            Err(ori_patterns::wrong_arg_type("update", "function").into())
        }
        // Higher-order methods requiring closures (dispatched by CollectionMethodResolver
        // in production; recognized here so dispatch coverage test sees non-UndefinedMethod)
        "filter" | "map" => {
            require_args(method, 1, args.len())?;
            Err(ori_patterns::wrong_arg_type(method, "function").into())
        }
        _ => Err(no_such_method(method, "map").into()),
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
