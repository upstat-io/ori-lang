//! Shared width calculation utilities.
//!
//! This module provides common helper functions used across the width calculation
//! system: digit counting for numeric literals, character width calculation for
//! multi-byte characters, and accumulation of widths with separators.

use super::ALWAYS_STACKED;

/// Calculate the display width of a Unicode character.
///
/// Based on Unicode Standard Annex #11 (East Asian Width):
/// - Width 0: Combining marks, zero-width characters
/// - Width 2: CJK ideographs, fullwidth forms, most emoji
/// - Width 1: Everything else (ASCII, Latin, etc.)
///
/// This is a simplified implementation covering common cases without
/// requiring external dependencies.
#[inline]
pub(super) fn char_display_width(c: char) -> usize {
    let cp = c as u32;

    // Zero-width characters
    if is_zero_width(cp) {
        return 0;
    }

    // Double-width characters
    if is_double_width(cp) {
        return 2;
    }

    // Default: single width
    1
}

/// Check if a codepoint is zero-width.
#[inline]
fn is_zero_width(cp: u32) -> bool {
    // Combining marks and modifiers
    matches!(cp,
        // Combining Diacritical Marks
        0x0300..=0x036F |
        // Combining Diacritical Marks Extended
        0x1AB0..=0x1AFF |
        // Combining Diacritical Marks Supplement
        0x1DC0..=0x1DFF |
        // Combining Diacritical Marks for Symbols
        0x20D0..=0x20FF |
        // Combining Half Marks
        0xFE20..=0xFE2F |
        // Zero-width characters
        0x200B..=0x200F | // ZWSP, ZWNJ, ZWJ, LRM, RLM
        0x2060..=0x2064 | // Word joiner, invisible operators
        0xFEFF           // BOM / ZWNBSP
    )
}

/// Check if a codepoint is double-width.
#[inline]
fn is_double_width(cp: u32) -> bool {
    matches!(cp,
        // CJK Unified Ideographs Extension A
        0x3400..=0x4DBF |
        // CJK Unified Ideographs
        0x4E00..=0x9FFF |
        // CJK Compatibility Ideographs
        0xF900..=0xFAFF |
        // CJK Unified Ideographs Extension B-G
        0x20000..=0x2FFFF |
        // Hangul Syllables
        0xAC00..=0xD7A3 |
        // Fullwidth Forms
        0xFF01..=0xFF60 |
        0xFFE0..=0xFFE6 |
        // Hiragana and Katakana
        0x3040..=0x30FF |
        // Bopomofo
        0x3100..=0x312F |
        // CJK Symbols and Punctuation
        0x3000..=0x303F |
        // Enclosed CJK Letters and Months
        0x3200..=0x32FF |
        // CJK Compatibility
        0x3300..=0x33FF |
        // Most emoji (Miscellaneous Symbols and Pictographs onward)
        0x1F300..=0x1F9FF |
        // Supplemental Symbols and Pictographs
        0x1FA00..=0x1FAFF |
        // Symbols and Pictographs Extended-A
        0x1FB00..=0x1FBFF |
        // Additional emoji ranges
        0x2600..=0x26FF | // Miscellaneous Symbols
        0x2700..=0x27BF | // Dingbats
        0x231A..=0x231B | // Watch, Hourglass
        0x23E9..=0x23F3 | // Various symbols
        0x23F8..=0x23FA   // Pause, etc.
    )
}

/// Separator width for comma-separated items: ", " = 2 characters.
pub(super) const COMMA_SEPARATOR_WIDTH: usize = 2;

/// Count decimal digits in a non-negative integer.
///
/// Returns the number of digits needed to represent `n` in base 10.
/// For `n == 0`, returns 1 (representing "0").
#[inline]
pub(super) fn decimal_digit_count(n: u64) -> usize {
    if n == 0 {
        return 1;
    }
    (n.ilog10() + 1) as usize
}

/// Accumulate widths with a separator between items.
///
/// Returns `ALWAYS_STACKED` if any item's width is `ALWAYS_STACKED`,
/// ensuring that stacked constructs propagate through containers.
///
/// # Arguments
///
/// * `items` - The items to measure
/// * `get_width` - Function to get the width of each item
/// * `separator_width` - Width of separator between items (e.g., 2 for ", ")
pub(super) fn accumulate_widths<T, F>(
    items: &[T],
    mut get_width: F,
    separator_width: usize,
) -> usize
where
    F: FnMut(&T) -> usize,
{
    if items.is_empty() {
        return 0;
    }

    let mut total = 0;
    for (i, item) in items.iter().enumerate() {
        let w = get_width(item);
        if w == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        total += w;
        if i < items.len() - 1 {
            total += separator_width;
        }
    }
    total
}

#[cfg(test)]
mod tests {
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
}
