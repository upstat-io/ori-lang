use super::*;

// String escapes

#[test]
fn string_no_escapes_fast_path() {
    let mut errors = Vec::new();
    assert!(unescape_string("hello world", 0, &mut errors).is_none());
    assert!(errors.is_empty());
}

#[test]
fn string_valid_escapes() {
    let mut errors = Vec::new();
    let result = unescape_string(r"hello\nworld", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("hello\nworld"));
    assert!(errors.is_empty());
}

#[test]
fn string_all_valid_escapes() {
    let mut errors = Vec::new();
    let result = unescape_string(r#"\"\\\n\t\r\0"#, 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\"\\\n\t\r\0"));
    assert!(errors.is_empty());
}

#[test]
fn string_single_quote_escape_is_error() {
    let mut errors = Vec::new();
    let result = unescape_string(r"hello\'world", 1, &mut errors);
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
    let result = unescape_string(r"\q", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidStringEscape { escape_char: 'q' }
    ));
}

// Negative pin: `\xHH` hex byte escapes are NOT yet implemented (Spec: Clause 7,
// grammar.ebnf `hex_escape`; tracked by the cook_escape::mod TODO(lexer)). Until
// implemented, `\x` is rejected as an ordinary invalid escape. These pins clamp the
// current "not implemented" boundary so the future implementer's accept tests flip
// these from rejection — stale-test detection.

#[test]
fn string_hex_byte_escape_not_yet_implemented_is_invalid() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\x41", 0, &mut errors);
    // `\x` rejected as invalid escape; the `41` after it passes through literally.
    assert_eq!(result.as_deref(), Some("\u{FFFD}41"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidStringEscape { escape_char: 'x' }
    ));
}

#[test]
fn char_hex_byte_escape_not_yet_implemented_is_invalid() {
    let mut errors = Vec::new();
    let result = unescape_char(r"\x41", 0, &mut errors);
    // `\x` rejected as invalid char escape; replacement char returned.
    assert_eq!(result, '\u{FFFD}');
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidCharEscape { escape_char: 'x' }
    ));
}

#[test]
fn template_hex_byte_escape_not_yet_implemented_is_invalid() {
    let mut errors = Vec::new();
    let result = unescape_template(r"\x41", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}41"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidTemplateEscape { escape_char: 'x' }
    ));
}

#[test]
fn string_trailing_backslash() {
    let mut errors = Vec::new();
    let result = unescape_string("test\\", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("test\\"));
    assert_eq!(errors.len(), 1);
}

// Char escapes

#[test]
fn char_simple() {
    let mut errors = Vec::new();
    assert_eq!(unescape_char("a", 0, &mut errors), 'a');
    assert!(errors.is_empty());
}

#[test]
fn char_valid_escapes() {
    let mut errors = Vec::new();
    assert_eq!(unescape_char(r"\'", 0, &mut errors), '\'');
    assert!(errors.is_empty());

    assert_eq!(unescape_char(r"\\", 0, &mut errors), '\\');
    assert_eq!(unescape_char(r"\n", 0, &mut errors), '\n');
    assert_eq!(unescape_char(r"\t", 0, &mut errors), '\t');
    assert_eq!(unescape_char(r"\r", 0, &mut errors), '\r');
    assert_eq!(unescape_char(r"\0", 0, &mut errors), '\0');
    assert!(errors.is_empty());
}

#[test]
fn char_double_quote_escape_is_error() {
    let mut errors = Vec::new();
    let result = unescape_char(r#"\""#, 1, &mut errors);
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
    let result = unescape_char(r"\q", 0, &mut errors);
    assert_eq!(result, '\u{FFFD}');
    assert_eq!(errors.len(), 1);
}

#[test]
fn char_unicode() {
    let mut errors = Vec::new();
    assert_eq!(unescape_char("λ", 0, &mut errors), 'λ');
    assert!(errors.is_empty());
}

#[test]
fn char_empty() {
    let mut errors = Vec::new();
    assert_eq!(unescape_char("", 0, &mut errors), '\0');
}

// Template escapes

#[test]
fn template_no_escapes_fast_path() {
    let mut errors = Vec::new();
    assert!(unescape_template("hello world", 0, &mut errors).is_none());
    assert!(errors.is_empty());
}

#[test]
fn template_backtick_escape() {
    let mut errors = Vec::new();
    let result = unescape_template(r"hello\`world", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("hello`world"));
    assert!(errors.is_empty());
}

#[test]
fn template_common_escapes() {
    let mut errors = Vec::new();
    let result = unescape_template(r"\\\n\t\r\0", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\\\n\t\r\0"));
    assert!(errors.is_empty());
}

#[test]
fn template_brace_escapes() {
    let mut errors = Vec::new();
    let result = unescape_template("hello{{world}}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("hello{world}"));
    assert!(errors.is_empty());
}

#[test]
fn template_invalid_escape() {
    let mut errors = Vec::new();
    let result = unescape_template(r"\q", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}"));
    assert_eq!(errors.len(), 1);
}

#[test]
fn template_mixed_escapes_and_braces() {
    let mut errors = Vec::new();
    let result = unescape_template(r"a\nb{{c}}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("a\nb{c}"));
    assert!(errors.is_empty());
}

#[test]
fn template_trailing_single_brace() {
    // A single { should pass through (it would be part of interpolation in real use)
    let mut errors = Vec::new();
    let result = unescape_template("a{b", 0, &mut errors);
    // No backslashes, no double braces — fast path
    assert!(result.is_none());
    assert!(errors.is_empty());
}

// Unicode escape: char happy paths

#[test]
fn char_unicode_escape_emoji() {
    let mut errors = Vec::new();
    let result = unescape_char(r"\u{1F600}", 0, &mut errors);
    assert_eq!(result, '😀');
    assert!(errors.is_empty());
}

#[test]
fn char_unicode_escape_ascii() {
    let mut errors = Vec::new();
    let result = unescape_char(r"\u{41}", 0, &mut errors);
    assert_eq!(result, 'A');
    assert!(errors.is_empty());
}

#[test]
fn char_unicode_escape_null() {
    let mut errors = Vec::new();
    let result = unescape_char(r"\u{0}", 0, &mut errors);
    assert_eq!(result, '\0');
    assert!(errors.is_empty());
}

#[test]
fn char_unicode_escape_max_codepoint() {
    let mut errors = Vec::new();
    let result = unescape_char(r"\u{10FFFF}", 0, &mut errors);
    assert_eq!(result, '\u{10FFFF}');
    assert!(errors.is_empty());
}

#[test]
fn char_unicode_escape_lowercase_hex() {
    let mut errors = Vec::new();
    let result = unescape_char(r"\u{1f600}", 0, &mut errors);
    assert_eq!(result, '😀');
    assert!(errors.is_empty());
}

// Unicode escape: string happy paths

#[test]
fn string_unicode_escape() {
    let mut errors = Vec::new();
    let result = unescape_string(r"hello\u{1F600}world", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("hello😀world"));
    assert!(errors.is_empty());
}

#[test]
fn string_unicode_escape_ascii() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\u{41}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("A"));
    assert!(errors.is_empty());
}

#[test]
fn string_unicode_escape_mixed() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\n\u{41}\t", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\nA\t"));
    assert!(errors.is_empty());
}

