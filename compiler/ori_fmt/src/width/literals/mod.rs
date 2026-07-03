//! Width calculation for literal expressions.
//!
//! Provides width calculations for primitive literals:
//! - Integer literals (including negatives)
//! - Float literals (using stack buffer to avoid allocation)
//! - Boolean literals ("true" / "false")
//! - String literals (including escape sequences and multi-byte chars)
//! - Character literals (including escape sequences)

use super::metrics::{char_display_width, decimal_digit_count};
use crate::formatter::{char_escape, string_escape};

/// Calculate width of an integer literal.
///
/// Handles both positive and negative integers. The width includes
/// the minus sign for negative numbers.
pub(super) fn int_width(n: i64) -> usize {
    if n == 0 {
        return 1;
    }

    let abs_n = n.unsigned_abs();
    let digits = decimal_digit_count(abs_n);

    if n < 0 {
        digits + 1 // Include minus sign
    } else {
        digits
    }
}

/// Calculate width of a float literal.
///
/// Uses a stack-allocated buffer to avoid heap allocation from `format!`.
/// The buffer is sized to accommodate any f64 value's default formatting.
pub(super) fn float_width(f: f64) -> usize {
    use std::io::Write;

    // 32 bytes is sufficient for any f64 in default format
    // (max ~24 chars for scientific notation edge cases)
    let mut buf = [0u8; 32];
    let mut cursor = std::io::Cursor::new(&mut buf[..]);

    // Infallible: a 32-byte buffer holds any f64 default format.
    let _ = write!(cursor, "{f}");

    // Safe: cursor position is at most 32, which fits in usize on all platforms
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Buffer is 32 bytes; position cannot exceed usize::MAX"
    )]
    {
        cursor.position() as usize
    }
}

/// Calculate width of a boolean literal.
///
/// Returns 4 for "true", 5 for "false".
#[inline]
pub(super) fn bool_width(b: bool) -> usize {
    if b {
        4
    } else {
        5
    }
}

/// Calculate width of a string literal (including quotes).
///
/// Accounts for:
/// - Escape sequences which take 2 characters when rendered
/// - Multi-byte characters (CJK, emoji) which take 2 columns
/// - Zero-width characters (combining marks) which take 0 columns
pub(super) fn string_width(s: impl AsRef<str>) -> usize {
    let s = s.as_ref();
    let mut width = 2; // Opening and closing quotes
    for c in s.chars() {
        width += match string_escape(c) {
            Some(esc) => esc.len(),
            None => char_display_width(c),
        };
    }
    width
}

/// Calculate width of a char literal (including quotes).
///
/// Accounts for:
/// - Escape sequences which require 4 characters total: `'\n'`, `'\t'`, etc.
/// - Multi-byte characters (CJK, emoji) which take 2 + quotes = 4 columns
/// - Regular characters require 3 characters: `'a'`
pub(super) fn char_width(c: char) -> usize {
    match char_escape(c) {
        Some(esc) => 2 + esc.len(),        // quotes + escaped form
        None => 2 + char_display_width(c), // quotes + display width
    }
}

#[cfg(test)]
mod tests;
