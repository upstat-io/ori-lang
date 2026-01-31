//! Escape Sequence Processing
//!
//! Handles escape sequences in string and character literals.
//! Recognized escapes: `\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`

/// Resolve a single escape character to its replacement.
///
/// Returns `Some(char)` for recognized escapes, `None` for unrecognized ones.
/// Recognized escapes: `\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`
#[inline]
pub(crate) fn resolve_escape(c: char) -> Option<char> {
    match c {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '0' => Some('\0'),
        _ => None,
    }
}

/// Process string escape sequences.
///
/// Uses `char_indices()` directly to avoid `Peekable` iterator overhead.
/// Invalid escapes are preserved literally (e.g., `\q` becomes `\q`).
#[inline]
pub(crate) fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.char_indices();

    while let Some((_, c)) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some((_, esc)) => {
                    if let Some(resolved) = resolve_escape(esc) {
                        result.push(resolved);
                    } else {
                        result.push('\\');
                        result.push(esc);
                    }
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Process char escape sequences.
///
/// Returns the unescaped character. Invalid escapes return the escaped character
/// (e.g., `\q` returns `q`). Empty input returns `\0`.
#[inline]
pub(crate) fn unescape_char(s: &str) -> char {
    let mut chars = s.chars();
    match chars.next() {
        Some('\\') => match chars.next() {
            Some(esc) => resolve_escape(esc).unwrap_or(esc),
            None => '\\',
        },
        Some(c) => c,
        None => '\0',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_escape_valid() {
        assert_eq!(resolve_escape('n'), Some('\n'));
        assert_eq!(resolve_escape('r'), Some('\r'));
        assert_eq!(resolve_escape('t'), Some('\t'));
        assert_eq!(resolve_escape('\\'), Some('\\'));
        assert_eq!(resolve_escape('"'), Some('"'));
        assert_eq!(resolve_escape('\''), Some('\''));
        assert_eq!(resolve_escape('0'), Some('\0'));
    }

    #[test]
    fn test_resolve_escape_invalid() {
        assert_eq!(resolve_escape('q'), None);
        assert_eq!(resolve_escape('x'), None);
        assert_eq!(resolve_escape('a'), None);
        assert_eq!(resolve_escape(' '), None);
    }

    #[test]
    fn test_unescape_string_no_escapes() {
        assert_eq!(unescape_string("hello world"), "hello world");
        assert_eq!(unescape_string(""), "");
        assert_eq!(unescape_string("abc123"), "abc123");
    }

    #[test]
    fn test_unescape_string_valid_escapes() {
        assert_eq!(unescape_string(r"hello\nworld"), "hello\nworld");
        assert_eq!(unescape_string(r"tab\there"), "tab\there");
        assert_eq!(unescape_string(r#"quote\"test"#), "quote\"test");
        assert_eq!(unescape_string(r"back\\slash"), "back\\slash");
        assert_eq!(unescape_string(r"null\0char"), "null\0char");
        assert_eq!(unescape_string(r"\n\r\t"), "\n\r\t");
    }

    #[test]
    fn test_unescape_string_invalid_escapes() {
        // Invalid escapes are preserved literally
        assert_eq!(unescape_string(r"\q"), "\\q");
        assert_eq!(unescape_string(r"\x"), "\\x");
        assert_eq!(unescape_string(r"test\qvalue"), "test\\qvalue");
    }

    #[test]
    fn test_unescape_string_trailing_backslash() {
        assert_eq!(unescape_string(r"test\"), "test\\");
    }

    #[test]
    fn test_unescape_char_simple() {
        assert_eq!(unescape_char("a"), 'a');
        assert_eq!(unescape_char("λ"), 'λ');
        assert_eq!(unescape_char("0"), '0');
    }

    #[test]
    fn test_unescape_char_escapes() {
        assert_eq!(unescape_char(r"\n"), '\n');
        assert_eq!(unescape_char(r"\t"), '\t');
        assert_eq!(unescape_char(r"\\"), '\\');
        assert_eq!(unescape_char(r"\'"), '\'');
    }

    #[test]
    fn test_unescape_char_invalid_escape() {
        // Invalid escape returns the escaped character
        assert_eq!(unescape_char(r"\q"), 'q');
    }

    #[test]
    fn test_unescape_char_empty() {
        assert_eq!(unescape_char(""), '\0');
    }

    #[test]
    fn test_unescape_char_lone_backslash() {
        assert_eq!(unescape_char("\\"), '\\');
    }
}
