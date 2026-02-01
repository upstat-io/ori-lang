//! Option method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use super::test_interner;
use crate::eval::Value;
use ori_eval::dispatch_builtin_method;

#[test]
fn unwrap_some() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::some(Value::int(42)), "unwrap", vec![], &interner).unwrap(),
        Value::int(42)
    );
}

#[test]
fn unwrap_none_error() {
    let interner = test_interner();
    assert!(dispatch_builtin_method(Value::None, "unwrap", vec![], &interner).is_err());
}

#[test]
fn unwrap_or() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(
            Value::some(Value::int(42)),
            "unwrap_or",
            vec![Value::int(0)],
            &interner
        )
        .unwrap(),
        Value::int(42)
    );
    assert_eq!(
        dispatch_builtin_method(Value::None, "unwrap_or", vec![Value::int(0)], &interner).unwrap(),
        Value::int(0)
    );
}

#[test]
fn is_some() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::some(Value::int(1)), "is_some", vec![], &interner).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::None, "is_some", vec![], &interner).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn is_none() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::None, "is_none", vec![], &interner).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::some(Value::int(1)), "is_none", vec![], &interner).unwrap(),
        Value::Bool(false)
    );
}
