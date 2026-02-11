//! Method dispatch for collection types (list, str, map, range).

use ori_ir::StringInterner;
use ori_patterns::{no_such_method, EvalResult, Value};

use super::compare::{compare_lists, ordering_to_value};
use super::helpers::{
    len_to_value, require_args, require_int_arg, require_list_arg, require_str_arg,
};

/// Dispatch methods on list values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_list_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::List(items) = receiver else {
        unreachable!("dispatch_list_method called with non-list receiver")
    };

    // Note: Value clones in this function are cheap - Value uses Arc for heap types.
    match method {
        "len" => len_to_value(items.len(), "list"),
        "is_empty" => Ok(Value::Bool(items.is_empty())),
        "first" => Ok(items.first().cloned().map_or(Value::None, Value::some)),
        "last" => Ok(items.last().cloned().map_or(Value::None, Value::some)),
        "contains" => {
            require_args("contains", 1, args.len())?;
            Ok(Value::Bool(items.contains(&args[0])))
        }
        "add" | "concat" => {
            require_args(method, 1, args.len())?;
            let other = require_list_arg(method, &args, 0)?;
            let mut result = (*items).clone();
            result.extend_from_slice(other);
            Ok(Value::list(result))
        }
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_list_arg("compare", &args, 0)?;
            let ord = compare_lists(&items, other, interner)?;
            Ok(ordering_to_value(ord))
        }
        // Clone trait - deep clone of list
        "clone" => {
            require_args("clone", 0, args.len())?;
            Ok(Value::list((*items).clone()))
        }
        // Debug trait - shows list structure
        "debug" => {
            require_args("debug", 0, args.len())?;
            let parts: Vec<String> = items.iter().map(|v| format!("{v:?}")).collect();
            Ok(Value::string(format!("[{}]", parts.join(", "))))
        }
        _ => Err(no_such_method(method, "list").into()),
    }
}

/// Dispatch methods on string values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_string_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Str(s) = receiver else {
        unreachable!("dispatch_string_method called with non-string receiver")
    };

    match method {
        "len" => len_to_value(s.len(), "string"),
        "is_empty" => Ok(Value::Bool(s.is_empty())),
        "to_uppercase" => Ok(Value::string(s.to_uppercase())),
        "to_lowercase" => Ok(Value::string(s.to_lowercase())),
        "trim" => Ok(Value::string(s.trim().to_string())),
        "contains" => {
            require_args("contains", 1, args.len())?;
            let needle = require_str_arg("contains", &args, 0)?;
            Ok(Value::Bool(s.contains(needle)))
        }
        "starts_with" => {
            require_args("starts_with", 1, args.len())?;
            let prefix = require_str_arg("starts_with", &args, 0)?;
            Ok(Value::Bool(s.starts_with(prefix)))
        }
        "ends_with" => {
            require_args("ends_with", 1, args.len())?;
            let suffix = require_str_arg("ends_with", &args, 0)?;
            Ok(Value::Bool(s.ends_with(suffix)))
        }
        "add" | "concat" => {
            require_args(method, 1, args.len())?;
            let other = require_str_arg(method, &args, 0)?;
            let result = format!("{}{}", &**s, other);
            Ok(Value::string(result))
        }
        // Comparable trait - lexicographic (Unicode codepoint)
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_str_arg("compare", &args, 0)?;
            Ok(ordering_to_value((**s).cmp(other)))
        }
        // Eq trait
        "equals" => {
            require_args("equals", 1, args.len())?;
            let other = require_str_arg("equals", &args, 0)?;
            Ok(Value::Bool(&**s == other))
        }
        // Clone trait
        "clone" => {
            require_args("clone", 0, args.len())?;
            Ok(Value::string(s.to_string()))
        }
        // Printable trait - returns the string itself
        "to_str" => {
            require_args("to_str", 0, args.len())?;
            Ok(Value::string(s.to_string()))
        }
        // Debug trait - shows escaped string with quotes
        "debug" => {
            require_args("debug", 0, args.len())?;
            Ok(Value::string(format!("\"{s}\"")))
        }
        // Hashable trait
        "hash" => {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            require_args("hash", 0, args.len())?;
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            Ok(Value::int(hasher.finish().cast_signed()))
        }
        _ => Err(no_such_method(method, "str").into()),
    }
}

/// Dispatch methods on range values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_range_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Range(r) = receiver else {
        unreachable!("dispatch_range_method called with non-range receiver")
    };

    match method {
        "len" => len_to_value(r.len(), "range"),
        "contains" => {
            require_args("contains", 1, args.len())?;
            let n = require_int_arg("contains", &args, 0)?;
            Ok(Value::Bool(r.contains(n)))
        }
        _ => Err(no_such_method(method, "range").into()),
    }
}

/// Dispatch methods on map values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_map_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Map(map) = receiver else {
        unreachable!("dispatch_map_method called with non-map receiver")
    };

    match method {
        "len" => len_to_value(map.len(), "map"),
        "is_empty" => Ok(Value::Bool(map.is_empty())),
        "contains_key" => {
            require_args("contains_key", 1, args.len())?;
            let key = require_str_arg("contains_key", &args, 0)?;
            Ok(Value::Bool(map.contains_key(key)))
        }
        "keys" => {
            // Clone Cow<str> keys to create Value::Str - required for owned return
            let keys: Vec<Value> = map.keys().map(|k| Value::string(k.clone())).collect();
            Ok(Value::list(keys))
        }
        "values" => {
            // Clone values for return list. Cheap: Value uses Arc for heap types.
            let values: Vec<Value> = map.values().cloned().collect();
            Ok(Value::list(values))
        }
        _ => Err(no_such_method(method, "map").into()),
    }
}
