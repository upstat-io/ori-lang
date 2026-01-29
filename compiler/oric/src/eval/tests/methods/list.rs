//! List method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use crate::eval::Value;
use ori_eval::dispatch_builtin_method;

#[test]
fn len() {
    assert_eq!(
        dispatch_builtin_method(
            Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]),
            "len",
            vec![]
        )
        .unwrap(),
        Value::int(3)
    );
    assert_eq!(
        dispatch_builtin_method(Value::list(vec![]), "len", vec![]).unwrap(),
        Value::int(0)
    );
}

#[test]
fn is_empty() {
    assert_eq!(
        dispatch_builtin_method(Value::list(vec![]), "is_empty", vec![]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(Value::list(vec![Value::int(1)]), "is_empty", vec![]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn first() {
    // Non-empty list
    let result = dispatch_builtin_method(
        Value::list(vec![Value::int(1), Value::int(2)]),
        "first",
        vec![],
    )
    .unwrap();
    assert_eq!(result, Value::some(Value::int(1)));

    // Empty list
    let result = dispatch_builtin_method(Value::list(vec![]), "first", vec![]).unwrap();
    assert_eq!(result, Value::None);
}

#[test]
fn last() {
    // Non-empty list
    let result = dispatch_builtin_method(
        Value::list(vec![Value::int(1), Value::int(2)]),
        "last",
        vec![],
    )
    .unwrap();
    assert_eq!(result, Value::some(Value::int(2)));

    // Empty list
    let result = dispatch_builtin_method(Value::list(vec![]), "last", vec![]).unwrap();
    assert_eq!(result, Value::None);
}

#[test]
fn contains() {
    let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);

    assert_eq!(
        dispatch_builtin_method(list.clone(), "contains", vec![Value::int(2)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(list, "contains", vec![Value::int(5)]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn contains_wrong_arg_count() {
    let list = Value::list(vec![Value::int(1)]);

    assert!(dispatch_builtin_method(list.clone(), "contains", vec![]).is_err());
    assert!(dispatch_builtin_method(list, "contains", vec![Value::int(1), Value::int(2)]).is_err());
}
