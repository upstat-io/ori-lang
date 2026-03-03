//! Spec-strict escape processing for the V2 cooking layer.
//!
//! Each literal context (string, char, template) has its own valid escape set
//! per the grammar specification. Invalid escapes push errors into the
//! accumulator rather than panicking.
//!
//! # Grammar Reference
//!
//! - String escapes (line 102): `\"` `\\` `\n` `\t` `\r` `\0` `\u{...}`
//! - Char escapes (line 127): `\'` `\\` `\n` `\t` `\r` `\0` `\u{...}`
//! - Template escapes (line 107): `` \` `` `\\` `\n` `\t` `\r` `\0` `\u{...}`
//! - Template braces (line 108): `{{` → `{`, `}}` → `}`
//! - Unicode escapes (line 111): `\u{` `hex_digit` { `hex_digit` } `}`

use crate::lex_error::{LexError, LexErrorContext, UnicodeEscapeDetail};
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

/// Parse the body of a `\u{...}` Unicode escape from a string slice.
///
/// `content` starts at the `u` character (immediately after `\`).
/// `backslash_offset` is the absolute source offset of the `\` character.
/// `context` is the literal context for error reporting.
///
/// Returns `(resolved_char, bytes_consumed)` where `bytes_consumed` counts
/// from the start of `content` (the `u`) through the closing `}` inclusive.
/// On error, returns `'\u{FFFD}'` (replacement character) and the number of
/// bytes consumed greedily.
#[allow(
    clippy::cast_possible_truncation,
    reason = "source offsets bounded by u32 — entire source file < u32::MAX bytes"
)]
fn parse_unicode_escape(
    content: &str,
    backslash_offset: u32,
    context: LexErrorContext,
    errors: &mut Vec<LexError>,
) -> (char, usize) {
    let bytes = content.as_bytes();
    debug_assert!(bytes.first() == Some(&b'u'), "content must start with 'u'");

    // After 'u', expect '{'
    if bytes.len() < 2 || bytes[1] != b'{' {
        let span = Span::new(backslash_offset, backslash_offset + 2);
        errors.push(LexError::invalid_unicode_escape(
            span,
            UnicodeEscapeDetail::MissingOpenBrace,
            context,
        ));
        return ('\u{FFFD}', 1); // consumed only 'u'
    }

    // Scan hex digits after '{'
    let mut i = 2; // past 'u{'
    let digit_start = i;
    while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
        i += 1;
    }
    let digit_count = i - digit_start;

    // Check for invalid characters before '}'
    if i < bytes.len() && bytes[i] != b'}' {
        let ch = content[i..].chars().next().unwrap_or('?');
        let ch_offset = backslash_offset + 1 + i as u32;
        let span = Span::new(ch_offset, ch_offset + ch.len_utf8() as u32);
        errors.push(LexError::invalid_unicode_escape(
            span,
            UnicodeEscapeDetail::InvalidHexDigit { ch },
            context,
        ));
        // Greedy recovery: skip past this char and any remaining content to '}'
        let mut j = i + ch.len_utf8();
        while j < bytes.len() && bytes[j] != b'}' {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b'}' {
            j += 1;
        }
        return ('\u{FFFD}', j);
    }

    // Check for empty digits
    if digit_count == 0 {
        let consumed = if i < bytes.len() && bytes[i] == b'}' {
            i + 1 // consume '}'
        } else {
            i
        };
        let span = Span::new(backslash_offset, backslash_offset + 1 + consumed as u32);
        errors.push(LexError::invalid_unicode_escape(
            span,
            UnicodeEscapeDetail::EmptyDigits,
            context,
        ));
        return ('\u{FFFD}', consumed);
    }

    // Check for too many digits
    if digit_count > 6 {
        let consumed = if i < bytes.len() && bytes[i] == b'}' {
            i + 1
        } else {
            i
        };
        let span = Span::new(backslash_offset, backslash_offset + 1 + consumed as u32);
        errors.push(LexError::invalid_unicode_escape(
            span,
            UnicodeEscapeDetail::TooManyDigits,
            context,
        ));
        return ('\u{FFFD}', consumed);
    }

    // Check for missing closing brace
    if i >= bytes.len() || bytes[i] != b'}' {
        let span = Span::new(backslash_offset, backslash_offset + 1 + i as u32);
        errors.push(LexError::invalid_unicode_escape(
            span,
            UnicodeEscapeDetail::MissingCloseBrace,
            context,
        ));
        return ('\u{FFFD}', i);
    }

    // Consume closing '}'
    let consumed = i + 1;
    let hex_str = &content[digit_start..i];

    // Parse hex value — safe because we validated all digits are hex
    let codepoint = u32::from_str_radix(hex_str, 16).unwrap_or(u32::MAX);

    // Check for surrogate codepoints (U+D800–U+DFFF)
    if (0xD800..=0xDFFF).contains(&codepoint) {
        let span = Span::new(backslash_offset, backslash_offset + 1 + consumed as u32);
        errors.push(LexError::invalid_unicode_escape(
            span,
            UnicodeEscapeDetail::SurrogateCodepoint { codepoint },
            context,
        ));
        return ('\u{FFFD}', consumed);
    }

    // Convert to char
    if let Some(c) = char::from_u32(codepoint) {
        (c, consumed)
    } else {
        let span = Span::new(backslash_offset, backslash_offset + 1 + consumed as u32);
        errors.push(LexError::invalid_unicode_escape(
            span,
            UnicodeEscapeDetail::OutOfRange { codepoint },
            context,
        ));
        ('\u{FFFD}', consumed)
    }
}

