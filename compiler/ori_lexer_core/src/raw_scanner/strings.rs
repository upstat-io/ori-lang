//! String and character literal scanning.
//!
//! String scanning uses SIMD-accelerated `skip_to_string_delim()` to skip
//! past ordinary content. Character scanning validates single-codepoint
//! content and recovers malformed multi-character literals as a single
//! error token.

use crate::tag::{RawTag, RawToken};

impl super::RawScanner<'_> {
    pub(super) fn string(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume opening '"'
        loop {
            // SIMD-accelerated skip past ordinary string content
            let b = self.cursor.skip_to_string_delim();
            match b {
                b'"' => {
                    self.cursor.advance(); // consume closing '"'
                    return RawToken {
                        tag: RawTag::String,
                        len: self.cursor.pos() - start,
                    };
                }
                b'\\' => {
                    self.cursor.advance(); // consume '\'
                    self.skip_escape_body();
                }
                b'\n' | b'\r' => {
                    return RawToken {
                        tag: RawTag::UnterminatedString,
                        len: self.cursor.pos() - start,
                    };
                }
                0 => {
                    if self.cursor.is_eof() {
                        return RawToken {
                            tag: RawTag::UnterminatedString,
                            len: self.cursor.pos() - start,
                        };
                    }
                    // Interior null — advance past it (cooking layer reports error)
                    self.cursor.advance();
                }
                _ => unreachable!("skip_to_string_delim returned unexpected byte"),
            }
        }
    }

    pub(super) fn char_literal(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume opening '\''

        // Handle char content — must advance the full UTF-8 code point,
        // not just one byte. 'λ' is 2 bytes (CE BB), '😀' is 4 bytes.
        // For escape sequences, `\u{...}` can span many bytes.
        match self.cursor.current() {
            b'\\' => {
                self.cursor.advance(); // consume '\'
                self.skip_escape_body();
            }
            b'\'' | b'\n' | b'\r' => {
                // Empty char literal or unterminated
                return RawToken {
                    tag: RawTag::UnterminatedChar,
                    len: self.cursor.pos() - start,
                };
            }
            0 => {
                if self.cursor.is_eof() {
                    return RawToken {
                        tag: RawTag::UnterminatedChar,
                        len: self.cursor.pos() - start,
                    };
                }
                self.cursor.advance(); // interior null
            }
            _ => self.cursor.advance_char(), // normal char (may be multi-byte UTF-8)
        }

        // Expect closing '\''
        if self.cursor.current() == b'\'' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::Char,
                len: self.cursor.pos() - start,
            }
        } else {
            // Recovery: scan forward to closing '\'' or line break so the
            // entire malformed literal (e.g. 'ab') is one error token.
            loop {
                match self.cursor.current() {
                    b'\'' => {
                        self.cursor.advance();
                        break;
                    }
                    b'\\' => {
                        self.cursor.advance();
                        self.skip_escape_body();
                    }
                    b'\n' | b'\r' => break,
                    0 if self.cursor.is_eof() => break,
                    _ => self.cursor.advance(),
                }
            }
            RawToken {
                tag: RawTag::UnterminatedChar,
                len: self.cursor.pos() - start,
            }
        }
    }

    /// Skip the body of an escape sequence after `\` has been consumed.
    ///
    /// For `\u` escapes: greedily consumes `u`, optional `{`, hex digits,
    /// and optional `}`. For all other escapes: advances 1 byte.
    ///
    /// This is boundary detection only — content validation is the cooked
    /// layer's responsibility.
    ///
    /// Used by string, char, and template scanning.
    #[inline]
    pub(super) fn skip_escape_body(&mut self) {
        let b = self.cursor.current();
        if b == 0 && self.cursor.is_eof() {
            return;
        }
        if b == b'u' {
            self.cursor.advance(); // consume 'u'
            if self.cursor.current() == b'{' {
                self.cursor.advance(); // consume '{'
                                       // Consume hex digits
                while self.cursor.current().is_ascii_hexdigit() {
                    self.cursor.advance();
                }
                if self.cursor.current() == b'}' {
                    self.cursor.advance(); // consume '}'
                }
            }
            // If no '{', stop — cursor is past 'u', at whatever follows.
            // The cooked layer will report MissingOpenBrace.
        } else {
            self.cursor.advance(); // single-byte escape
        }
    }
}
