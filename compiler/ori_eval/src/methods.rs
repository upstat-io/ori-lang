//! Method dispatch implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for built-in method calls. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.

use ori_patterns::{no_such_method, wrong_arg_count, wrong_arg_type, EvalError, EvalResult, Value};

// Argument Validation Helpers

/// Validate expected argument count.
#[inline]
fn require_args(method: &str, expected: usize, actual: usize) -> Result<(), EvalError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wrong_arg_count(method, expected, actual))
    }
}

/// Extract a string argument at the given index.
#[inline]
fn require_str_arg<'a>(
    method: &str,
    args: &'a [Value],
    index: usize,
) -> Result<&'a str, EvalError> {
    match args.get(index) {
        Some(Value::Str(s)) => Ok(s.as_str()),
        _ => Err(wrong_arg_type(method, "string")),
    }
}

/// Extract an integer argument at the given index.
#[inline]
fn require_int_arg(method: &str, args: &[Value], index: usize) -> Result<i64, EvalError> {
    match args.get(index) {
        Some(Value::Int(n)) => Ok(n.raw()),
        _ => Err(wrong_arg_type(method, "int")),
    }
}

/// Convert a collection length to a Value, with overflow check.
#[inline]
fn len_to_value(len: usize, collection_type: &str) -> EvalResult {
    i64::try_from(len)
        .map(Value::int)
        .map_err(|_| EvalError::new(format!("{collection_type} too large")))
}

/// All built-in methods registered in the evaluator's direct dispatch.
///
/// Used by cross-crate consistency tests to verify the evaluator and type
/// checker agree on which methods exist. Each entry is `(type_name, method_name)`.
/// Sorted by type then method for deterministic comparison.
pub const EVAL_BUILTIN_METHODS: &[(&str, &str)] = &[
    // list
    ("list", "contains"),
    ("list", "first"),
    ("list", "is_empty"),
    ("list", "last"),
    ("list", "len"),
    // map
    ("map", "contains_key"),
    ("map", "is_empty"),
    ("map", "keys"),
    ("map", "len"),
    ("map", "values"),
    // option
    ("option", "is_none"),
    ("option", "is_some"),
    ("option", "unwrap"),
    ("option", "unwrap_or"),
    // range
    ("range", "contains"),
    ("range", "len"),
    // result
    ("result", "is_err"),
    ("result", "is_ok"),
    ("result", "unwrap"),
    // str
    ("str", "contains"),
    ("str", "ends_with"),
    ("str", "is_empty"),
    ("str", "len"),
    ("str", "starts_with"),
    ("str", "to_lowercase"),
    ("str", "to_uppercase"),
    ("str", "trim"),
];

// Direct Dispatch Function

/// Dispatch a built-in method call using direct pattern matching.
///
/// This is the preferred entry point for built-in method calls. It uses
/// enum-based dispatch which is faster than trait objects for fixed type sets.
pub fn dispatch_builtin_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    match &receiver {
        Value::List(_) => dispatch_list_method(receiver, method, args),
        Value::Str(_) => dispatch_string_method(receiver, method, args),
        Value::Map(_) => dispatch_map_method(receiver, method, args),
        Value::Range(_) => dispatch_range_method(receiver, method, args),
        Value::Some(_) | Value::None => dispatch_option_method(receiver, method, args),
        Value::Ok(_) | Value::Err(_) => dispatch_result_method(receiver, method, args),
        Value::Newtype { .. } => dispatch_newtype_method(receiver, method, args),
        _ => Err(no_such_method(method, receiver.type_name())),
    }
}

// Type-Specific Dispatch Functions

/// Dispatch methods on newtype values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_newtype_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Newtype { inner, .. } = receiver else {
        unreachable!("dispatch_newtype_method called with non-newtype value");
    };

    match method {
        "unwrap" => {
            if !args.is_empty() {
                return Err(wrong_arg_count("unwrap", 0, args.len()));
            }
            Ok((*inner).clone())
        }
        _ => Err(no_such_method(method, "newtype")),
    }
}

/// Dispatch methods on list values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_list_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::List(items) = receiver else {
        unreachable!("dispatch_list_method called with non-list receiver")
    };

    match method {
        "len" => len_to_value(items.len(), "list"),
        "is_empty" => Ok(Value::Bool(items.is_empty())),
        "first" => Ok(items.first().cloned().map_or(Value::None, Value::some)),
        "last" => Ok(items.last().cloned().map_or(Value::None, Value::some)),
        "contains" => {
            require_args("contains", 1, args.len())?;
            Ok(Value::Bool(items.contains(&args[0])))
        }
        _ => Err(no_such_method(method, "list")),
    }
}

/// Dispatch methods on string values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_string_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
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
        _ => Err(no_such_method(method, "str")),
    }
}

/// Dispatch methods on range values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_range_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
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
        _ => Err(no_such_method(method, "range")),
    }
}

/// Dispatch methods on map values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_map_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
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
            let keys: Vec<Value> = map.keys().map(|k| Value::string(k.clone())).collect();
            Ok(Value::list(keys))
        }
        "values" => {
            let values: Vec<Value> = map.values().cloned().collect();
            Ok(Value::list(values))
        }
        _ => Err(no_such_method(method, "map")),
    }
}

/// Dispatch methods on Option values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_option_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    match (method, &receiver) {
        ("unwrap" | "unwrap_or", Value::Some(v)) => Ok((**v).clone()),
        ("unwrap", Value::None) => Err(EvalError::new("called unwrap on None")),
        ("is_some", Value::Some(_)) | ("is_none", Value::None) => Ok(Value::Bool(true)),
        ("is_some", Value::None) | ("is_none", Value::Some(_)) => Ok(Value::Bool(false)),
        ("unwrap_or", Value::None) => {
            require_args("unwrap_or", 1, args.len())?;
            match args.into_iter().next() {
                Some(default) => Ok(default),
                None => unreachable!("require_args verified length is 1"),
            }
        }
        _ => Err(no_such_method(method, "Option")),
    }
}

/// Dispatch methods on Result values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_result_method(receiver: Value, method: &str, _args: Vec<Value>) -> EvalResult {
    match (method, &receiver) {
        ("unwrap", Value::Ok(v)) => Ok((**v).clone()),
        ("unwrap", Value::Err(e)) => Err(EvalError::new(format!("called unwrap on Err: {e:?}"))),
        ("is_ok", Value::Ok(_)) | ("is_err", Value::Err(_)) => Ok(Value::Bool(true)),
        ("is_ok", Value::Err(_)) | ("is_err", Value::Ok(_)) => Ok(Value::Bool(false)),
        _ => Err(no_such_method(method, "Result")),
    }
}
