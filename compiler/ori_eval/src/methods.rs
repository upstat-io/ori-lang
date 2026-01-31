//! Method dispatch implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for built-in method calls. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.

use ori_patterns::{
    division_by_zero, integer_overflow, modulo_by_zero, no_such_method, wrong_arg_count,
    wrong_arg_type, EvalError, EvalResult, Heap, ScalarInt, Value,
};

// =============================================================================
// Associated Function Dispatch
// =============================================================================

/// Dispatch an associated function call (static method without receiver instance).
///
/// Associated functions are called on type names rather than instances,
/// e.g., `Duration.from_seconds(s: 10)`.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature with other dispatch functions"
)]
pub fn dispatch_associated_function(type_name: &str, method: &str, args: Vec<Value>) -> EvalResult {
    match type_name {
        "Duration" => dispatch_duration_associated(method, &args),
        "Size" => dispatch_size_associated(method, &args),
        _ => Err(no_such_method(method, type_name)),
    }
}

/// Dispatch Duration associated functions (factory methods).
fn dispatch_duration_associated(method: &str, args: &[Value]) -> EvalResult {
    match method {
        "from_nanoseconds" => {
            require_args("from_nanoseconds", 1, args.len())?;
            let ns = require_int_arg("from_nanoseconds", args, 0)?;
            Ok(Value::Duration(ns))
        }
        "from_microseconds" => {
            require_args("from_microseconds", 1, args.len())?;
            let us = require_int_arg("from_microseconds", args, 0)?;
            us.checked_mul(1_000)
                .map(Value::Duration)
                .ok_or_else(|| EvalError::new("duration overflow"))
        }
        "from_milliseconds" => {
            require_args("from_milliseconds", 1, args.len())?;
            let ms = require_int_arg("from_milliseconds", args, 0)?;
            ms.checked_mul(1_000_000)
                .map(Value::Duration)
                .ok_or_else(|| EvalError::new("duration overflow"))
        }
        "from_seconds" => {
            require_args("from_seconds", 1, args.len())?;
            let s = require_int_arg("from_seconds", args, 0)?;
            s.checked_mul(1_000_000_000)
                .map(Value::Duration)
                .ok_or_else(|| EvalError::new("duration overflow"))
        }
        "from_minutes" => {
            require_args("from_minutes", 1, args.len())?;
            let m = require_int_arg("from_minutes", args, 0)?;
            m.checked_mul(60_000_000_000)
                .map(Value::Duration)
                .ok_or_else(|| EvalError::new("duration overflow"))
        }
        "from_hours" => {
            require_args("from_hours", 1, args.len())?;
            let h = require_int_arg("from_hours", args, 0)?;
            h.checked_mul(3_600_000_000_000)
                .map(Value::Duration)
                .ok_or_else(|| EvalError::new("duration overflow"))
        }
        _ => Err(no_such_method(method, "Duration")),
    }
}

/// Dispatch Size associated functions (factory methods).
fn dispatch_size_associated(method: &str, args: &[Value]) -> EvalResult {
    match method {
        "from_bytes" => {
            require_args("from_bytes", 1, args.len())?;
            let b = require_int_arg("from_bytes", args, 0)?;
            if b < 0 {
                return Err(EvalError::new("Size cannot be negative"));
            }
            #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
            Ok(Value::Size(b as u64))
        }
        "from_kilobytes" => {
            require_args("from_kilobytes", 1, args.len())?;
            let kb = require_int_arg("from_kilobytes", args, 0)?;
            if kb < 0 {
                return Err(EvalError::new("Size cannot be negative"));
            }
            #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
            (kb as u64)
                .checked_mul(1024)
                .map(Value::Size)
                .ok_or_else(|| EvalError::new("size overflow"))
        }
        "from_megabytes" => {
            require_args("from_megabytes", 1, args.len())?;
            let mb = require_int_arg("from_megabytes", args, 0)?;
            if mb < 0 {
                return Err(EvalError::new("Size cannot be negative"));
            }
            #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
            (mb as u64)
                .checked_mul(1024 * 1024)
                .map(Value::Size)
                .ok_or_else(|| EvalError::new("size overflow"))
        }
        "from_gigabytes" => {
            require_args("from_gigabytes", 1, args.len())?;
            let gb = require_int_arg("from_gigabytes", args, 0)?;
            if gb < 0 {
                return Err(EvalError::new("Size cannot be negative"));
            }
            #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
            (gb as u64)
                .checked_mul(1024 * 1024 * 1024)
                .map(Value::Size)
                .ok_or_else(|| EvalError::new("size overflow"))
        }
        "from_terabytes" => {
            require_args("from_terabytes", 1, args.len())?;
            let tb = require_int_arg("from_terabytes", args, 0)?;
            if tb < 0 {
                return Err(EvalError::new("Size cannot be negative"));
            }
            #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
            (tb as u64)
                .checked_mul(1024 * 1024 * 1024 * 1024)
                .map(Value::Size)
                .ok_or_else(|| EvalError::new("size overflow"))
        }
        _ => Err(no_such_method(method, "Size")),
    }
}

