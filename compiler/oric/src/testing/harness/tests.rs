use super::*;

#[test]
fn test_eval_source() {
    let result = eval_source("@main () -> int = 42");
    assert_eq!(result.unwrap(), Value::int(42));
}

#[test]
fn test_assert_eval_int() {
    assert_eval_int("1 + 2", 3);
    assert_eval_int("10 - 3", 7);
    assert_eval_int("4 * 5", 20);
}

#[test]
fn test_assert_eval_bool() {
    assert_eval_bool("true", true);
    assert_eval_bool("false", false);
    assert_eval_bool("1 == 1", true);
    assert_eval_bool("1 == 2", false);
}

#[test]
fn test_assert_eval_str() {
    assert_eval_str("\"hello\"", "hello");
    assert_eval_str("\"a\" + \"b\"", "ab");
}

#[test]
fn test_parse_source() {
    let (parsed, _interner) = parse_source("@main () -> int = 42");
    assert!(!parsed.has_errors());
    assert_eq!(parsed.module.functions.len(), 1);
}

#[test]
fn test_type_check_source() {
    let (_, result, _interner) = type_check_source("@main () -> int = 42");
    assert!(!result.has_errors());
}
