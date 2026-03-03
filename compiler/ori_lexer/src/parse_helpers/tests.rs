use super::*;

#[test]
fn test_parse_int_skip_underscores() {
    assert_eq!(parse_int_skip_underscores("123", 10), Some(123));
    assert_eq!(parse_int_skip_underscores("1_000_000", 10), Some(1_000_000));
    assert_eq!(parse_int_skip_underscores("1_2_3", 10), Some(123));
    assert_eq!(parse_int_skip_underscores("___1___", 10), Some(1));
}

#[test]
fn test_parse_int_hex_with_underscores() {
    assert_eq!(parse_int_skip_underscores("FF", 16), Some(255));
    assert_eq!(parse_int_skip_underscores("F_F", 16), Some(255));
    assert_eq!(
        parse_int_skip_underscores("dead_beef", 16),
        Some(0xdead_beef)
    );
}

#[test]
fn test_parse_int_binary_with_underscores() {
    assert_eq!(parse_int_skip_underscores("1010", 2), Some(10));
    assert_eq!(parse_int_skip_underscores("1_0_1_0", 2), Some(10));
    assert_eq!(parse_int_skip_underscores("1111_0000", 2), Some(240));
}

#[test]
fn test_parse_int_overflow() {
    // Should return None on overflow
    assert_eq!(
        parse_int_skip_underscores("99999999999999999999999", 10),
        None
    );
}

#[test]
fn test_parse_int_empty_digits_returns_none() {
    // Finding 1: empty digit sequence must return None, not Some(0)
    assert_eq!(parse_int_skip_underscores("", 16), None);
    assert_eq!(parse_int_skip_underscores("", 2), None);
    assert_eq!(parse_int_skip_underscores("", 10), None);
}

#[test]
fn test_parse_int_only_underscores_returns_none() {
    // Finding 1: underscores-only = no real digits seen
    assert_eq!(parse_int_skip_underscores("___", 16), None);
    assert_eq!(parse_int_skip_underscores("_", 2), None);
    assert_eq!(parse_int_skip_underscores("__", 10), None);
}

#[test]
fn test_parse_int_zero_digit_still_valid() {
    // Regression guard: actual digit zero must still parse correctly
    assert_eq!(parse_int_skip_underscores("0", 16), Some(0));
    assert_eq!(parse_int_skip_underscores("0", 2), Some(0));
    assert_eq!(parse_int_skip_underscores("0", 10), Some(0));
}

#[test]
#[allow(
    clippy::approx_constant,
    reason = "testing float parsing, not using mathematical constants"
)]
fn test_parse_float_skip_underscores() {
    assert_eq!(parse_float_skip_underscores("3.14"), Some(3.14));
    assert_eq!(parse_float_skip_underscores("1_000.5"), Some(1000.5));
    assert_eq!(parse_float_skip_underscores("1.5e10"), Some(1.5e10));
}

#[test]
fn test_parse_float_overflow_returns_none() {
    // Finding 2: overflow to infinity must return None
    assert_eq!(parse_float_skip_underscores("1e999"), None);
    assert_eq!(parse_float_skip_underscores("-1e999"), None);
}

#[test]
fn test_parse_float_max_finite_is_valid() {
    // Regression guard: f64::MAX is finite and must parse
    assert_eq!(parse_float_skip_underscores("1e308"), Some(1e308));
    assert!(parse_float_skip_underscores("1.7976931348623157e308").is_some());
}
