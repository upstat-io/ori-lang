//! Method dispatch implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for built-in method calls. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.

use ori_ir::builtin_constants::{duration, size};
use ori_ir::StringInterner;
use ori_patterns::{
    division_by_zero, integer_overflow, modulo_by_zero, no_such_method, wrong_arg_count,
    wrong_arg_type, EvalError, EvalResult, Heap, OrderingValue, ScalarInt, Value,
};

// Factory Helper Functions

/// Create a Duration value from an integer with a multiplier.
///
/// Reduces repetition in Duration factory methods (`from_microseconds`, `from_seconds`, etc.).
#[inline]
fn duration_from_int(method: &str, args: &[Value], multiplier: i64) -> EvalResult {
    require_args(method, 1, args.len())?;
    let val = require_int_arg(method, args, 0)?;
    val.checked_mul(multiplier)
        .map(Value::Duration)
        .ok_or_else(|| EvalError::new("duration overflow"))
}

/// Create a Size value from an integer with a multiplier.
///
/// Reduces repetition in Size factory methods (`from_kilobytes`, `from_megabytes`, etc.).
/// Handles the negative value check that Size requires.
#[inline]
fn size_from_int(method: &str, args: &[Value], multiplier: u64) -> EvalResult {
    require_args(method, 1, args.len())?;
    let val = require_int_arg(method, args, 0)?;
    if val < 0 {
        return Err(EvalError::new("Size cannot be negative"));
    }
    #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
    (val as u64)
        .checked_mul(multiplier)
        .map(Value::Size)
        .ok_or_else(|| EvalError::new("size overflow"))
}

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
        "from_nanoseconds" => duration_from_int(method, args, 1),
        "from_microseconds" => duration_from_int(method, args, duration::NS_PER_US),
        "from_milliseconds" => duration_from_int(method, args, duration::NS_PER_MS),
        "from_seconds" => duration_from_int(method, args, duration::NS_PER_S),
        "from_minutes" => duration_from_int(method, args, duration::NS_PER_M),
        "from_hours" => duration_from_int(method, args, duration::NS_PER_H),
        "default" => {
            require_args("default", 0, args.len())?;
            Ok(Value::Duration(0)) // 0ns is the default Duration
        }
        _ => Err(no_such_method(method, "Duration")),
    }
}

/// Dispatch Size associated functions (factory methods).
fn dispatch_size_associated(method: &str, args: &[Value]) -> EvalResult {
    match method {
        "from_bytes" => size_from_int(method, args, 1),
        "from_kilobytes" => size_from_int(method, args, size::BYTES_PER_KB),
        "from_megabytes" => size_from_int(method, args, size::BYTES_PER_MB),
        "from_gigabytes" => size_from_int(method, args, size::BYTES_PER_GB),
        "from_terabytes" => size_from_int(method, args, size::BYTES_PER_TB),
        "default" => {
            require_args("default", 0, args.len())?;
            Ok(Value::Size(0)) // 0b is the default Size
        }
        _ => Err(no_such_method(method, "Size")),
    }
}

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

/// Extract a bool argument at the given index.
#[inline]
fn require_bool_arg(method: &str, args: &[Value], index: usize) -> Result<bool, EvalError> {
    match args.get(index) {
        Some(Value::Bool(b)) => Ok(*b),
        _ => Err(wrong_arg_type(method, "bool")),
    }
}

/// Extract a char argument at the given index.
#[inline]
fn require_char_arg(method: &str, args: &[Value], index: usize) -> Result<char, EvalError> {
    match args.get(index) {
        Some(Value::Char(c)) => Ok(*c),
        _ => Err(wrong_arg_type(method, "char")),
    }
}

