//! String method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use super::test_interner;
use crate::eval::Value;
use ori_eval::dispatch_builtin_method_str as dispatch_builtin_method;

#[test]
fn len() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::string("hello"), "len", vec![], &interner).unwrap(),
        Value::int(5)
    );
    assert_eq!(
        dispatch_builtin_method(Value::string(""), "len", vec![], &interner).unwrap(),
        Value::int(0)
    );
}

#[test]
fn is_empty() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::string(""), "is_empty", vec![], &interner).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::string("hello"), "is_empty", vec![], &interner).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn to_uppercase() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::string("hello"), "to_uppercase", vec![], &interner).unwrap(),
        Value::string("HELLO")
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::string("Hello World"),
            "to_uppercase",
            vec![],
            &interner
        )
        .unwrap(),
        Value::string("HELLO WORLD")
    );
}

#[test]
fn to_lowercase() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::string("HELLO"), "to_lowercase", vec![], &interner).unwrap(),
        Value::string("hello")
    );
}

#[test]
fn trim() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::string("  hello  "), "trim", vec![], &interner).unwrap(),
        Value::string("hello")
    );
    assert_eq!(
        dispatch_builtin_method(Value::string("\n\thello\t\n"), "trim", vec![], &interner).unwrap(),
        Value::string("hello")
    );
}

#[test]
fn contains() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello world"),
            "contains",
            vec![Value::string("world")],
            &interner
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello"),
            "contains",
            vec![Value::string("xyz")],
            &interner
        )
        .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn starts_with() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello world"),
            "starts_with",
            vec![Value::string("hello")],
            &interner
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello"),
            "starts_with",
            vec![Value::string("world")],
            &interner
        )
        .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn ends_with() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello world"),
            "ends_with",
            vec![Value::string("world")],
            &interner
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::string("hello"),
            "ends_with",
            vec![Value::string("xyz")],
            &interner
        )
        .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn wrong_arg_type() {
    let interner = test_interner();
    // contains expects string, not int
    assert!(dispatch_builtin_method(
        Value::string("hello"),
        "contains",
        vec![Value::int(1)],
        &interner
    )
    .is_err());
}
