use super::*;

#[test]
fn detects_triple_equals() {
    assert_eq!(what_is_next(b"===", 0), NextContext::Operator("==="));
}

#[test]
fn detects_not_triple_equals() {
    assert_eq!(what_is_next(b"!==", 0), NextContext::Operator("!=="));
}

#[test]
fn detects_increment() {
    assert_eq!(what_is_next(b"++", 0), NextContext::Operator("++"));
}

#[test]
fn detects_semicolon() {
    assert_eq!(what_is_next(b";", 0), NextContext::Punctuation(';'));
}

#[test]
fn detects_unicode() {
    // Smart quote "\u{201C}" is multi-byte UTF-8
    let source = "\u{201C}hello";
    assert!(matches!(
        what_is_next(source.as_bytes(), 0),
        NextContext::Unicode('\u{201C}')
    ));
}

#[test]
fn detects_eof() {
    assert_eq!(what_is_next(b"", 0), NextContext::EndOfFile);
    assert_eq!(what_is_next(b"x", 1), NextContext::EndOfFile);
}

#[test]
fn single_equals_is_punctuation() {
    // A single = should NOT match === pattern
    assert_eq!(what_is_next(b"=x", 0), NextContext::Punctuation('='));
}

#[test]
fn ascii_control_char_is_other() {
    // 0x01 is < 0x80, not a multi-char operator, and not in the punctuation
    // set -> NextContext::Other (the fall-through ASCII arm).
    assert_eq!(what_is_next(b"\x01", 0), NextContext::Other('\u{1}'));
}

#[test]
fn ascii_letter_is_other() {
    // An ASCII letter is not punctuation/operator -> NextContext::Other.
    assert_eq!(what_is_next(b"a", 0), NextContext::Other('a'));
}

#[test]
fn ascii_digit_is_other() {
    // An ASCII digit falls through to the catch-all -> NextContext::Other.
    assert_eq!(what_is_next(b"7", 0), NextContext::Other('7'));
}