/// Extract a byte argument at the given index.
#[inline]
fn require_byte_arg(method: &str, args: &[Value], index: usize) -> Result<u8, EvalError> {
    match args.get(index) {
        Some(Value::Byte(b)) => Ok(*b),
        _ => Err(wrong_arg_type(method, "byte")),
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
pub fn dispatch_builtin_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    match &receiver {
        Value::Int(_) => dispatch_int_method(receiver, method, args, interner),
        Value::Float(_) => dispatch_float_method(receiver, method, args, interner),
        Value::Bool(_) => dispatch_bool_method(receiver, method, args, interner),
        Value::Char(_) => dispatch_char_method(receiver, method, args, interner),
        Value::Byte(_) => dispatch_byte_method(receiver, method, args, interner),
        Value::List(_) => dispatch_list_method(receiver, method, args, interner),
        Value::Str(_) => dispatch_string_method(receiver, method, args, interner),
        Value::Map(_) => dispatch_map_method(receiver, method, args),
        Value::Range(_) => dispatch_range_method(receiver, method, args),
        Value::Some(_) | Value::None => dispatch_option_method(receiver, method, args, interner),
        Value::Ok(_) | Value::Err(_) => dispatch_result_method(receiver, method, args, interner),
        Value::Newtype { .. } => dispatch_newtype_method(receiver, method, args),
        Value::Duration(_) => dispatch_duration_method(receiver, method, args, interner),
        Value::Size(_) => dispatch_size_method(receiver, method, args, interner),
        Value::Ordering(_) => dispatch_ordering_method(receiver, method, args, interner),
        _ => Err(no_such_method(method, receiver.type_name())),
    }
}

// Type-Specific Dispatch Functions

/// Dispatch operator methods on integer values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_int_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
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
        // Comparable trait
        "compare" => {
            require_args("compare", 1, args.len())?;
            let b = require_scalar_int_arg("compare", &args, 0)?;
            Ok(ordering_to_value(a.cmp(&b), interner))
        }
        _ => Err(no_such_method(method, "int")),
    }
}

/// Dispatch operator methods on float values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_float_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
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
        // Comparable trait - IEEE 754 total ordering
        "compare" => {
            require_args("compare", 1, args.len())?;
            let b = require_float_arg("compare", &args, 0)?;
            // Use total_cmp for IEEE 754 total ordering (handles NaN consistently)
            Ok(ordering_to_value(a.total_cmp(&b), interner))
        }
        _ => Err(no_such_method(method, "float")),
    }
}

/// Dispatch operator methods on bool values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_bool_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::Bool(a) = receiver else {
        unreachable!("dispatch_bool_method called with non-bool receiver")
    };

    match method {
        "not" => {
            require_args("not", 0, args.len())?;
            Ok(Value::Bool(!a))
        }
        // Comparable trait - false < true
        "compare" => {
            require_args("compare", 1, args.len())?;
            let b = require_bool_arg("compare", &args, 0)?;
            Ok(ordering_to_value(a.cmp(&b), interner))
        }
        _ => Err(no_such_method(method, "bool")),
    }
}

/// Dispatch methods on char values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_char_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::Char(c) = receiver else {
        unreachable!("dispatch_char_method called with non-char receiver")
    };

    match method {
        // Comparable trait - Unicode codepoint order
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_char_arg("compare", &args, 0)?;
            Ok(ordering_to_value(c.cmp(&other), interner))
        }
        _ => Err(no_such_method(method, "char")),
    }
}

/// Dispatch methods on byte values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_byte_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let Value::Byte(b) = receiver else {
        unreachable!("dispatch_byte_method called with non-byte receiver")
    };

    match method {
        // Comparable trait - numeric order
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_byte_arg("compare", &args, 0)?;
            Ok(ordering_to_value(b.cmp(&other), interner))
        }
        _ => Err(no_such_method(method, "byte")),
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
fn dispatch_list_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
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
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_list_arg("compare", &args, 0)?;
            let ord = compare_lists(&items, other, interner)?;
            Ok(ordering_to_value(ord, interner))
        }
        _ => Err(no_such_method(method, "list")),
    }
}

/// Dispatch methods on string values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_string_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
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
        // Comparable trait - lexicographic (Unicode codepoint)
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_str_arg("compare", &args, 0)?;
            Ok(ordering_to_value(s.as_str().cmp(other), interner))
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
fn dispatch_option_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
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
        // Comparable trait - None < Some(_)
        ("compare", _) => {
            require_args("compare", 1, args.len())?;
            let ord = compare_option_values(&receiver, &args[0], interner)?;
            Ok(ordering_to_value(ord, interner))
        }
        _ => Err(no_such_method(method, "Option")),
    }
}

/// Dispatch methods on Result values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_result_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    match method {
        "unwrap" => match &receiver {
            Value::Ok(v) => Ok((**v).clone()),
            Value::Err(e) => Err(EvalError::new(format!("called unwrap on Err: {e:?}"))),
            _ => unreachable!(),
        },
        "is_ok" => Ok(Value::Bool(matches!(&receiver, Value::Ok(_)))),
        "is_err" => Ok(Value::Bool(matches!(&receiver, Value::Err(_)))),
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = &args[0];
            let ord = compare_result_values(&receiver, other, interner)?;
            Ok(ordering_to_value(ord, interner))
        }
        _ => Err(no_such_method(method, "Result")),
    }
}

