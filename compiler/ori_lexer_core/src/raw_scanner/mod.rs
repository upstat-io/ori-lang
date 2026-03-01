//! Hand-written raw scanner producing `(RawTag, len)` pairs.
//!
//! The scanner operates on a sentinel-terminated [`Cursor`] and produces
//! [`RawToken`] values with zero heap allocation (except for the template
//! literal nesting stack). It does not resolve keywords, validate escapes,
//! or parse numeric values — those are deferred to the cooking layer.
//!
//! # Design
//!
//! Main dispatch covers all 256 byte values. Each arm calls a focused method
//! that advances the cursor and returns `RawToken { tag, len }`. The sentinel
//! byte (`0x00`) naturally dispatches to `eof()`.
//!
//! # Module structure
//!
//! Scanner methods are split by token category:
//! - [`operators`] — operator and punctuation scanning
//! - [`numbers`] — numeric literal scanning (decimal, hex, binary, suffixes)
//! - [`strings`] — string and character literal scanning
//! - [`templates`] — template literal scanning with interpolation

mod numbers;
mod operators;
mod strings;
mod templates;

use crate::cursor::Cursor;
use crate::tag::{RawTag, RawToken};

/// Nesting state inside a single template interpolation.
///
/// Tracks brace, paren, and bracket depth so the scanner can disambiguate
/// format-spec `:` (only valid at interpolation top-level) from `:` inside
/// nested expressions like `func(a: b)` or `map[k:v]`.
#[derive(Clone, Debug, Default)]
struct InterpolationDepth {
    brace: u32,
    paren: u32,
    bracket: u32,
}

impl InterpolationDepth {
    /// Returns `true` when all nesting counters are zero — meaning we're at
    /// the top level of the interpolation and `:` is a format spec separator.
    fn is_top_level(&self) -> bool {
        self.brace == 0 && self.paren == 0 && self.bracket == 0
    }
}

/// Pure, allocation-free scanner (except template depth stack).
///
/// Produces one token at a time as a `(tag, length)` pair.
/// Error conditions are encoded as `RawTag` variants, not as `Result::Err`.
pub struct RawScanner<'a> {
    cursor: Cursor<'a>,
    /// Stack tracking nesting depth inside template literal interpolations.
    /// Each entry represents a nesting level with brace, paren, and bracket
    /// counters. When a `}` is encountered and the top brace depth is 0,
    /// the interpolation ends.
    template_depth: Vec<InterpolationDepth>,
}

impl<'a> RawScanner<'a> {
    /// Create a new scanner from a cursor.
    pub fn new(cursor: Cursor<'a>) -> Self {
        Self {
            cursor,
            template_depth: Vec::new(),
        }
    }

    /// Produce the next raw token.
    ///
    /// Returns `RawTag::Eof` with `len == 0` when the source is exhausted.
    /// Subsequent calls after EOF continue to return `Eof`.
    #[inline]
    pub fn next_token(&mut self) -> RawToken {
        let start = self.cursor.pos();
        match self.cursor.current() {
            0 => self.eof(),
            b' ' | b'\t' => self.whitespace(start),
            b'\r' => self.carriage_return(start),
            b'\n' => self.newline(start),
            b'a'..=b'z' | b'A'..=b'Z' => self.identifier(start),
            b'_' => self.underscore_or_ident(start),
            b'0'..=b'9' => self.number(start),
            b'"' => self.string(start),
            b'\'' => self.char_literal(start),
            b'`' => self.template_literal(start),
            b'/' => self.slash_or_comment(start),
            b'+' => self.plus(start),
            b'-' => self.minus_or_arrow(start),
            b'*' => self.star(start),
            b'%' => self.percent(start),
            b'^' => self.caret(start),
            b'~' => self.single(start, RawTag::Tilde),
            b'=' => self.equal(start),
            b'!' => self.bang(start),
            b'<' => self.less(start),
            b'>' => self.single(start, RawTag::Greater),
            b'.' => self.dot(start),
            b'?' => self.question(start),
            b'|' => self.pipe(start),
            b'&' => self.ampersand(start),
            b'(' => self.left_paren(start),
            b')' => self.right_paren(start),
            b'[' => self.left_bracket(start),
            b']' => self.right_bracket(start),
            b'{' => self.left_brace(start),
            b'}' => self.right_brace(start),
            b',' => self.single(start, RawTag::Comma),
            b':' => self.colon(start),
            b';' => self.single(start, RawTag::Semicolon),
            b'@' => self.at(start),
            b'$' => self.single(start, RawTag::Dollar),
            b'#' => self.hash(start),
            b'\\' => self.single(start, RawTag::Backslash),
            // Control characters (excluding \t, \n, \r), DEL, and non-ASCII bytes
            1..=8 | 11..=12 | 14..=31 | 127..=255 => self.invalid_byte(start),
        }
    }

