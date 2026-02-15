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
#[allow(
    clippy::approx_constant,
    reason = "testing float parsing, not using mathematical constants"
)]
fn test_parse_float_skip_underscores() {
    assert_eq!(parse_float_skip_underscores("3.14"), Some(3.14));
    assert_eq!(parse_float_skip_underscores("1_000.5"), Some(1000.5));
    assert_eq!(parse_float_skip_underscores("1.5e10"), Some(1.5e10));
}