/// Dispatch methods on Duration values.
/// Duration is stored as i64 nanoseconds.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_duration_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

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
        "sub" | "subtract" => {
            require_args(method, 1, args.len())?;
            let other = require_duration_arg(method, &args, 0)?;
            ns.checked_sub(other)
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration subtraction"))
        }
        "mul" | "multiply" => {
            require_args(method, 1, args.len())?;
            let scalar = require_int_arg(method, &args, 0)?;
            ns.checked_mul(scalar)
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration multiplication"))
        }
        "div" | "divide" => {
            require_args(method, 1, args.len())?;
            let scalar = require_int_arg(method, &args, 0)?;
            if scalar == 0 {
                Err(division_by_zero())
            } else {
                ns.checked_div(scalar)
                    .map(Value::Duration)
                    .ok_or_else(|| integer_overflow("duration division"))
            }
        }
        "rem" | "remainder" => {
            require_args(method, 1, args.len())?;
            let other = require_duration_arg(method, &args, 0)?;
            if other == 0 {
                Err(modulo_by_zero())
            } else {
                ns.checked_rem(other)
                    .map(Value::Duration)
                    .ok_or_else(|| integer_overflow("duration modulo"))
            }
        }
        "neg" | "negate" => {
            require_args(method, 0, args.len())?;
            ns.checked_neg()
                .map(Value::Duration)
                .ok_or_else(|| integer_overflow("duration negation"))
        }
        // Trait methods
        "hash" => {
            require_args("hash", 0, args.len())?;
            let mut hasher = DefaultHasher::new();
            "Duration".hash(&mut hasher);
            ns.hash(&mut hasher);
            #[expect(
                clippy::cast_possible_wrap,
                reason = "Hash values are opaque identifiers"
            )]
            Ok(Value::int(hasher.finish() as i64))
        }
        "clone" => {
            require_args("clone", 0, args.len())?;
            Ok(Value::Duration(ns))
        }
        "to_str" => {
            require_args("to_str", 0, args.len())?;
            Ok(Value::string(format_duration(ns)))
        }
        "equals" => {
            require_args("equals", 1, args.len())?;
            let other = require_duration_arg("equals", &args, 0)?;
            Ok(Value::Bool(ns == other))
        }
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_duration_arg("compare", &args, 0)?;
            Ok(ordering_to_value(ns.cmp(&other), interner))
        }
        _ => Err(no_such_method(method, "Duration")),
    }
}

/// Format a Duration (nanoseconds) as a human-readable string.
fn format_duration(ns: i64) -> String {
    use duration::unsigned as dur;

    let abs_ns = ns.unsigned_abs();
    let sign = if ns < 0 { "-" } else { "" };

    if abs_ns == 0 {
        return "0ns".to_string();
    }

    // Use the largest unit that gives a whole number
    if abs_ns.is_multiple_of(dur::NS_PER_H) {
        let hours = abs_ns / dur::NS_PER_H;
        format!("{sign}{hours}h")
    } else if abs_ns.is_multiple_of(dur::NS_PER_M) {
        let minutes = abs_ns / dur::NS_PER_M;
        format!("{sign}{minutes}m")
    } else if abs_ns.is_multiple_of(dur::NS_PER_S) {
        let seconds = abs_ns / dur::NS_PER_S;
        format!("{sign}{seconds}s")
    } else if abs_ns.is_multiple_of(dur::NS_PER_MS) {
        let milliseconds = abs_ns / dur::NS_PER_MS;
        format!("{sign}{milliseconds}ms")
    } else if abs_ns.is_multiple_of(dur::NS_PER_US) {
        let microseconds = abs_ns / dur::NS_PER_US;
        format!("{sign}{microseconds}us")
    } else {
        format!("{sign}{abs_ns}ns")
    }
}

