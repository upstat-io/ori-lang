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
