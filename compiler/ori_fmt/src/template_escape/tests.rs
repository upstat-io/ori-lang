use super::escape_template_text;

#[test]
fn escape_template_text_backslash_doubles() {
    // Cooked `a\b` (a, backslash, b) must re-escape so it round-trips.
    assert_eq!(escape_template_text("a\\b"), "a\\\\b");
}

#[test]
fn escape_template_text_backtick_gains_backslash() {
    assert_eq!(escape_template_text("`"), "\\`");
}

#[test]
fn escape_template_text_braces_double() {
    assert_eq!(escape_template_text("{"), "{{");
    assert_eq!(escape_template_text("}"), "}}");
    assert_eq!(escape_template_text("{x} = "), "{{x}} = ");
}

#[test]
fn escape_template_text_control_chars_stay_literal() {
    // Newline/tab/CR/NUL are valid literal template content; layout preserved.
    assert_eq!(escape_template_text("a\nb"), "a\nb");
    assert_eq!(escape_template_text("a\tb"), "a\tb");
    assert_eq!(escape_template_text("a\rb"), "a\rb");
    assert_eq!(escape_template_text("a\0b"), "a\0b");
}

#[test]
fn escape_template_text_introduced_backslash_not_re_escaped() {
    // Single pass: the backslash introduced for `` \` `` is not doubled.
    assert_eq!(escape_template_text("`x`"), "\\`x\\`");
}

#[test]
fn escape_template_text_plain_text_unchanged() {
    assert_eq!(escape_template_text("hello world"), "hello world");
}