/// Dispatch methods on Size values.
/// Size is stored as u64 bytes.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_size_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

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
        "sub" | "subtract" => {
            require_args(method, 1, args.len())?;
            let other = require_size_arg(method, &args, 0)?;
            bytes
                .checked_sub(other)
                .map(Value::Size)
                .ok_or_else(|| EvalError::new("size subtraction would result in negative value"))
        }
        "mul" | "multiply" => {
            require_args(method, 1, args.len())?;
            let scalar = require_int_arg(method, &args, 0)?;
            if scalar < 0 {
                return Err(EvalError::new("cannot multiply Size by negative integer"));
            }
            #[expect(clippy::cast_sign_loss, reason = "checked for negative above")]
            bytes
                .checked_mul(scalar as u64)
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size multiplication"))
        }
        "div" | "divide" => {
            require_args(method, 1, args.len())?;
            let scalar = require_int_arg(method, &args, 0)?;
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
        "rem" | "remainder" => {
            require_args(method, 1, args.len())?;
            let other = require_size_arg(method, &args, 0)?;
            if other == 0 {
                Err(modulo_by_zero())
            } else {
                bytes
                    .checked_rem(other)
                    .map(Value::Size)
                    .ok_or_else(|| integer_overflow("size modulo"))
            }
        }
        // Trait methods
        "hash" => {
            require_args("hash", 0, args.len())?;
            let mut hasher = DefaultHasher::new();
            "Size".hash(&mut hasher);
            bytes.hash(&mut hasher);
            #[expect(
                clippy::cast_possible_wrap,
                reason = "Hash values are opaque identifiers"
            )]
            Ok(Value::int(hasher.finish() as i64))
        }
        "clone" => {
            require_args("clone", 0, args.len())?;
            Ok(Value::Size(bytes))
        }
        "to_str" => {
            require_args("to_str", 0, args.len())?;
            Ok(Value::string(format_size(bytes)))
        }
        "equals" => {
            require_args("equals", 1, args.len())?;
            let other = require_size_arg("equals", &args, 0)?;
            Ok(Value::Bool(bytes == other))
        }
        "compare" => {
            require_args("compare", 1, args.len())?;
            let other = require_size_arg("compare", &args, 0)?;
            Ok(ordering_to_value(bytes.cmp(&other), interner))
        }
        _ => Err(no_such_method(method, "Size")),
    }
}

/// Format a Size (bytes) as a human-readable string.
fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0b".to_string();
    }

    // Use the largest unit that gives a whole number
    if bytes.is_multiple_of(size::BYTES_PER_TB) {
        let terabytes = bytes / size::BYTES_PER_TB;
        format!("{terabytes}tb")
    } else if bytes.is_multiple_of(size::BYTES_PER_GB) {
        let gigabytes = bytes / size::BYTES_PER_GB;
        format!("{gigabytes}gb")
    } else if bytes.is_multiple_of(size::BYTES_PER_MB) {
        let megabytes = bytes / size::BYTES_PER_MB;
        format!("{megabytes}mb")
    } else if bytes.is_multiple_of(size::BYTES_PER_KB) {
        let kilobytes = bytes / size::BYTES_PER_KB;
        format!("{kilobytes}kb")
    } else {
        format!("{bytes}b")
    }
}

/// Compare two Option values.
///
/// Per spec: None < Some(_). When both are Some, compare inner values.
fn compare_option_values(
    a: &Value,
    b: &Value,
    interner: &StringInterner,
) -> Result<std::cmp::Ordering, EvalError> {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::None, Value::None) => Ok(Ordering::Equal),
        (Value::None, Value::Some(_)) => Ok(Ordering::Less),
        (Value::Some(_), Value::None) => Ok(Ordering::Greater),
        (Value::Some(a_inner), Value::Some(b_inner)) => compare_values(a_inner, b_inner, interner),
        _ => Err(EvalError::new("compare requires Option values")),
    }
}

