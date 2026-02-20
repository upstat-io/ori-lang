use super::*;

#[test]
fn test_parse_source_simple_function() {
    let output = parse_source("@add(a: int, b: int) -> int = a + b;");
    assert!(!output.has_errors());
    assert_eq!(output.module.functions.len(), 1);
}

#[test]
fn test_parse_ok_succeeds() {
    let output = parse_ok("let $x = 42;");
    assert!(!output.has_errors());
}

#[test]
#[should_panic(expected = "Expected successful parse")]
fn test_parse_ok_panics_on_error() {
    parse_ok("@foo(");
}

#[test]
fn test_parse_err_catches_error() {
    parse_err("@foo(", "expected");
}
