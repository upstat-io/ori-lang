use super::*;

#[test]
fn test_decimal_digit_count_zero() {
    assert_eq!(decimal_digit_count(0), 1);
}

#[test]
fn test_decimal_digit_count_single_digit() {
    assert_eq!(decimal_digit_count(1), 1);
    assert_eq!(decimal_digit_count(9), 1);
}

#[test]
fn test_decimal_digit_count_multi_digit() {
    assert_eq!(decimal_digit_count(10), 2);
    assert_eq!(decimal_digit_count(99), 2);
    assert_eq!(decimal_digit_count(100), 3);
    assert_eq!(decimal_digit_count(999), 3);
    assert_eq!(decimal_digit_count(1000), 4);
    assert_eq!(decimal_digit_count(123_456), 6);
}

#[test]
fn test_decimal_digit_count_large() {
    assert_eq!(decimal_digit_count(u64::MAX), 20); // 18446744073709551615
}

#[test]
fn test_accumulate_widths_empty() {
    let items: Vec<usize> = vec![];
    assert_eq!(accumulate_widths(&items, |&w| w, COMMA_SEPARATOR_WIDTH), 0);
}

#[test]
fn test_accumulate_widths_single() {
    let items = vec![5];
    assert_eq!(accumulate_widths(&items, |&w| w, COMMA_SEPARATOR_WIDTH), 5);
}

#[test]
fn test_accumulate_widths_multiple() {
    let items = vec![1, 2, 3];
    // 1 + 2 + 2 + 2 + 3 = 10 (two separators of width 2)
    assert_eq!(accumulate_widths(&items, |&w| w, COMMA_SEPARATOR_WIDTH), 10);
}

#[test]
fn test_accumulate_widths_always_stacked_propagation() {
    let items = vec![1, ALWAYS_STACKED, 3];
    assert_eq!(
        accumulate_widths(&items, |&w| w, COMMA_SEPARATOR_WIDTH),
        ALWAYS_STACKED
    );
}

// Character display width tests

#[test]
fn test_char_display_width_ascii() {
    assert_eq!(char_display_width('a'), 1);
    assert_eq!(char_display_width('Z'), 1);
    assert_eq!(char_display_width('0'), 1);
    assert_eq!(char_display_width(' '), 1);
    assert_eq!(char_display_width('!'), 1);
}

#[test]
fn test_char_display_width_latin_extended() {
    assert_eq!(char_display_width('é'), 1);
    assert_eq!(char_display_width('ñ'), 1);
    assert_eq!(char_display_width('ü'), 1);
}

#[test]
fn test_char_display_width_cjk() {
    // CJK Unified Ideographs
    assert_eq!(char_display_width('世'), 2);
    assert_eq!(char_display_width('界'), 2);
    assert_eq!(char_display_width('中'), 2);
    assert_eq!(char_display_width('文'), 2);
}

#[test]
fn test_char_display_width_japanese() {
    // Hiragana
    assert_eq!(char_display_width('あ'), 2);
    assert_eq!(char_display_width('か'), 2);
    // Katakana
    assert_eq!(char_display_width('ア'), 2);
    assert_eq!(char_display_width('カ'), 2);
}

#[test]
fn test_char_display_width_hangul() {
    assert_eq!(char_display_width('한'), 2);
    assert_eq!(char_display_width('글'), 2);
}

#[test]
fn test_char_display_width_emoji() {
    assert_eq!(char_display_width('😀'), 2);
    assert_eq!(char_display_width('🎉'), 2);
    assert_eq!(char_display_width('❤'), 2);
}

#[test]
fn test_char_display_width_fullwidth() {
    // Fullwidth Latin
    assert_eq!(char_display_width('Ａ'), 2);
    assert_eq!(char_display_width('！'), 2);
}

#[test]
fn test_char_display_width_combining_marks() {
    // Combining diacritical marks (zero width)
    assert_eq!(char_display_width('\u{0301}'), 0); // Combining acute accent
    assert_eq!(char_display_width('\u{0308}'), 0); // Combining diaeresis
}

#[test]
fn test_char_display_width_zero_width_chars() {
    assert_eq!(char_display_width('\u{200B}'), 0); // Zero-width space
    assert_eq!(char_display_width('\u{200D}'), 0); // Zero-width joiner
    assert_eq!(char_display_width('\u{FEFF}'), 0); // BOM
}
