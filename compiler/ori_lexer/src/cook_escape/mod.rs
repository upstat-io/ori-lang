//! Spec-strict escape processing for the V2 cooking layer.
//!
//! Each literal context (string, char, template) has its own valid escape set
//! per the grammar specification. Invalid escapes push errors into the
//! accumulator rather than panicking.
//!
//! # Architecture
//!
//! `unescape_string_v2` and `unescape_template_v2` are thin wrappers around
//! `unescape_with_context`, which implements the shared scanning loop
//! parameterized by [`EscapeContext`]. `unescape_char_v2` is structurally
//! different (single-char, returns `char`) and remains standalone.
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

/// Escape processing context, parameterizing the shared unescape loop.
///
/// Each variant encapsulates the policy differences between string and template
/// escape processing: fast-path predicate, context-specific escapes, brace
/// handling, error factories, and `LexErrorContext` construction.
///
/// Adding a new escape form (e.g., `\xHH`) requires one new match arm in
/// [`unescape_with_context`] — the context enum keeps all dispatch in one place.
#[derive(Clone, Copy)]
enum EscapeContext {
    String,
    Template,
}

impl EscapeContext {
    /// Check if content needs escape processing (fast-path bypass).
    ///
    /// String: only backslashes trigger processing.
    /// Template: backslashes OR adjacent brace pairs (`{{`/`}}`) trigger processing.
    fn needs_processing(self, content: &str) -> bool {
        let bytes = content.as_bytes();
        match self {
            EscapeContext::String => bytes.contains(&b'\\'),
            EscapeContext::Template => {
                bytes.contains(&b'\\')
                    || bytes
                        .windows(2)
                        .any(|w| (w[0] == b'{' && w[1] == b'{') || (w[0] == b'}' && w[1] == b'}'))
            }
        }
    }

    /// Build the `LexErrorContext` for unicode escape error reporting.
    fn make_error_context(self, base_offset: u32) -> LexErrorContext {
        match self {
            EscapeContext::String => LexErrorContext::InsideString { start: base_offset },
            EscapeContext::Template => LexErrorContext::InsideTemplate {
                start: base_offset,
                nesting: 0,
            },
        }
    }

    /// Push an invalid-escape error appropriate to this context.
    fn push_invalid_escape(self, errors: &mut Vec<LexError>, span: Span, esc: char) {
        match self {
            EscapeContext::String => errors.push(LexError::invalid_string_escape(span, esc)),
            EscapeContext::Template => errors.push(LexError::invalid_template_escape(span, esc)),
        }
    }
}

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
#[expect(
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

/// Shared unescape implementation for string and template literals.
///
/// The scanning loop is identical across both contexts — differences are
/// encapsulated in the [`EscapeContext`] policy methods and context-guarded
/// match arms. Adding a new escape form (e.g., `\xHH` for hex byte escapes)
/// requires exactly one new match arm here.
#[expect(
    clippy::cast_possible_truncation,
    reason = "source offsets bounded by u32 — entire source file < u32::MAX bytes"
)]
fn unescape_with_context(
    content: &str,
    base_offset: u32,
    errors: &mut Vec<LexError>,
    ctx: EscapeContext,
) -> Option<String> {
    if !ctx.needs_processing(content) {
        return None;
    }

    let mut result = String::with_capacity(content.len());
    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            let rest = &content[i + 1..];
            if let Some(esc) = rest.chars().next() {
                match esc {
                    // Context-specific delimiter escapes
                    '"' if matches!(ctx, EscapeContext::String) => {
                        result.push('"');
                        i += 2;
                    }
                    '`' if matches!(ctx, EscapeContext::Template) => {
                        result.push('`');
                        i += 2;
                    }
                    // String-specific error recovery: \' is invalid but pushes the char
                    '\'' if matches!(ctx, EscapeContext::String) => {
                        let esc_start = base_offset + i as u32;
                        errors.push(LexError::single_quote_escape_in_string(Span::new(
                            esc_start,
                            esc_start + 2,
                        )));
                        result.push('\'');
                        i += 2;
                    }
                    // Unicode escape (shared across all contexts)
                    'u' => {
                        let backslash_offset = base_offset + i as u32;
                        let context = ctx.make_error_context(base_offset);
                        let (resolved, bytes_consumed) = parse_unicode_escape(
                            &content[i + 1..],
                            backslash_offset,
                            context,
                            errors,
                        );
                        result.push(resolved);
                        i += 1 + bytes_consumed;
                    }
                    // Future extension point: \xHH hex byte escapes would go here
                    // as a new match arm (spec: 07-lexical-elements.md line 292,
                    // grammar.ebnf lines 116-118; tracked in roadmap 15C.13)

                    // Common escapes (\\ \n \t \r \0) + invalid escape fallback
                    _ => {
                        if let Some(resolved) = resolve_common_escape(esc) {
                            result.push(resolved);
                        } else {
                            let esc_start = base_offset + i as u32;
                            let esc_end = esc_start + 1 + esc.len_utf8() as u32;
                            ctx.push_invalid_escape(errors, Span::new(esc_start, esc_end), esc);
                            result.push('\u{FFFD}');
                        }
                        i += 1 + esc.len_utf8();
                    }
                }
            } else {
                // Trailing backslash
                let esc_start = base_offset + i as u32;
                ctx.push_invalid_escape(errors, Span::new(esc_start, esc_start + 1), '\\');
                result.push('\\');
                i += 1;
            }
        } else if matches!(ctx, EscapeContext::Template)
            && i + 1 < bytes.len()
            && ((b == b'{' && bytes[i + 1] == b'{') || (b == b'}' && bytes[i + 1] == b'}'))
        {
            // Template brace-pair collapsing: {{ → {, }} → } (spec line 108)
            result.push(b as char);
            i += 2;
        } else {
            // Regular character: ASCII fast path / multi-byte
            if b < 128 {
                result.push(b as char);
                i += 1;
            } else {
                let ch = content[i..].chars().next().unwrap_or('\0');
                result.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    Some(result)
}

/// Unescape a string literal's content (between the `"`s).
///
/// Valid escapes per grammar line 102: `\"` `\\` `\n` `\t` `\r` `\0` `\u{...}`.
/// `\'` is **not** valid in strings — a `SingleQuoteEscapeInString` error is pushed,
/// but the `'` character is still added to the output (error recovery).
///
/// Fast path: if no backslashes, returns `None` to signal the caller can
/// intern the source slice directly.
pub(crate) fn unescape_string_v2(
    content: &str,
    base_offset: u32,
    errors: &mut Vec<LexError>,
) -> Option<String> {
    unescape_with_context(content, base_offset, errors, EscapeContext::String)
}

/// Unescape a char literal's content (between the `'`s).
///
/// Valid escapes per grammar line 127: `\'` `\\` `\n` `\t` `\r` `\0` `\u{...}`.
/// `\"` is **not** valid in char literals.
#[expect(
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
pub(crate) fn unescape_template_v2(
    content: &str,
    base_offset: u32,
    errors: &mut Vec<LexError>,
) -> Option<String> {
    unescape_with_context(content, base_offset, errors, EscapeContext::Template)
}

#[cfg(test)]
mod tests;
