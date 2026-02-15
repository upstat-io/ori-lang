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
mod tests;