// =============================================================================
// Instance Method Dispatch
// =============================================================================

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

/// Extract a `ScalarInt` argument at the given index.
#[inline]
fn require_scalar_int_arg(
    method: &str,
    args: &[Value],
    index: usize,
) -> Result<ScalarInt, EvalError> {
    match args.get(index) {
        Some(Value::Int(n)) => Ok(*n),
        _ => Err(wrong_arg_type(method, "int")),
    }
}

/// Extract a float argument at the given index.
#[inline]
fn require_float_arg(method: &str, args: &[Value], index: usize) -> Result<f64, EvalError> {
    match args.get(index) {
        Some(Value::Float(f)) => Ok(*f),
        _ => Err(wrong_arg_type(method, "float")),
    }
}

/// Extract a list argument at the given index.
#[inline]
fn require_list_arg<'a>(
    method: &str,
    args: &'a [Value],
    index: usize,
) -> Result<&'a Heap<Vec<Value>>, EvalError> {
    match args.get(index) {
        Some(Value::List(l)) => Ok(l),
        _ => Err(wrong_arg_type(method, "list")),
    }
}

/// Extract a Duration argument at the given index.
#[inline]
fn require_duration_arg(method: &str, args: &[Value], index: usize) -> Result<i64, EvalError> {
    match args.get(index) {
        Some(Value::Duration(d)) => Ok(*d),
        _ => Err(wrong_arg_type(method, "Duration")),
    }
}

