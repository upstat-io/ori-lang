//! Argument validation and shared utility functions.

use std::fmt::Write;

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
    ("Duration", "debug"),
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
    // Iterator methods are dispatched by CollectionMethodResolver, not here.
    // Option - methods and traits
    ("Option", "clone"),
    ("Option", "compare"),
    ("Option", "debug"),
    ("Option", "is_none"),
    ("Option", "is_some"),
    ("Option", "iter"),
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
    ("Ordering", "then"),
    ("Ordering", "to_str"),
    // Result - methods and traits
    ("Result", "clone"),
    ("Result", "compare"),
    ("Result", "debug"),
    ("Result", "is_err"),
    ("Result", "is_ok"),
    ("Result", "unwrap"),
    // Set - methods and traits
    ("Set", "debug"),
    ("Set", "iter"),
    ("Set", "len"),
    // Size - operators and traits
    ("Size", "add"),
    ("Size", "bytes"),
    ("Size", "clone"),
    ("Size", "compare"),
    ("Size", "debug"),
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
    ("list", "iter"),
    ("list", "last"),
    ("list", "len"),
    // map
    ("map", "clone"),
    ("map", "contains_key"),
    ("map", "debug"),
    ("map", "is_empty"),
    ("map", "iter"),
    ("map", "keys"),
    ("map", "len"),
    ("map", "values"),
    // range
    ("range", "contains"),
    ("range", "iter"),
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
    ("str", "escape"),
    ("str", "hash"),
    ("str", "is_empty"),
    ("str", "iter"),
    ("str", "len"),
    ("str", "starts_with"),
    ("str", "to_lowercase"),
    ("str", "to_str"),
    ("str", "to_uppercase"),
    ("str", "trim"),
    // tuple - traits
    ("tuple", "clone"),
    ("tuple", "debug"),
    ("tuple", "len"),
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

/// Escape a string for Debug output (newlines, tabs, quotes, backslashes, null).
pub fn escape_debug_str(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\0' => result.push_str("\\0"),
            c => result.push(c),
        }
    }
    result
}

/// Escape a char for Debug output.
pub fn escape_debug_char(c: char) -> String {
    match c {
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\0' => "\\0".to_string(),
        c => c.to_string(),
    }
}

/// Format a `Value` using Debug semantics (developer-facing structural output).
///
/// This is the recursive workhorse for `.debug()` on collections, Option, Result,
/// and tuples. Each value is formatted as it would appear in a `.debug()` call.
pub fn debug_value(val: &Value) -> String {
    match val {
        Value::Int(n) => n.raw().to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{}\"", escape_debug_str(s)),
        Value::Char(c) => format!("'{}'", escape_debug_char(*c)),
        Value::Byte(b) => format!("0x{b:02x}"),
        Value::Void => "void".to_string(),
        Value::None => "None".to_string(),
        Value::Some(v) => format!("Some({})", debug_value(v)),
        Value::Ok(v) => format!("Ok({})", debug_value(v)),
        Value::Err(v) => format!("Err({})", debug_value(v)),
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(debug_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Map(map) => {
            let mut result = String::from("{");
            let mut first = true;
            for (k, v) in map.iter() {
                if !first {
                    result.push_str(", ");
                }
                first = false;
                // Map keys are internally prefixed (e.g. "s:name", "i:42").
                // Strip the type prefix for debug display.
                let display_key = k.split_once(':').map_or(k.as_str(), |(_, rest)| rest);
                let _ = write!(
                    result,
                    "{}: {}",
                    escape_debug_str(display_key),
                    debug_value(v)
                );
            }
            result.push('}');
            result
        }
        Value::Set(items) => {
            let parts: Vec<String> = items.values().map(debug_value).collect();
            format!("Set {{{}}}", parts.join(", "))
        }
        Value::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(debug_value).collect();
            format!("({})", parts.join(", "))
        }
        Value::Duration(ns) => super::units::format_duration_debug(*ns),
        Value::Size(bytes) => super::units::format_size_debug(*bytes),
        Value::Ordering(ord) => ord.name().to_string(),
        Value::Range(r) => format!("{r:?}"),
        // Struct, Closure, Iterator, etc. — fall back to Display
        other => format!("{other}"),
    }
}

#[cfg(test)]
mod tests;