#[test]
fn string_multiple_unicode_escapes() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\u{48}\u{65}\u{6C}\u{6C}\u{6F}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("Hello"));
    assert!(errors.is_empty());
}

// Unicode escape: template happy paths

#[test]
fn template_unicode_escape() {
    let mut errors = Vec::new();
    let result = unescape_template(r"hello\u{1F600}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("hello😀"));
    assert!(errors.is_empty());
}

#[test]
fn template_unicode_escape_mixed() {
    let mut errors = Vec::new();
    let result = unescape_template(r"\u{41}\n\`", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("A\n`"));
    assert!(errors.is_empty());
}

// Unicode escape: error cases

#[test]
fn char_unicode_escape_surrogate() {
    let mut errors = Vec::new();
    let result = unescape_char(r"\u{D800}", 0, &mut errors);
    assert_eq!(result, '\u{FFFD}');
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::SurrogateCodepoint { codepoint: 0xD800 }
        }
    ));
    assert_eq!(errors[0].context, LexErrorContext::InsideChar);
}

#[test]
fn string_unicode_escape_out_of_range() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\u{110000}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::OutOfRange {
                codepoint: 0x11_0000
            }
        }
    ));
    assert!(matches!(
        errors[0].context,
        LexErrorContext::InsideString { .. }
    ));
}

#[test]
fn unicode_escape_empty_digits() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\u{}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::EmptyDigits
        }
    ));
}

#[test]
fn unicode_escape_invalid_hex_digit() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\u{GGGG}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::InvalidHexDigit { ch: 'G' }
        }
    ));
}

#[test]
fn unicode_escape_missing_close_brace() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\u{41", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::MissingCloseBrace
        }
    ));
}

#[test]
fn char_unicode_escape_missing_open_brace() {
    // Content from '\u' token (raw layer sees '\u' then closing ')
    let mut errors = Vec::new();
    let result = unescape_char(r"\u", 0, &mut errors);
    assert_eq!(result, '\u{FFFD}');
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::MissingOpenBrace
        }
    ));
    assert_eq!(errors[0].context, LexErrorContext::InsideChar);
}

#[test]
fn string_unicode_escape_missing_open_brace() {
    // "\uX" — the 'u' is followed by 'X', not '{'
    let mut errors = Vec::new();
    let result = unescape_string(r"\uX", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}X"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::MissingOpenBrace
        }
    ));
    assert!(matches!(
        errors[0].context,
        LexErrorContext::InsideString { .. }
    ));
}

#[test]
fn unicode_escape_too_many_digits() {
    let mut errors = Vec::new();
    let result = unescape_string(r"\u{1234567}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::TooManyDigits
        }
    ));
}

#[test]
fn template_unicode_escape_surrogate() {
    let mut errors = Vec::new();
    let result = unescape_template(r"\u{DFFF}", 0, &mut errors);
    assert_eq!(result.as_deref(), Some("\u{FFFD}"));
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        crate::lex_error::LexErrorKind::InvalidUnicodeEscape {
            detail: crate::lex_error::UnicodeEscapeDetail::SurrogateCodepoint { codepoint: 0xDFFF }
        }
    ));
    assert!(matches!(
        errors[0].context,
        LexErrorContext::InsideTemplate { .. }
    ));
}

// Unicode escape: error spans

#[test]
fn unicode_escape_span_in_string() {
    // "abc\u{D800}xyz" — backslash at byte 3, base_offset = 1 (after opening ")
    // So backslash is at source offset 4
    // parse_unicode_escape gets "u{D800}" (7 bytes), consumed = 7
    // Span: backslash_offset .. backslash_offset + 1 + consumed = 4..12
    let mut errors = Vec::new();
    let _ = unescape_string(r"abc\u{D800}xyz", 1, &mut errors);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].span, Span::new(4, 12));
}

#[test]
fn unicode_escape_span_missing_open_brace() {
    // "\u" at start, base_offset = 0
    // Backslash at 0, span covers \u = 0..2
    let mut errors = Vec::new();
    let _ = unescape_char(r"\u", 0, &mut errors);
    assert_eq!(errors[0].span, Span::new(0, 2));
}