/// Extract a Size argument at the given index.
#[inline]
fn require_size_arg(method: &str, args: &[Value], index: usize) -> Result<u64, EvalError> {
    match args.get(index) {
        Some(Value::Size(s)) => Ok(*s),
        _ => Err(wrong_arg_type(method, "Size")),
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
    // bool
    ("bool", "not"),
    // duration
    ("duration", "add"),
    ("duration", "div"),
    ("duration", "hours"),
    ("duration", "microseconds"),
    ("duration", "milliseconds"),
    ("duration", "minutes"),
    ("duration", "mul"),
    ("duration", "nanoseconds"),
    ("duration", "neg"),
    ("duration", "rem"),
    ("duration", "seconds"),
    ("duration", "sub"),
    // float
    ("float", "add"),
    ("float", "div"),
    ("float", "mul"),
    ("float", "neg"),
    ("float", "sub"),
    // int
    ("int", "add"),
    ("int", "bit_and"),
    ("int", "bit_not"),
    ("int", "bit_or"),
    ("int", "bit_xor"),
    ("int", "div"),
    ("int", "floor_div"),
    ("int", "mul"),
    ("int", "neg"),
    ("int", "rem"),
    ("int", "shl"),
    ("int", "shr"),
    ("int", "sub"),
    // list
    ("list", "add"),
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
    // size
    ("size", "add"),
    ("size", "bytes"),
    ("size", "div"),
    ("size", "gigabytes"),
    ("size", "kilobytes"),
    ("size", "megabytes"),
    ("size", "mul"),
    ("size", "rem"),
    ("size", "sub"),
    ("size", "terabytes"),
    // str
    ("str", "add"),
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
///
/// Handles operator trait methods (add, sub, mul, etc.) uniformly for all types.
pub fn dispatch_builtin_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    match &receiver {
        Value::Int(_) => dispatch_int_method(receiver, method, args),
        Value::Float(_) => dispatch_float_method(receiver, method, args),
        Value::Bool(_) => dispatch_bool_method(receiver, method, args),
        Value::List(_) => dispatch_list_method(receiver, method, args),
        Value::Str(_) => dispatch_string_method(receiver, method, args),
        Value::Map(_) => dispatch_map_method(receiver, method, args),
        Value::Range(_) => dispatch_range_method(receiver, method, args),
        Value::Some(_) | Value::None => dispatch_option_method(receiver, method, args),
        Value::Ok(_) | Value::Err(_) => dispatch_result_method(receiver, method, args),
        Value::Newtype { .. } => dispatch_newtype_method(receiver, method, args),
        Value::Duration(_) => dispatch_duration_method(receiver, method, args),
        Value::Size(_) => dispatch_size_method(receiver, method, args),
        _ => Err(no_such_method(method, receiver.type_name())),
    }
}

// Type-Specific Dispatch Functions

/// Dispatch operator methods on integer values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_int_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Int(a) = receiver else {
        unreachable!("dispatch_int_method called with non-int receiver")
    };

    match method {
        // Binary arithmetic operators
        "add" => {
            require_args("add", 1, args.len())?;
            let b = require_scalar_int_arg("add", &args, 0)?;
            a.checked_add(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("addition"))
        }
        "sub" => {
            require_args("sub", 1, args.len())?;
            let b = require_scalar_int_arg("sub", &args, 0)?;
            a.checked_sub(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("subtraction"))
        }
        "mul" => {
            require_args("mul", 1, args.len())?;
            let b = require_scalar_int_arg("mul", &args, 0)?;
            a.checked_mul(b)
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("multiplication"))
        }
        "div" => {
            require_args("div", 1, args.len())?;
            let b = require_scalar_int_arg("div", &args, 0)?;
            if b.is_zero() {
                Err(division_by_zero())
            } else {
                a.checked_div(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("division"))
            }
        }
        "floor_div" => {
            require_args("floor_div", 1, args.len())?;
            let b = require_scalar_int_arg("floor_div", &args, 0)?;
            if b.is_zero() {
                Err(division_by_zero())
            } else {
                a.checked_floor_div(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("floor division"))
            }
        }
        "rem" => {
            require_args("rem", 1, args.len())?;
            let b = require_scalar_int_arg("rem", &args, 0)?;
            if b.is_zero() {
                Err(modulo_by_zero())
            } else {
                a.checked_rem(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("remainder"))
            }
        }
        // Unary operators
        "neg" => {
            require_args("neg", 0, args.len())?;
            a.checked_neg()
                .map(Value::Int)
                .ok_or_else(|| integer_overflow("negation"))
        }
        // Bitwise operators
        "bit_and" => {
            require_args("bit_and", 1, args.len())?;
            let b = require_scalar_int_arg("bit_and", &args, 0)?;
            Ok(Value::Int(a & b))
        }
        "bit_or" => {
            require_args("bit_or", 1, args.len())?;
            let b = require_scalar_int_arg("bit_or", &args, 0)?;
            Ok(Value::Int(a | b))
        }
        "bit_xor" => {
            require_args("bit_xor", 1, args.len())?;
            let b = require_scalar_int_arg("bit_xor", &args, 0)?;
            Ok(Value::Int(a ^ b))
        }
        "bit_not" => {
            require_args("bit_not", 0, args.len())?;
            Ok(Value::Int(!a))
        }
        "shl" => {
            require_args("shl", 1, args.len())?;
            let b = require_scalar_int_arg("shl", &args, 0)?;
            a.checked_shl(b).map(Value::Int).ok_or_else(|| {
                EvalError::new(format!("shift amount {} out of range (0-63)", b.raw()))
            })
        }
        "shr" => {
            require_args("shr", 1, args.len())?;
            let b = require_scalar_int_arg("shr", &args, 0)?;
            a.checked_shr(b).map(Value::Int).ok_or_else(|| {
                EvalError::new(format!("shift amount {} out of range (0-63)", b.raw()))
            })
        }
        _ => Err(no_such_method(method, "int")),
    }
}

/// Dispatch operator methods on float values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_float_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Float(a) = receiver else {
        unreachable!("dispatch_float_method called with non-float receiver")
    };

    match method {
        "add" => {
            require_args("add", 1, args.len())?;
            let b = require_float_arg("add", &args, 0)?;
            Ok(Value::Float(a + b))
        }
        "sub" => {
            require_args("sub", 1, args.len())?;
            let b = require_float_arg("sub", &args, 0)?;
            Ok(Value::Float(a - b))
        }
        "mul" => {
            require_args("mul", 1, args.len())?;
            let b = require_float_arg("mul", &args, 0)?;
            Ok(Value::Float(a * b))
        }
        "div" => {
            require_args("div", 1, args.len())?;
            let b = require_float_arg("div", &args, 0)?;
            Ok(Value::Float(a / b))
        }
        "neg" => {
            require_args("neg", 0, args.len())?;
            Ok(Value::Float(-a))
        }
        _ => Err(no_such_method(method, "float")),
    }
}

