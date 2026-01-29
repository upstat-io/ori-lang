//! String method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::Value;
use ori_eval::dispatch_builtin_method;

#[test]
fn len() {
    assert_eq!(
        dispatch_builtin_method(Value::string("hello"), "len", vec![]).unwrap(),
        Value::int(5)
    );
    assert_eq!(
        dispatch_builtin_method(Value::string(""), "len", vec![]).unwrap(),
        Value::int(0)
    );
}

#[test]
fn is_empty() {
    assert_eq!(
        dispatch_builtin_method(Value::string(""), "is_empty", vec![]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::string("hello"), "is_empty", vec![]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn to_uppercase() {
    assert_eq!(
        dispatch_builtin_method(Value::string("hello"), "to_uppercase", vec![]).unwrap(),
        Value::string("HELLO")
    );
    assert_eq!(
        dispatch_builtin_method(Value::string("Hello World"), "to_uppercase", vec![]).unwrap(),
        Value::string("HELLO WORLD")
    );
}

#[test]
fn to_lowercase() {
    assert_eq!(
        dispatch_builtin_method(Value::string("HELLO"), "to_lowercase", vec![]).unwrap(),
        Value::string("hello")
    );
}

#[test]
fn trim() {
    assert_eq!(
        dispatch_builtin_method(Value::string("  hello  "), "trim", vec![]).unwrap(),
        Value::string("hello")
    );
    assert_eq!(
        dispatch_builtin_method(Value::string("\n\thello\t\n"), "trim", vec![]).unwrap(),
        Value::string("hello")
    );
}

#[test]
fn contains() {
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello world"),
            "contains",
            vec![Value::string("world")]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello"),
            "contains",
            vec![Value::string("xyz")]
        )
        .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn starts_with() {
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello world"),
            "starts_with",
            vec![Value::string("hello")]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello"),
            "starts_with",
            vec![Value::string("world")]
        )
        .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn ends_with() {
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello world"),
            "ends_with",
            vec![Value::string("world")]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello"),
            "ends_with",
            vec![Value::string("xyz")]
        )
        .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn wrong_arg_type() {
    // contains expects string, not int
    assert!(
        dispatch_builtin_method(Value::string("hello"), "contains", vec![Value::int(1)]).is_err()
    );
}
