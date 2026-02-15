use super::*;

#[test]
fn test_int_constructor() {
    assert_eq!(test_int(42), Value::int(42));
}

#[test]
fn test_bool_constructor() {
    assert_eq!(test_bool(true), Value::Bool(true));
    assert_eq!(test_bool(false), Value::Bool(false));
}

#[test]
fn test_option_constructors() {
    assert_eq!(test_some(test_int(1)), Value::some(Value::int(1)));
    assert_eq!(test_none(), Value::None);
}

#[test]
fn test_result_constructors() {
    assert_eq!(test_ok(test_int(1)), Value::ok(Value::int(1)));
    let err_val = test_err(test_str("error"));
    assert!(matches!(err_val, Value::Err(_)));
}

#[test]
fn test_is_int() {
    assert!(is_int(&Value::int(42), 42));
    assert!(!is_int(&Value::int(42), 43));
    assert!(!is_int(&Value::Bool(true), 42));
}

#[test]
fn test_is_bool() {
    assert!(is_bool(&Value::Bool(true), true));
    assert!(!is_bool(&Value::Bool(true), false));
}

#[test]
fn test_is_none() {
    assert!(is_none(&Value::None));
    assert!(!is_none(&Value::some(Value::int(1))));
}