/// Dispatch operator methods on bool values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_bool_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Bool(a) = receiver else {
        unreachable!("dispatch_bool_method called with non-bool receiver")
    };

    match method {
        "not" => {
            require_args("not", 0, args.len())?;
            Ok(Value::Bool(!a))
        }
        _ => Err(no_such_method(method, "bool")),
    }
}

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
        "add" => {
            require_args("add", 1, args.len())?;
            let other = require_list_arg("add", &args, 0)?;
            let mut result = (*items).clone();
            result.extend_from_slice(other);
            Ok(Value::list(result))
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
        "add" => {
            require_args("add", 1, args.len())?;
            let other = require_str_arg("add", &args, 0)?;
            let result = format!("{}{}", &**s, other);
            Ok(Value::string(result))
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

/// Dispatch methods on Duration values.
/// Duration is stored as i64 nanoseconds.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_duration_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Duration(ns) = receiver else {
        unreachable!("dispatch_duration_method called with non-duration receiver")
    };

    match method {
        "nanoseconds" => Ok(Value::int(ns)),
        "microseconds" => Ok(Value::int(ns / 1_000)),
        "milliseconds" => Ok(Value::int(ns / 1_000_000)),
        "seconds" => Ok(Value::int(ns / 1_000_000_000)),
        "minutes" => Ok(Value::int(ns / (60 * 1_000_000_000))),
        "hours" => Ok(Value::int(ns / (60 * 60 * 1_000_000_000))),
        // Operator methods
        "add" => {
            require_args("add", 1, args.len())?;
            let other = require_duration_arg("add", &args, 0)?;
            ns.checked_add(other)
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration addition"))
        }
        "sub" => {
            require_args("sub", 1, args.len())?;
            let other = require_duration_arg("sub", &args, 0)?;
            ns.checked_sub(other)
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration subtraction"))
        }
        "mul" => {
            require_args("mul", 1, args.len())?;
            let scalar = require_int_arg("mul", &args, 0)?;
            ns.checked_mul(scalar)
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration multiplication"))
        }
        "div" => {
            require_args("div", 1, args.len())?;
            let scalar = require_int_arg("div", &args, 0)?;
            if scalar == 0 {
                Err(division_by_zero())
            } else {
                ns.checked_div(scalar)
                    .map(Value::Duration)
                    .ok_or_else(|| integer_overflow("duration division"))
            }
        }
        "rem" => {
            require_args("rem", 1, args.len())?;
            let other = require_duration_arg("rem", &args, 0)?;
            if other == 0 {
                Err(modulo_by_zero())
            } else {
                ns.checked_rem(other)
                    .map(Value::Duration)
                    .ok_or_else(|| integer_overflow("duration modulo"))
            }
        }
        "neg" => {
            require_args("neg", 0, args.len())?;
            ns.checked_neg()
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration negation"))
        }
        _ => Err(no_such_method(method, "Duration")),
    }
}

/// Dispatch methods on Size values.
/// Size is stored as u64 bytes.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_size_method(receiver: Value, method: &str, args: Vec<Value>) -> EvalResult {
    let Value::Size(bytes) = receiver else {
        unreachable!("dispatch_size_method called with non-size receiver")
    };

    // Convert u64 to i64 safely (truncating division results fit in i64)
    let to_int = |v: u64| -> EvalResult {
        i64::try_from(v)
            .map(Value::int)
            .map_err(|_| EvalError::new("size value too large for int"))
    };

    match method {
        "bytes" => to_int(bytes),
        "kilobytes" => to_int(bytes / 1024),
        "megabytes" => to_int(bytes / (1024 * 1024)),
        "gigabytes" => to_int(bytes / (1024 * 1024 * 1024)),
        "terabytes" => to_int(bytes / (1024 * 1024 * 1024 * 1024)),
        // Operator methods
        "add" => {
            require_args("add", 1, args.len())?;
            let other = require_size_arg("add", &args, 0)?;
            bytes
                .checked_add(other)
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size addition"))
        }
        "sub" => {
            require_args("sub", 1, args.len())?;
            let other = require_size_arg("sub", &args, 0)?;
            bytes
                .checked_sub(other)
                .map(Value::Size)
                .ok_or_else(|| EvalError::new("size subtraction would result in negative value"))
        }
        "mul" => {
            require_args("mul", 1, args.len())?;
            let scalar = require_int_arg("mul", &args, 0)?;
            if scalar < 0 {
                return Err(EvalError::new("cannot multiply Size by negative integer"));
            }
            #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
            bytes
                .checked_mul(scalar as u64)
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size multiplication"))
        }
        "div" => {
            require_args("div", 1, args.len())?;
            let scalar = require_int_arg("div", &args, 0)?;
            if scalar == 0 {
                return Err(division_by_zero());
            }
            if scalar < 0 {
                return Err(EvalError::new("cannot divide Size by negative integer"));
            }
            #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
            bytes
                .checked_div(scalar as u64)
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size division"))
        }
        "rem" => {
            require_args("rem", 1, args.len())?;
            let other = require_size_arg("rem", &args, 0)?;
            if other == 0 {
                Err(modulo_by_zero())
            } else {
                bytes
                    .checked_rem(other)
                    .map(Value::Size)
                    .ok_or_else(|| integer_overflow("size modulo"))
            }
        }
        _ => Err(no_such_method(method, "Size")),
    }
}