    // EOF

    fn eof(&mut self) -> RawToken {
        if self.cursor.is_eof() {
            if let Some(_depth) = self.template_depth.pop() {
                // EOF inside template interpolation — no closing `}` was
                // encountered, so template_middle_or_tail never ran. Emit
                // one UnterminatedTemplate per orphaned nesting level so
                // consumers detect the error from the token stream alone.
                return RawToken {
                    tag: RawTag::UnterminatedTemplate,
                    len: 0,
                };
            }
            RawToken {
                tag: RawTag::Eof,
                len: 0,
            }
        } else {
            // Interior null byte — advance past it. The integration layer
            // skips InteriorNull tokens since SourceBuffer already reported
            // these via encoding_issues() with more specific diagnostics.
            let start = self.cursor.pos();
            self.cursor.advance();
            RawToken {
                tag: RawTag::InteriorNull,
                len: self.cursor.pos() - start,
            }
        }
    }

    // Whitespace & newlines

    #[inline]
    fn whitespace(&mut self, start: u32) -> RawToken {
        self.cursor.eat_whitespace();
        RawToken {
            tag: RawTag::Whitespace,
            len: self.cursor.pos() - start,
        }
    }

    fn carriage_return(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '\r'
        if self.cursor.current() == b'\n' {
            // CRLF normalization: \r\n -> single Newline with len=2
            self.cursor.advance();
            RawToken {
                tag: RawTag::Newline,
                len: self.cursor.pos() - start,
            }
        } else {
            // Lone \r: horizontal whitespace per grammar
            RawToken {
                tag: RawTag::Whitespace,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn newline(&mut self, start: u32) -> RawToken {
        self.cursor.advance();
        RawToken {
            tag: RawTag::Newline,
            len: self.cursor.pos() - start,
        }
    }

    // Comments

    fn slash_or_comment(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume first '/'
        match self.cursor.current() {
            b'/' => {
                self.cursor.advance(); // consume second '/'
                                       // SIMD-accelerated scan to end of line
                self.cursor.eat_until_newline_or_eof();
                RawToken {
                    tag: RawTag::LineComment,
                    len: self.cursor.pos() - start,
                }
            }
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::SlashEq,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Slash,
                len: self.cursor.pos() - start,
            },
        }
    }

    // Identifiers

    #[inline]
    fn identifier(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume first char (already validated)
        self.eat_ident_continue();
        RawToken {
            tag: RawTag::Ident,
            len: self.cursor.pos() - start,
        }
    }

    fn underscore_or_ident(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '_'
        if is_ident_continue(self.cursor.current()) {
            self.eat_ident_continue();
            RawToken {
                tag: RawTag::Ident,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Underscore,
                len: self.cursor.pos() - start,
            }
        }
    }

    #[inline]
    fn eat_ident_continue(&mut self) {
        self.cursor.eat_while(is_ident_continue);
    }

    // Delimiters (template-aware)

    fn left_paren(&mut self, start: u32) -> RawToken {
        self.cursor.advance();
        if let Some(depth) = self.template_depth.last_mut() {
            depth.paren += 1;
        }
        RawToken {
            tag: RawTag::LeftParen,
            len: self.cursor.pos() - start,
        }
    }

    fn right_paren(&mut self, start: u32) -> RawToken {
        self.cursor.advance();
        if let Some(depth) = self.template_depth.last_mut() {
            depth.paren = depth.paren.saturating_sub(1);
        }
        RawToken {
            tag: RawTag::RightParen,
            len: self.cursor.pos() - start,
        }
    }

    fn left_bracket(&mut self, start: u32) -> RawToken {
        self.cursor.advance();
        if let Some(depth) = self.template_depth.last_mut() {
            depth.bracket += 1;
        }
        RawToken {
            tag: RawTag::LeftBracket,
            len: self.cursor.pos() - start,
        }
    }

    fn right_bracket(&mut self, start: u32) -> RawToken {
        self.cursor.advance();
        if let Some(depth) = self.template_depth.last_mut() {
            depth.bracket = depth.bracket.saturating_sub(1);
        }
        RawToken {
            tag: RawTag::RightBracket,
            len: self.cursor.pos() - start,
        }
    }

    fn left_brace(&mut self, start: u32) -> RawToken {
        self.cursor.advance();
        // If inside a template interpolation, increment brace depth
        if let Some(depth) = self.template_depth.last_mut() {
            depth.brace += 1;
        }
        RawToken {
            tag: RawTag::LeftBrace,
            len: self.cursor.pos() - start,
        }
    }

    fn right_brace(&mut self, start: u32) -> RawToken {
        if let Some(depth) = self.template_depth.last_mut() {
            if depth.brace == 0 {
                // This `}` closes the interpolation — scan template continuation
                self.template_depth.pop();
                return self.template_middle_or_tail(start);
            }
            depth.brace -= 1;
        }
        self.cursor.advance();
        RawToken {
            tag: RawTag::RightBrace,
            len: self.cursor.pos() - start,
        }
    }

    // Error tokens

    fn invalid_byte(&mut self, start: u32) -> RawToken {
        self.cursor.advance();
        RawToken {
            tag: RawTag::InvalidByte,
            len: self.cursor.pos() - start,
        }
    }
}

impl Iterator for RawScanner<'_> {
    type Item = RawToken;

    fn next(&mut self) -> Option<RawToken> {
        let tok = self.next_token();
        if tok.tag == RawTag::Eof {
            None
        } else {
            Some(tok)
        }
    }
}

/// 256-byte lookup table for identifier continuation bytes.
/// `true` for a-z, A-Z, 0-9, and underscore.
/// Table lookup replaces the multi-range `matches!` with a single indexed read.
/// The sentinel byte (0x00) maps to `false`, naturally terminating loops.
#[allow(
    clippy::cast_possible_truncation,
    reason = "loop counter i is 0..=255, always fits in u8"
)]
static IS_IDENT_CONTINUE_TABLE: [bool; 256] = {
    let mut table = [false; 256];
    let mut i = 0u16;
    while i < 256 {
        table[i as usize] = matches!(
            i as u8,
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'
        );
        i += 1;
    }
    table
};

/// Returns `true` if `b` is a valid identifier continuation byte.
#[inline]
fn is_ident_continue(b: u8) -> bool {
    IS_IDENT_CONTINUE_TABLE[b as usize]
}

/// Convenience function: tokenize a source string and collect all raw tokens.
///
/// Returns a `Vec<RawToken>` containing all tokens except the final `Eof`.
/// For streaming/iterator access, construct a `SourceBuffer` + `RawScanner` directly.
pub fn tokenize(source: &str) -> Vec<RawToken> {
    let buf = crate::SourceBuffer::new(source);
    let mut scanner = RawScanner::new(buf.cursor());
    let mut tokens = Vec::new();
    loop {
        let tok = scanner.next_token();
        if tok.tag == RawTag::Eof {
            break;
        }
        tokens.push(tok);
    }
    tokens
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test assertions use unwrap/expect for clarity"
)]
mod tests;
