//! Result method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::Value;
use ori_eval::dispatch_builtin_method;

#[test]
fn unwrap_ok() {
    assert_eq!(
        dispatch_builtin_method(Value::ok(Value::int(42)), "unwrap", vec![]).unwrap(),
        Value::int(42)
    );
}

#[test]
fn unwrap_err_error() {
    assert!(dispatch_builtin_method(Value::err(Value::string("error")), "unwrap", vec![]).is_err());
}

#[test]
fn is_ok() {
    assert_eq!(
        dispatch_builtin_method(Value::ok(Value::int(1)), "is_ok", vec![]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::err(Value::string("e")), "is_ok", vec![]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn is_err() {
    assert_eq!(
        dispatch_builtin_method(Value::err(Value::string("e")), "is_err", vec![]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::ok(Value::int(1)), "is_err", vec![]).unwrap(),
        Value::Bool(false)
    );
}
