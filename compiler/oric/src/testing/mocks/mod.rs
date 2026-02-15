//! Mock implementations and test utilities for testing.
//!
//! This module provides test value constructors and other utilities
//! for testing the compiler.

use crate::eval::Value;

// Test Value Constructors

/// Create a simple test value for use in tests.
pub fn test_int(n: i64) -> Value {
    Value::int(n)
}

/// Create a simple test float value.
pub fn test_float(f: f64) -> Value {
    Value::Float(f)
}

/// Create a simple test string value.
pub fn test_str(s: &str) -> Value {
    Value::string(s)
}

/// Create a simple test boolean value.
pub fn test_bool(b: bool) -> Value {
    Value::Bool(b)
}

/// Create a test char value.
pub fn test_char(c: char) -> Value {
    Value::Char(c)
}

/// Create a test Some value.
pub fn test_some(v: Value) -> Value {
    Value::some(v)
}

/// Create a test None value.
pub fn test_none() -> Value {
    Value::None
}

/// Create a test Ok value.
pub fn test_ok(v: Value) -> Value {
    Value::ok(v)
}

/// Create a test Err value.
pub fn test_err(v: Value) -> Value {
    Value::err(v)
}

/// Create a test list value.
pub fn test_list(items: Vec<Value>) -> Value {
    Value::list(items)
}

/// Create a test tuple value.
pub fn test_tuple(items: Vec<Value>) -> Value {
    Value::tuple(items)
}

/// Create a void value.
pub fn test_void() -> Value {
    Value::Void
}

// Value Matchers

/// Check if a value is an integer with the expected value.
pub fn is_int(value: &Value, expected: i64) -> bool {
    matches!(value, Value::Int(n) if n.raw() == expected)
}

/// Check if a value is a float within epsilon of expected.
pub fn is_float(value: &Value, expected: f64) -> bool {
    match value {
        Value::Float(f) => (f - expected).abs() < 1e-10,
        _ => false,
    }
}

/// Check if a value is a boolean with the expected value.
pub fn is_bool(value: &Value, expected: bool) -> bool {
    matches!(value, Value::Bool(b) if *b == expected)
}

/// Check if a value is a string with the expected value.
pub fn is_str(value: &Value, expected: &str) -> bool {
    match value {
        Value::Str(s) => **s == *expected,
        _ => false,
    }
}

/// Check if a value is Some containing the expected value.
pub fn is_some_with(value: &Value, expected: &Value) -> bool {
    match value {
        Value::Some(inner) => inner.as_ref() == expected,
        _ => false,
    }
}

/// Check if a value is None.
pub fn is_none(value: &Value) -> bool {
    matches!(value, Value::None)
}

/// Check if a value is Ok containing the expected value.
pub fn is_ok_with(value: &Value, expected: &Value) -> bool {
    match value {
        Value::Ok(inner) => inner.as_ref() == expected,
        _ => false,
    }
}

/// Check if a value is Err.
pub fn is_err(value: &Value) -> bool {
    matches!(value, Value::Err(_))
}

#[cfg(test)]
mod tests;
