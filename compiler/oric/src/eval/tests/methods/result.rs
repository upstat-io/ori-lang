//! Result method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use super::test_interner;
use crate::eval::Value;
use ori_eval::dispatch_builtin_method;

#[test]
fn unwrap_ok() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::ok(Value::int(42)), "unwrap", vec![], &interner).unwrap(),
        Value::int(42)
    );
}

#[test]
fn unwrap_err_error() {
    let interner = test_interner();
    assert!(dispatch_builtin_method(
        Value::err(Value::string("error")),
        "unwrap",
        vec![],
        &interner
    )
    .is_err());
}

#[test]
fn is_ok() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::ok(Value::int(1)), "is_ok", vec![], &interner).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::err(Value::string("e")), "is_ok", vec![], &interner)
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn is_err() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::err(Value::string("e")), "is_err", vec![], &interner)
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::ok(Value::int(1)), "is_err", vec![], &interner).unwrap(),
        Value::Bool(false)
    );
}
