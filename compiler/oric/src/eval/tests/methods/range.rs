//! Range method tests.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use super::test_interner;
use crate::eval::Value;
use ori_eval::dispatch_builtin_method_str as dispatch_builtin_method;
use ori_patterns::RangeValue;

#[test]
fn len() {
    let interner = test_interner();
    assert_eq!(
        dispatch_builtin_method(
            Value::Range(RangeValue::exclusive(0, 10)),
            "len",
            vec![],
            &interner
        )
        .unwrap(),
        Value::int(10)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::Range(RangeValue::inclusive(0, 10)),
            "len",
            vec![],
            &interner
        )
        .unwrap(),
        Value::int(11)
    );
    assert_eq!(
        dispatch_builtin_method(
            Value::Range(RangeValue::exclusive(5, 5)),
            "len",
            vec![],
            &interner
        )
        .unwrap(),
        Value::int(0)
    );
}

#[test]
fn contains() {
    let interner = test_interner();
    let range = Value::Range(RangeValue::exclusive(0, 10));

    assert_eq!(
        dispatch_builtin_method(range.clone(), "contains", vec![Value::int(5)], &interner).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(range.clone(), "contains", vec![Value::int(0)], &interner).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        dispatch_builtin_method(range.clone(), "contains", vec![Value::int(10)], &interner)
            .unwrap(),
        Value::Bool(false) // Exclusive end
    );
    assert_eq!(
        dispatch_builtin_method(range, "contains", vec![Value::int(-1)], &interner).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn contains_inclusive() {
    let interner = test_interner();
    let range = Value::Range(RangeValue::inclusive(0, 10));

    assert_eq!(
        dispatch_builtin_method(range.clone(), "contains", vec![Value::int(10)], &interner)
            .unwrap(),
        Value::Bool(true) // Inclusive end
    );
}
