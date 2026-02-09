//! Argument validation and shared utility functions.

use ori_patterns::{
    wrong_arg_count, wrong_arg_type, EvalError, EvalResult, Heap, ScalarInt, Value,
};

/// All built-in methods registered in the evaluator's direct dispatch.
///
/// Used by cross-crate consistency tests to verify the evaluator and type
/// checker agree on which methods exist. Each entry is `(type_name, method_name)`.
/// Sorted by type then method for deterministic comparison.
pub const EVAL_BUILTIN_METHODS: &[(&str, &str)] = &[
    // bool - operators and traits
    ("bool", "clone"),
    ("bool", "compare"),
    ("bool", "debug"),
    ("bool", "equals"),
    ("bool", "hash"),
    ("bool", "not"),
    ("bool", "to_str"),
    // byte - traits
    ("byte", "clone"),
    ("byte", "compare"),
    ("byte", "debug"),
    ("byte", "equals"),
    ("byte", "hash"),
    ("byte", "to_str"),
    // char - traits
    ("char", "clone"),
    ("char", "compare"),
    ("char", "debug"),
    ("char", "equals"),
    ("char", "hash"),
    ("char", "to_str"),
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
    // float - operators and traits
    ("float", "add"),
    ("float", "clone"),
    ("float", "compare"),
    ("float", "debug"),
    ("float", "div"),
    ("float", "equals"),
    ("float", "hash"),
    ("float", "mul"),
    ("float", "neg"),
    ("float", "sub"),
    ("float", "to_str"),
    // int - operators and traits
    ("int", "add"),
    ("int", "bit_and"),
    ("int", "bit_not"),
    ("int", "bit_or"),
    ("int", "bit_xor"),
    ("int", "clone"),
    ("int", "compare"),
    ("int", "debug"),
    ("int", "div"),
    ("int", "equals"),
    ("int", "floor_div"),
    ("int", "hash"),
    ("int", "mul"),
    ("int", "neg"),
    ("int", "rem"),
    ("int", "shl"),
    ("int", "shr"),
    ("int", "sub"),
    ("int", "to_str"),
    // list - methods and traits
    ("list", "add"),
    ("list", "clone"),
    ("list", "contains"),
    ("list", "debug"),
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
    ("option", "ok_or"),
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
    // str - methods and traits
    ("str", "add"),
    ("str", "clone"),
    ("str", "compare"),
    ("str", "contains"),
    ("str", "debug"),
    ("str", "ends_with"),
    ("str", "equals"),
    ("str", "hash"),
    ("str", "is_empty"),
    ("str", "len"),
    ("str", "starts_with"),
    ("str", "to_lowercase"),
    ("str", "to_str"),
    ("str", "to_uppercase"),
    ("str", "trim"),
];

/// Validate expected argument count.
#[inline]
pub fn require_args(method: &str, expected: usize, actual: usize) -> Result<(), EvalError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wrong_arg_count(method, expected, actual))
    }
}

/// Extract a string argument at the given index.
#[inline]
pub fn require_str_arg<'a>(
    method: &str,
    args: &'a [Value],
    index: usize,
) -> Result<&'a str, EvalError> {
    match args.get(index) {
        Some(Value::Str(s)) => Ok(&**s),
        _ => Err(wrong_arg_type(method, "string")),
    }
}

/// Extract an integer argument at the given index.
#[inline]
pub fn require_int_arg(method: &str, args: &[Value], index: usize) -> Result<i64, EvalError> {
    match args.get(index) {
        Some(Value::Int(n)) => Ok(n.raw()),
        _ => Err(wrong_arg_type(method, "int")),
    }
}

/// Extract a `ScalarInt` argument at the given index.
#[inline]
pub fn require_scalar_int_arg(
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
pub fn require_float_arg(method: &str, args: &[Value], index: usize) -> Result<f64, EvalError> {
    match args.get(index) {
        Some(Value::Float(f)) => Ok(*f),
        _ => Err(wrong_arg_type(method, "float")),
    }
}

/// Extract a list argument at the given index.
#[inline]
pub fn require_list_arg<'a>(
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
pub fn require_duration_arg(method: &str, args: &[Value], index: usize) -> Result<i64, EvalError> {
    match args.get(index) {
        Some(Value::Duration(d)) => Ok(*d),
        _ => Err(wrong_arg_type(method, "Duration")),
    }
}

/// Extract a Size argument at the given index.
#[inline]
pub fn require_size_arg(method: &str, args: &[Value], index: usize) -> Result<u64, EvalError> {
    match args.get(index) {
        Some(Value::Size(s)) => Ok(*s),
        _ => Err(wrong_arg_type(method, "Size")),
    }
}

/// Extract a bool argument at the given index.
#[inline]
pub fn require_bool_arg(method: &str, args: &[Value], index: usize) -> Result<bool, EvalError> {
    match args.get(index) {
        Some(Value::Bool(b)) => Ok(*b),
        _ => Err(wrong_arg_type(method, "bool")),
    }
}

/// Extract a char argument at the given index.
#[inline]
pub fn require_char_arg(method: &str, args: &[Value], index: usize) -> Result<char, EvalError> {
    match args.get(index) {
        Some(Value::Char(c)) => Ok(*c),
        _ => Err(wrong_arg_type(method, "char")),
    }
}

/// Extract a byte argument at the given index.
#[inline]
pub fn require_byte_arg(method: &str, args: &[Value], index: usize) -> Result<u8, EvalError> {
    match args.get(index) {
        Some(Value::Byte(b)) => Ok(*b),
        _ => Err(wrong_arg_type(method, "byte")),
    }
}

/// Convert a collection length to a Value, with overflow check.
#[inline]
pub fn len_to_value(len: usize, collection_type: &str) -> EvalResult {
    i64::try_from(len)
        .map(Value::int)
        .map_err(|_| EvalError::new(format!("{collection_type} too large")).into())
}
