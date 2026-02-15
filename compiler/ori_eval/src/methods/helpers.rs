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
    // Proper-cased types sort before lowercase in ASCII (A-Z < a-z).
    // Type names must match TypeNames (e.g., "Duration" not "duration").
    //
    // Duration - operators and traits
    ("Duration", "add"),
    ("Duration", "clone"),
    ("Duration", "compare"),
    ("Duration", "div"),
    ("Duration", "divide"),
    ("Duration", "equals"),
    ("Duration", "hash"),
    ("Duration", "hours"),
    ("Duration", "microseconds"),
    ("Duration", "milliseconds"),
    ("Duration", "minutes"),
    ("Duration", "mul"),
    ("Duration", "multiply"),
    ("Duration", "nanoseconds"),
    ("Duration", "neg"),
    ("Duration", "negate"),
    ("Duration", "rem"),
    ("Duration", "remainder"),
    ("Duration", "seconds"),
    ("Duration", "sub"),
    ("Duration", "subtract"),
    ("Duration", "to_str"),
    // Option - methods and traits
    ("Option", "clone"),
    ("Option", "compare"),
    ("Option", "is_none"),
    ("Option", "is_some"),
    ("Option", "ok_or"),
    ("Option", "unwrap"),
    ("Option", "unwrap_or"),
    // Ordering - predicates and traits
    ("Ordering", "clone"),
    ("Ordering", "compare"),
    ("Ordering", "debug"),
    ("Ordering", "equals"),
    ("Ordering", "hash"),
    ("Ordering", "is_equal"),
    ("Ordering", "is_greater"),
    ("Ordering", "is_greater_or_equal"),
    ("Ordering", "is_less"),
    ("Ordering", "is_less_or_equal"),
    ("Ordering", "reverse"),
    ("Ordering", "to_str"),
    // Result - methods and traits
    ("Result", "clone"),
    ("Result", "compare"),
    ("Result", "is_err"),
    ("Result", "is_ok"),
    ("Result", "unwrap"),
    // Size - operators and traits
    ("Size", "add"),
    ("Size", "bytes"),
    ("Size", "clone"),
    ("Size", "compare"),
    ("Size", "div"),
    ("Size", "divide"),
    ("Size", "equals"),
    ("Size", "gigabytes"),
    ("Size", "hash"),
    ("Size", "kilobytes"),
    ("Size", "megabytes"),
    ("Size", "mul"),
    ("Size", "multiply"),
    ("Size", "rem"),
    ("Size", "remainder"),
    ("Size", "sub"),
    ("Size", "subtract"),
    ("Size", "terabytes"),
    ("Size", "to_str"),
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
    ("list", "compare"),
    ("list", "concat"),
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
    // range
    ("range", "contains"),
    ("range", "len"),
    // str - methods and traits
    ("str", "add"),
    ("str", "clone"),
    ("str", "compare"),
    ("str", "concat"),
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
    // tuple - traits
    ("tuple", "clone"),
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
