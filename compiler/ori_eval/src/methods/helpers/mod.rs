//! Argument validation and shared utility functions.

use std::fmt::Write;

use ori_ir::StringLookup;
use ori_patterns::{wrong_arg_count, wrong_arg_type, EvalError, EvalResult, ScalarInt, Value};

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
) -> Result<&'a [Value], EvalError> {
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
/// tuples, structs, and variants. Each value is formatted as it would appear in
/// a `.debug()` call. The `interner` parameter enables proper formatting of
/// struct/variant field and type names.
pub fn debug_value(val: &Value, interner: &dyn StringLookup) -> String {
    match val {
        Value::Int(n) => n.raw().to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{}\"", escape_debug_str(s)),
        Value::Char(c) => format!("'{}'", escape_debug_char(*c)),
        Value::Byte(b) => format!("0x{b:02x}"),
        Value::Void => "void".to_string(),
        Value::None => "None".to_string(),
        Value::Some(v) => format!("Some({})", debug_value(v, interner)),
        Value::Ok(v) => format!("Ok({})", debug_value(v, interner)),
        Value::Err(v) => format!("Err({})", debug_value(v, interner)),
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(|v| debug_value(v, interner)).collect();
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
                    debug_value(v, interner)
                );
            }
            result.push('}');
            result
        }
        Value::Set(items) => {
            let parts: Vec<String> = items.values().map(|v| debug_value(v, interner)).collect();
            format!("Set {{{}}}", parts.join(", "))
        }
        Value::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(|v| debug_value(v, interner)).collect();
            format!("({})", parts.join(", "))
        }
        Value::Struct(s) => {
            let type_name = interner.lookup(s.type_name);
            let mut sorted_fields: Vec<_> = s.layout.field_names().collect();
            sorted_fields.sort_by_key(|&(_, idx)| idx);
            let fields_str: Vec<String> = sorted_fields
                .iter()
                .map(|&(name, idx)| {
                    let field_name = interner.lookup(name);
                    let field_val = &s.fields[idx];
                    format!("{field_name}: {}", debug_value(field_val, interner))
                })
                .collect();
            format!("{type_name} {{ {} }}", fields_str.join(", "))
        }
        Value::Variant {
            variant_name,
            fields,
            ..
        } => {
            let name = interner.lookup(*variant_name);
            if fields.is_empty() {
                name.to_string()
            } else {
                let parts: Vec<String> = fields.iter().map(|v| debug_value(v, interner)).collect();
                format!("{name}({})", parts.join(", "))
            }
        }
        Value::Newtype { type_name, inner } => {
            let name = interner.lookup(*type_name);
            format!("{name}({})", debug_value(inner, interner))
        }
        Value::Duration(ns) => super::units::format_duration_debug(*ns),
        Value::Size(bytes) => super::units::format_size_debug(*bytes),
        Value::Ordering(ord) => ord.name().to_string(),
        Value::Range(r) => format!("{r:?}"),
        // Closure, Iterator, etc. — fall back to Display
        other => format!("{other}"),
    }
}

#[cfg(test)]
mod tests;
