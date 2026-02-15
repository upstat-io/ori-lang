use super::*;

// Integer width tests

#[test]
fn test_int_width_zero() {
    assert_eq!(int_width(0), 1);
}

#[test]
fn test_int_width_single_digit() {
    assert_eq!(int_width(1), 1);
    assert_eq!(int_width(9), 1);
}

#[test]
fn test_int_width_multi_digit() {
    assert_eq!(int_width(10), 2);
    assert_eq!(int_width(99), 2);
    assert_eq!(int_width(100), 3);
    assert_eq!(int_width(1000), 4);
    assert_eq!(int_width(123_456), 6);
}

#[test]
fn test_int_width_negative() {
    assert_eq!(int_width(-1), 2); // "-1"
    assert_eq!(int_width(-99), 3); // "-99"
    assert_eq!(int_width(-100), 4); // "-100"
}

#[test]
fn test_int_width_boundary() {
    assert_eq!(int_width(i64::MAX), 19); // 9223372036854775807
    assert_eq!(int_width(i64::MIN), 20); // -9223372036854775808
}

// Float width tests

#[test]
#[expect(
    clippy::approx_constant,
    reason = "Testing width of literal 3.14, not using PI"
)]
fn test_float_width_simple() {
    assert_eq!(float_width(0.0), 1); // "0"
    assert_eq!(float_width(3.14), 4); // "3.14"
    assert_eq!(float_width(2.5), 3); // "2.5"
}

#[test]
fn test_float_width_negative() {
    assert_eq!(float_width(-1.5), 4); // "-1.5"
}

// Boolean width tests

#[test]
fn test_bool_width() {
    assert_eq!(bool_width(true), 4); // "true"
    assert_eq!(bool_width(false), 5); // "false"
}

// String width tests

#[test]
fn test_string_width_empty() {
    assert_eq!(string_width(""), 2); // '""'
}

#[test]
fn test_string_width_simple() {
    assert_eq!(string_width("hello"), 7); // '"hello"'
    assert_eq!(string_width("a"), 3); // '"a"'
}

#[test]
fn test_string_width_with_escapes() {
    // "a\nb": quotes = 2, 'a' = 1, '\n' = 2 (escaped), 'b' = 1 -> 6
    assert_eq!(string_width("a\nb"), 6);
    assert_eq!(string_width("\\"), 4); // '"\\"' -> 2 + 2
    assert_eq!(string_width("\""), 4); // '"\""' -> 2 + 2
}

// Char width tests

#[test]
fn test_char_width_regular() {
    assert_eq!(char_width('a'), 3); // "'a'"
    assert_eq!(char_width('z'), 3);
}

#[test]
fn test_char_width_escaped() {
    assert_eq!(char_width('\n'), 4); // "'\n'"
    assert_eq!(char_width('\\'), 4); // "'\\'"
    assert_eq!(char_width('\''), 4); // "'\\''"
    assert_eq!(char_width('\t'), 4);
    assert_eq!(char_width('\r'), 4);
    assert_eq!(char_width('\0'), 4);
}

// Multi-byte character width tests

#[test]
fn test_string_width_cjk() {
    // "世界" = 2 quotes + 2*2 = 6
    assert_eq!(string_width("世界"), 6);
    // "Hello, 世界!" = 2 quotes + 8 ASCII/punct + 2*2 CJK = 14
    // H(1) + e(1) + l(1) + l(1) + o(1) + ,(1) + space(1) + 世(2) + 界(2) + !(1) = 12 + 2 quotes
    assert_eq!(string_width("Hello, 世界!"), 14);
}

#[test]
fn test_string_width_emoji() {
    // "😀" = 2 quotes + 2 = 4
    assert_eq!(string_width("😀"), 4);
    // "Hi 😀!" = 2 quotes + H(1) + i(1) + space(1) + 😀(2) + !(1) = 8
    assert_eq!(string_width("Hi 😀!"), 8);
}

#[test]
fn test_string_width_mixed_scripts() {
    // "Café" = 2 quotes + 4 = 6 (é is single width)
    assert_eq!(string_width("Café"), 6);
}

#[test]
fn test_char_width_cjk() {
    // '世' = 2 quotes + 2 = 4
    assert_eq!(char_width('世'), 4);
}

#[test]
fn test_char_width_emoji() {
    // '😀' = 2 quotes + 2 = 4
    assert_eq!(char_width('😀'), 4);
}