/// Compare two values of the same type.
///
/// Used for comparing inner values of Option and other compound types.
fn compare_values(
    a: &Value,
    b: &Value,
    interner: &StringInterner,
) -> Result<std::cmp::Ordering, EvalError> {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => Ok(a.total_cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Ok(a.cmp(b)),
        (Value::Str(a), Value::Str(b)) => Ok(a.as_str().cmp(b.as_str())),
        (Value::Char(a), Value::Char(b)) => Ok(a.cmp(b)),
        (Value::Byte(a), Value::Byte(b)) => Ok(a.cmp(b)),
        (Value::Duration(a), Value::Duration(b)) => Ok(a.cmp(b)),
        (Value::Size(a), Value::Size(b)) => Ok(a.cmp(b)),
        (Value::None, Value::None) => Ok(Ordering::Equal),
        (Value::None, Value::Some(_)) | (Value::Ok(_), Value::Err(_)) => Ok(Ordering::Less),
        (Value::Some(_), Value::None) | (Value::Err(_), Value::Ok(_)) => Ok(Ordering::Greater),
        (Value::Some(a_inner), Value::Some(b_inner))
        | (Value::Ok(a_inner), Value::Ok(b_inner))
        | (Value::Err(a_inner), Value::Err(b_inner)) => compare_values(a_inner, b_inner, interner),
        // List comparison: lexicographic
        (Value::List(a_items), Value::List(b_items)) => compare_lists(a_items, b_items, interner),
        _ => Err(EvalError::new(format!(
            "cannot compare {} with {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

/// Compare two lists lexicographically.
///
/// Compares element by element. First difference determines the result.
/// If one is a prefix of the other, the shorter list is less.
fn compare_lists(
    a: &[Value],
    b: &[Value],
    interner: &StringInterner,
) -> Result<std::cmp::Ordering, EvalError> {
    use std::cmp::Ordering;
    for (a_item, b_item) in a.iter().zip(b.iter()) {
        let ord = compare_values(a_item, b_item, interner)?;
        if ord != Ordering::Equal {
            return Ok(ord);
        }
    }
    // All compared elements are equal, compare lengths
    Ok(a.len().cmp(&b.len()))
}

/// Compare two Result values.
///
/// Per spec: Ok(_) < Err(_). When both are same variant, compare inner values.
fn compare_result_values(
    a: &Value,
    b: &Value,
    interner: &StringInterner,
) -> Result<std::cmp::Ordering, EvalError> {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Ok(a_inner), Value::Ok(b_inner)) | (Value::Err(a_inner), Value::Err(b_inner)) => {
            compare_values(a_inner, b_inner, interner)
        }
        (Value::Ok(_), Value::Err(_)) => Ok(Ordering::Less),
        (Value::Err(_), Value::Ok(_)) => Ok(Ordering::Greater),
        _ => Err(EvalError::new("compare requires Result values")),
    }
}

/// Convert Rust Ordering to Ori Ordering value.
///
/// Creates a first-class `Value::Ordering` value.
fn ordering_to_value(ord: std::cmp::Ordering, _interner: &StringInterner) -> Value {
    Value::ordering_from_cmp(ord)
}

// Ordering Type Method Dispatch

/// Extract `OrderingValue` from `Value::Ordering`.
fn extract_ordering(value: &Value) -> Option<OrderingValue> {
    match value {
        Value::Ordering(ord) => Some(*ord),
        _ => None,
    }
}

/// Dispatch methods on Ordering values.
///
/// Implements the methods specified in ordering-type-proposal.md:
/// - `is_less()`, `is_equal()`, `is_greater()` → `bool`
/// - `is_less_or_equal()`, `is_greater_or_equal()` → `bool`
/// - `reverse()` → `Ordering`
/// - `compare()` → `Ordering` (Comparable trait)
/// - `equals()` → `bool` (Eq trait)
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
fn dispatch_ordering_method(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    // Extract OrderingValue from either Value::Ordering or legacy Value::Variant
    let Some(ord) = extract_ordering(&receiver) else {
        unreachable!("dispatch_ordering_method called with non-ordering receiver")
    };

    match method {
        // Predicate methods
        "is_less" => Ok(Value::Bool(ord == OrderingValue::Less)),
        "is_equal" => Ok(Value::Bool(ord == OrderingValue::Equal)),
        "is_greater" => Ok(Value::Bool(ord == OrderingValue::Greater)),
        "is_less_or_equal" => Ok(Value::Bool(
            ord == OrderingValue::Less || ord == OrderingValue::Equal,
        )),
        "is_greater_or_equal" => Ok(Value::Bool(
            ord == OrderingValue::Greater || ord == OrderingValue::Equal,
        )),

        // Reverse method
        "reverse" => {
            let reversed = match ord {
                OrderingValue::Less => OrderingValue::Greater,
                OrderingValue::Equal => OrderingValue::Equal,
                OrderingValue::Greater => OrderingValue::Less,
            };
            Ok(Value::Ordering(reversed))
        }

        // Clone trait
        "clone" => Ok(Value::Ordering(ord)),

        // Printable and Debug traits (same representation for Ordering)
        "to_str" | "debug" => Ok(Value::string(ord.name())),

        // Hashable trait
        "hash" => {
            let hash_val = match ord {
                OrderingValue::Less => -1i64,
                OrderingValue::Equal => 0i64,
                OrderingValue::Greater => 1i64,
            };
            Ok(Value::Int(hash_val.into()))
        }

        // Eq trait
        "equals" => {
            require_args("equals", 1, args.len())?;
            let Some(other_ord) = extract_ordering(&args[0]) else {
                return Err(EvalError::new("equals requires Ordering value"));
            };
            Ok(Value::Bool(ord == other_ord))
        }

        // Comparable trait: Less < Equal < Greater
        "compare" => {
            require_args("compare", 1, args.len())?;
            let Some(other_ord) = extract_ordering(&args[0]) else {
                return Err(EvalError::new("compare requires Ordering value"));
            };
            // Tags are ordered: Less(0) < Equal(1) < Greater(2)
            Ok(ordering_to_value(
                ord.to_tag().cmp(&other_ord.to_tag()),
                interner,
            ))
        }

        _ => Err(no_such_method(method, "Ordering")),
    }
}