/// Unescape a string literal's content (between the `"`s).
///
/// Valid escapes per grammar line 102: `\"` `\\` `\n` `\t` `\r` `\0` `\u{...}`.
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
    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let rest = &content[i + 1..];
            if let Some(esc) = rest.chars().next() {
                match esc {
                    '"' => {
                        result.push('"');
                        i += 2;
                    }
                    '\'' => {
                        let esc_start = base_offset + i as u32;
                        let esc_end = esc_start + 2;
                        errors.push(LexError::single_quote_escape_in_string(Span::new(
                            esc_start, esc_end,
                        )));
                        result.push('\'');
                        i += 2;
                    }
                    'u' => {
                        let backslash_offset = base_offset + i as u32;
                        let context = LexErrorContext::InsideString { start: base_offset };
                        let (resolved, bytes_consumed) = parse_unicode_escape(
                            &content[i + 1..],
                            backslash_offset,
                            context,
                            errors,
                        );
                        result.push(resolved);
                        i += 1 + bytes_consumed;
                    }
                    _ => {
                        if let Some(resolved) = resolve_common_escape(esc) {
                            result.push(resolved);
                        } else {
                            let esc_start = base_offset + i as u32;
                            let esc_end = esc_start + 1 + esc.len_utf8() as u32;
                            errors.push(LexError::invalid_string_escape(
                                Span::new(esc_start, esc_end),
                                esc,
                            ));
                            result.push('\u{FFFD}');
                        }
                        i += 1 + esc.len_utf8();
                    }
                }
            } else {
                // Trailing backslash
                let esc_start = base_offset + i as u32;
                errors.push(LexError::invalid_string_escape(
                    Span::new(esc_start, esc_start + 1),
                    '\\',
                ));
                result.push('\\');
                i += 1;
            }
        } else {
            let ch = content[i..].chars().next().unwrap_or('\0');
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    Some(result)
}

/// Unescape a char literal's content (between the `'`s).
///
/// Valid escapes per grammar line 127: `\'` `\\` `\n` `\t` `\r` `\0` `\u{...}`.
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
    let mut chars = content.char_indices();
    match chars.next() {
        Some((_, '\\')) => match chars.next() {
            Some((_, '\'')) => '\'',
            Some((_, '"')) => {
                // \" is NOT valid in char literals per grammar line 127
                errors.push(LexError::double_quote_escape_in_char(Span::new(
                    base_offset,
                    base_offset + 2,
                )));
                '"'
            }
            Some((j, 'u')) => {
                let context = LexErrorContext::InsideChar;
                let (resolved, _) =
                    parse_unicode_escape(&content[j..], base_offset, context, errors);
                resolved
            }
            Some((_, esc)) => {
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
        Some((_, c)) => c,
        None => {
            // Empty char literal — shouldn't happen with valid raw tokens
            '\0'
        }
    }
}

/// Unescape a template literal's content (between delimiters).
///
/// Valid escapes per grammar line 107: `` \` `` `\\` `\n` `\t` `\r` `\0` `\u{...}`.
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
                    'u' => {
                        let backslash_offset = base_offset + i as u32;
                        let context = LexErrorContext::InsideTemplate {
                            start: base_offset,
                            nesting: 0,
                        };
                        let (resolved, bytes_consumed) = parse_unicode_escape(
                            &content[i + 1..],
                            backslash_offset,
                            context,
                            errors,
                        );
                        result.push(resolved);
                        i += 1 + bytes_consumed; // 1 for '\', bytes_consumed for 'u{...}'
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
