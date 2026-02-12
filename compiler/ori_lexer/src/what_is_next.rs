//! Context inspection for error message generation.
//!
//! Inspired by Elm's `whatIsNext` pattern: inspect what character or sequence
//! the lexer got stuck on to produce tailored error messages.

/// What was found at the position where the lexer got stuck.
///
/// Used to generate context-aware error messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NextContext {
    /// An operator-like sequence (e.g., `===`, `++`).
    Operator(&'static str),
    /// A single punctuation character.
    Punctuation(char),
    /// A non-ASCII character (with confusable info if available).
    Unicode(char),
    /// At end of file.
    EndOfFile,
    /// Something else.
    Other(char),
}

/// Inspect the character/sequence at the given position to classify what
/// the lexer got stuck on.
///
/// This is a lightweight inspection — it peeks at a few bytes ahead but
/// does not perform full tokenization. Used by the cooker when generating
/// error messages for `InvalidByte` and other context-sensitive errors.
pub(crate) fn what_is_next(source: &[u8], pos: u32) -> NextContext {
    let pos = pos as usize;
    if pos >= source.len() {
        return NextContext::EndOfFile;
    }

    let byte = source[pos];

    // Non-ASCII byte — could be Unicode confusable
    if byte >= 0x80 {
        // Try to decode as UTF-8
        if let Ok(s) = std::str::from_utf8(&source[pos..]) {
            if let Some(ch) = s.chars().next() {
                return NextContext::Unicode(ch);
            }
        }
        return NextContext::Other(byte as char);
    }

    let ch = byte as char;

    // Multi-character operator patterns
    match byte {
        b'=' if matches!(peek(source, pos + 1), Some(b'='))
            && matches!(peek(source, pos + 2), Some(b'=')) =>
        {
            NextContext::Operator("===")
        }
        b'!' if matches!(peek(source, pos + 1), Some(b'='))
            && matches!(peek(source, pos + 2), Some(b'=')) =>
        {
            NextContext::Operator("!==")
        }
        b'+' if matches!(peek(source, pos + 1), Some(b'+')) => NextContext::Operator("++"),
        b'-' if matches!(peek(source, pos + 1), Some(b'-')) => NextContext::Operator("--"),
        // Single punctuation
        b';' | b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b':' | b'@' | b'#'
        | b'\\' | b'\'' | b'"' | b'`' | b'?' | b'!' | b'~' | b'^' | b'&' | b'|' | b'+' | b'-'
        | b'*' | b'/' | b'%' | b'=' | b'<' | b'>' | b'$' | b'_' => NextContext::Punctuation(ch),
        // ASCII control characters, letters, digits, and everything else
        _ => NextContext::Other(ch),
    }
}

/// Peek at a byte without bounds checking.
#[inline]
fn peek(source: &[u8], pos: usize) -> Option<u8> {
    source.get(pos).copied()
}

#[cfg(test)]
mod tests {
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
    fn detects_decrement() {
        assert_eq!(what_is_next(b"--x", 0), NextContext::Operator("--"));
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
}
