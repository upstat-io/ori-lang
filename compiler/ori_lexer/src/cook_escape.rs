//! Spec-strict escape processing for the V2 cooking layer.
//!
//! Each literal context (string, char, template) has its own valid escape set
//! per the grammar specification. Invalid escapes push errors into the
//! accumulator rather than panicking.
//!
//! # Grammar Reference
//!
//! - String escapes (line 102): `\"` `\\` `\n` `\t` `\r` `\0`
//! - Char escapes (line 127): `\'` `\\` `\n` `\t` `\r` `\0`
//! - Template escapes (line 107): `` \` `` `\\` `\n` `\t` `\r` `\0`
//! - Template braces (line 108): `{{` → `{`, `}}` → `}`

use crate::lex_error::LexError;
use ori_ir::Span;

/// Resolve a common escape character (shared across all contexts).
///
/// Returns `Some(char)` for escapes valid in all contexts: `\\` `\n` `\t` `\r` `\0`.
#[inline]
fn resolve_common_escape(c: char) -> Option<char> {
    match c {
        '\\' => Some('\\'),
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '0' => Some('\0'),
        _ => None,
    }
}

/// Unescape a string literal's content (between the `"`s).
///
/// Valid escapes per grammar line 102: `\"` `\\` `\n` `\t` `\r` `\0`.
/// `\'` is **not** valid in strings — a `SingleQuoteEscapeInString` error is pushed.
///
/// Fast path: if no backslashes, returns `None` to signal the caller can
/// intern the source slice directly.
#[allow(
    clippy::cast_possible_truncation,
    reason = "source offsets bounded by u32 — entire source file < u32::MAX bytes"
)]
pub(crate) fn unescape_string_v2(
    content: &str,
    base_offset: u32,
    errors: &mut Vec<LexError>,
) -> Option<String> {
    if !content.contains('\\') {
        return None;
    }

    let mut result = String::with_capacity(content.len());
    let mut chars = content.char_indices();

    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some((_, '"')) => result.push('"'),
                Some((j, '\'')) => {
                    // \' is NOT valid in strings per grammar line 102
                    let esc_start = base_offset + i as u32;
                    let esc_end = base_offset + j as u32 + 1;
                    errors.push(LexError::single_quote_escape_in_string(Span::new(
                        esc_start, esc_end,
                    )));
                    // Use the literal quote as replacement
                    result.push('\'');
                }
                Some((j, esc)) => {
                    if let Some(resolved) = resolve_common_escape(esc) {
                        result.push(resolved);
                    } else {
                        let esc_start = base_offset + i as u32;
                        let esc_end = base_offset + j as u32 + esc.len_utf8() as u32;
                        errors.push(LexError::invalid_string_escape(
                            Span::new(esc_start, esc_end),
                            esc,
                        ));
                        // Use replacement character for invalid escapes
                        result.push('\u{FFFD}');
                    }
                }
                None => {
                    // Trailing backslash
                    let esc_start = base_offset + i as u32;
                    errors.push(LexError::invalid_string_escape(
                        Span::new(esc_start, esc_start + 1),
                        '\\',
                    ));
                    result.push('\\');
                }
            }
        } else {
            result.push(c);
        }
    }

    Some(result)
}

/// Unescape a char literal's content (between the `'`s).
///
/// Valid escapes per grammar line 127: `\'` `\\` `\n` `\t` `\r` `\0`.
/// `\"` is **not** valid in char literals.
#[allow(
    clippy::cast_possible_truncation,
    reason = "source offsets bounded by u32 — entire source file < u32::MAX bytes"
)]
pub(crate) fn unescape_char_v2(
    content: &str,
    base_offset: u32,
    errors: &mut Vec<LexError>,
) -> char {
    let mut chars = content.chars();
    match chars.next() {
        Some('\\') => match chars.next() {
            Some('\'') => '\'',
            Some('"') => {
                // \" is NOT valid in char literals per grammar line 127
                errors.push(LexError::double_quote_escape_in_char(Span::new(
                    base_offset,
                    base_offset + 2,
                )));
                '"'
            }
            Some(esc) => {
                if let Some(resolved) = resolve_common_escape(esc) {
                    resolved
                } else {
                    errors.push(LexError::invalid_char_escape(
                        Span::new(base_offset, base_offset + 1 + esc.len_utf8() as u32),
                        esc,
                    ));
                    '\u{FFFD}'
                }
            }
            None => {
                errors.push(LexError::invalid_char_escape(
                    Span::new(base_offset, base_offset + 1),
                    '\\',
                ));
                '\\'
            }
        },
        Some(c) => c,
        None => {
            // Empty char literal — shouldn't happen with valid raw tokens
            '\0'
        }
    }
}

