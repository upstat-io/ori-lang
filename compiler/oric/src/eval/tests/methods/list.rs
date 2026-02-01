//! List method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use super::test_interner;
use crate::eval::Value;
use ori_eval::dispatch_builtin_method;

#[test]
fn len() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(
            Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]),
            "len",
            vec![],
            &interner
        )
        .unwrap(),
        Value::int(3)
    );
    assert_eq!(
        dispatch_builtin_method(Value::list(vec![]), "len", vec![], &interner).unwrap(),
        Value::int(0)
    );
}

#[test]
fn is_empty() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(Value::list(vec![]), "is_empty", vec![], &interner).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::list(vec![Value::int(1)]),
            "is_empty",
            vec![],
            &interner
        )
        .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn first() {
    let interner = test_interner();
    // Non-empty list
    let result = dispatch_builtin_method(
        Value::list(vec![Value::int(1), Value::int(2)]),
        "first",
        vec![],
        &interner,
    )
    .unwrap();
    assert_eq!(result, Value::some(Value::int(1)));

    // Empty list
    let result = dispatch_builtin_method(Value::list(vec![]), "first", vec![], &interner).unwrap();
    assert_eq!(result, Value::None);
}

#[test]
fn last() {
    let interner = test_interner();
    // Non-empty list
    let result = dispatch_builtin_method(
        Value::list(vec![Value::int(1), Value::int(2)]),
        "last",
        vec![],
        &interner,
    )
    .unwrap();
    assert_eq!(result, Value::some(Value::int(2)));

    // Empty list
    let result = dispatch_builtin_method(Value::list(vec![]), "last", vec![], &interner).unwrap();
    assert_eq!(result, Value::None);
}

#[test]
fn contains() {
    let interner = test_interner();
    let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);

    assert_eq!(
        dispatch_builtin_method(list.clone(), "contains", vec![Value::int(2)], &interner).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(list, "contains", vec![Value::int(5)], &interner).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn contains_wrong_arg_count() {
    let interner = test_interner();
    let list = Value::list(vec![Value::int(1)]);

    assert!(dispatch_builtin_method(list.clone(), "contains", vec![], &interner).is_err());
    assert!(dispatch_builtin_method(
        list,
        "contains",
        vec![Value::int(1), Value::int(2)],
        &interner
    )
    .is_err());
}
