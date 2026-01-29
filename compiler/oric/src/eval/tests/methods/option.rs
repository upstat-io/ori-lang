//! Option method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::Value;
use ori_eval::dispatch_builtin_method;

#[test]
fn unwrap_some() {
    assert_eq!(
        dispatch_builtin_method(Value::some(Value::int(42)), "unwrap", vec![]).unwrap(),
        Value::int(42)
    );
}

#[test]
fn unwrap_none_error() {
    assert!(dispatch_builtin_method(Value::None, "unwrap", vec![]).is_err());
}

#[test]
fn unwrap_or() {
    assert_eq!(
        dispatch_builtin_method(
            Value::some(Value::int(42)),
            "unwrap_or",
            vec![Value::int(0)]
        )
        .unwrap(),
        Value::int(42)
    );
    assert_eq!(
        dispatch_builtin_method(Value::None, "unwrap_or", vec![Value::int(0)]).unwrap(),
        Value::int(0)
    );
}

#[test]
fn is_some() {
    assert_eq!(
        dispatch_builtin_method(Value::some(Value::int(1)), "is_some", vec![]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::None, "is_some", vec![]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn is_none() {
    assert_eq!(
        dispatch_builtin_method(Value::None, "is_none", vec![]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::some(Value::int(1)), "is_none", vec![]).unwrap(),
        Value::Bool(false)
    );
}