/// Unescape a template literal's content (between delimiters).
///
/// Valid escapes per grammar line 107: `` \` `` `\\` `\n` `\t` `\r` `\0`.
/// Brace escapes per grammar line 108: `{{` → `{`, `}}` → `}`.
///
/// Fast path: if no backslashes and no consecutive braces, returns `None`
/// to signal the caller can intern the source slice directly.
#[allow(
    clippy::cast_possible_truncation,
    reason = "source offsets bounded by u32 — entire source file < u32::MAX bytes"
)]
pub(crate) fn unescape_template_v2(
    content: &str,
    base_offset: u32,
    errors: &mut Vec<LexError>,
) -> Option<String> {
    // Fast path: check if any processing is needed
    let needs_unescape = content.contains('\\');
    let needs_brace_unescape = content.contains("{{") || content.contains("}}");
    if !needs_unescape && !needs_brace_unescape {
        return None;
    }

    let mut result = String::with_capacity(content.len());
    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            // Get the next char (could be multi-byte)
            let rest = &content[i + 1..];
            if let Some(esc) = rest.chars().next() {
                match esc {
                    '`' => {
                        result.push('`');
                        i += 1 + esc.len_utf8();
                    }
                    _ => {
                        if let Some(resolved) = resolve_common_escape(esc) {
                            result.push(resolved);
                            i += 1 + esc.len_utf8();
                        } else {
                            let esc_start = base_offset + i as u32;
                            let esc_end = esc_start + 1 + esc.len_utf8() as u32;
                            errors.push(LexError::invalid_template_escape(
                                Span::new(esc_start, esc_end),
                                esc,
                            ));
                            result.push('\u{FFFD}');
                            i += 1 + esc.len_utf8();
                        }
                    }
                }
            } else {
                // Trailing backslash
                let esc_start = base_offset + i as u32;
                errors.push(LexError::invalid_template_escape(
                    Span::new(esc_start, esc_start + 1),
                    '\\',
                ));
                result.push('\\');
                i += 1;
            }
        } else if b == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            result.push('{');
            i += 2;
        } else if b == b'}' && i + 1 < bytes.len() && bytes[i + 1] == b'}' {
            result.push('}');
            i += 2;
        } else {
            // Regular character — figure out its UTF-8 length
            let ch = content[i..].chars().next().unwrap_or('\0');
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // === String escapes ===

    #[test]
    fn string_no_escapes_fast_path() {
        let mut errors = Vec::new();
        assert!(unescape_string_v2("hello world", 0, &mut errors).is_none());
        assert!(errors.is_empty());
    }

    #[test]
    fn string_valid_escapes() {
        let mut errors = Vec::new();
        let result = unescape_string_v2(r"hello\nworld", 0, &mut errors);
        assert_eq!(result.as_deref(), Some("hello\nworld"));
        assert!(errors.is_empty());
    }

    #[test]
    fn string_all_valid_escapes() {
        let mut errors = Vec::new();
        let result = unescape_string_v2(r#"\"\\\n\t\r\0"#, 0, &mut errors);
        assert_eq!(result.as_deref(), Some("\"\\\n\t\r\0"));
        assert!(errors.is_empty());
    }

    #[test]
    fn string_single_quote_escape_is_error() {
        let mut errors = Vec::new();
        let result = unescape_string_v2(r"hello\'world", 1, &mut errors);
        assert_eq!(result.as_deref(), Some("hello'world"));
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].kind,
            crate::lex_error::LexErrorKind::SingleQuoteEscapeInString
        );
        // Escape starts at offset 1+5=6 (\) to 1+6+1=8 (')
        assert_eq!(errors[0].span, Span::new(6, 8));
    }

    #[test]
    fn string_invalid_escape() {
        let mut errors = Vec::new();
        let result = unescape_string_v2(r"\q", 0, &mut errors);
        assert_eq!(result.as_deref(), Some("\u{FFFD}"));
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0].kind,
            crate::lex_error::LexErrorKind::InvalidStringEscape { escape_char: 'q' }
        ));
    }

    #[test]
    fn string_trailing_backslash() {
        let mut errors = Vec::new();
        let result = unescape_string_v2("test\\", 0, &mut errors);
        assert_eq!(result.as_deref(), Some("test\\"));
        assert_eq!(errors.len(), 1);
    }

    // === Char escapes ===

    #[test]
    fn char_simple() {
        let mut errors = Vec::new();
        assert_eq!(unescape_char_v2("a", 0, &mut errors), 'a');
        assert!(errors.is_empty());
    }

    #[test]
    fn char_valid_escapes() {
        let mut errors = Vec::new();
        assert_eq!(unescape_char_v2(r"\'", 0, &mut errors), '\'');
        assert!(errors.is_empty());

        assert_eq!(unescape_char_v2(r"\\", 0, &mut errors), '\\');
        assert_eq!(unescape_char_v2(r"\n", 0, &mut errors), '\n');
        assert_eq!(unescape_char_v2(r"\t", 0, &mut errors), '\t');
        assert_eq!(unescape_char_v2(r"\r", 0, &mut errors), '\r');
        assert_eq!(unescape_char_v2(r"\0", 0, &mut errors), '\0');
        assert!(errors.is_empty());
    }

    #[test]
    fn char_double_quote_escape_is_error() {
        let mut errors = Vec::new();
        let result = unescape_char_v2(r#"\""#, 1, &mut errors);
        assert_eq!(result, '"');
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].kind,
            crate::lex_error::LexErrorKind::DoubleQuoteEscapeInChar
        );
    }

    #[test]
    fn char_invalid_escape() {
        let mut errors = Vec::new();
        let result = unescape_char_v2(r"\q", 0, &mut errors);
        assert_eq!(result, '\u{FFFD}');
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn char_unicode() {
        let mut errors = Vec::new();
        assert_eq!(unescape_char_v2("λ", 0, &mut errors), 'λ');
        assert!(errors.is_empty());
    }

    #[test]
    fn char_empty() {
        let mut errors = Vec::new();
        assert_eq!(unescape_char_v2("", 0, &mut errors), '\0');
    }

    // === Template escapes ===

    #[test]
    fn template_no_escapes_fast_path() {
        let mut errors = Vec::new();
        assert!(unescape_template_v2("hello world", 0, &mut errors).is_none());
        assert!(errors.is_empty());
    }

    #[test]
    fn template_backtick_escape() {
        let mut errors = Vec::new();
        let result = unescape_template_v2(r"hello\`world", 0, &mut errors);
        assert_eq!(result.as_deref(), Some("hello`world"));
        assert!(errors.is_empty());
    }

    #[test]
    fn template_common_escapes() {
        let mut errors = Vec::new();
        let result = unescape_template_v2(r"\\\n\t\r\0", 0, &mut errors);
        assert_eq!(result.as_deref(), Some("\\\n\t\r\0"));
        assert!(errors.is_empty());
    }

    #[test]
    fn template_brace_escapes() {
        let mut errors = Vec::new();
        let result = unescape_template_v2("hello{{world}}", 0, &mut errors);
        assert_eq!(result.as_deref(), Some("hello{world}"));
        assert!(errors.is_empty());
    }

    #[test]
    fn template_invalid_escape() {
        let mut errors = Vec::new();
        let result = unescape_template_v2(r"\q", 0, &mut errors);
        assert_eq!(result.as_deref(), Some("\u{FFFD}"));
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn template_mixed_escapes_and_braces() {
        let mut errors = Vec::new();
        let result = unescape_template_v2(r"a\nb{{c}}", 0, &mut errors);
        assert_eq!(result.as_deref(), Some("a\nb{c}"));
        assert!(errors.is_empty());
    }

    #[test]
    fn template_trailing_single_brace() {
        // A single { should pass through (it would be part of interpolation in real use)
        let mut errors = Vec::new();
        let result = unescape_template_v2("a{b", 0, &mut errors);
        // No backslashes, no double braces — fast path
        assert!(result.is_none());
        assert!(errors.is_empty());
    }
}
