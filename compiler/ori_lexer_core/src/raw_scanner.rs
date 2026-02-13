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
            b'+' => self.single(start, RawTag::Plus),
            b'-' => self.minus_or_arrow(start),
            b'*' => self.single(start, RawTag::Star),
            b'%' => self.single(start, RawTag::Percent),
            b'^' => self.single(start, RawTag::Caret),
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
            b'@' => self.single(start, RawTag::At),
            b'$' => self.single(start, RawTag::Dollar),
            b'#' => self.hash(start),
            b'\\' => self.single(start, RawTag::Backslash),
            // Control characters (excluding \t, \n, \r), DEL, and non-ASCII bytes
            1..=8 | 11..=12 | 14..=31 | 127..=255 => self.invalid_byte(start),
        }
    }

    // ─── EOF ───────────────────────────────────────────────────────

    fn eof(&mut self) -> RawToken {
        if self.cursor.is_eof() {
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

    // ─── Whitespace & Newlines ─────────────────────────────────────

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

    // ─── Comments ──────────────────────────────────────────────────

    fn slash_or_comment(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume first '/'
        if self.cursor.current() == b'/' {
            self.cursor.advance(); // consume second '/'
                                   // SIMD-accelerated scan to end of line
            self.cursor.eat_until_newline_or_eof();
            RawToken {
                tag: RawTag::LineComment,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Slash,
                len: self.cursor.pos() - start,
            }
        }
    }

    // ─── Identifiers ───────────────────────────────────────────────

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

    // ─── Operators ─────────────────────────────────────────────────

    /// Single-byte token: advance one byte and emit the given tag.
    fn single(&mut self, start: u32, tag: RawTag) -> RawToken {
        self.cursor.advance();
        RawToken {
            tag,
            len: self.cursor.pos() - start,
        }
    }

    fn minus_or_arrow(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '-'
        if self.cursor.current() == b'>' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::Arrow,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Minus,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn equal(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '='
        match self.cursor.current() {
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::EqualEqual,
                    len: self.cursor.pos() - start,
                }
            }
            b'>' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::FatArrow,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Equal,
                len: self.cursor.pos() - start,
            },
        }
    }

    fn bang(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '!'
        if self.cursor.current() == b'=' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::BangEqual,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Bang,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn less(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '<'
        match self.cursor.current() {
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::LessEqual,
                    len: self.cursor.pos() - start,
                }
            }
            b'<' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::Shl,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Less,
                len: self.cursor.pos() - start,
            },
        }
    }

    fn dot(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '.'
        if self.cursor.current() == b'.' {
            self.cursor.advance(); // consume second '.'
            if self.cursor.current() == b'=' {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::DotDotEqual,
                    len: self.cursor.pos() - start,
                }
            } else if self.cursor.current() == b'.' {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::DotDotDot,
                    len: self.cursor.pos() - start,
                }
            } else {
                RawToken {
                    tag: RawTag::DotDot,
                    len: self.cursor.pos() - start,
                }
            }
        } else {
            RawToken {
                tag: RawTag::Dot,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn question(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '?'
        if self.cursor.current() == b'?' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::QuestionQuestion,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Question,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn pipe(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '|'
        if self.cursor.current() == b'|' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::PipePipe,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Pipe,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn ampersand(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '&'
        if self.cursor.current() == b'&' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::AmpersandAmpersand,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Ampersand,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn colon(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume ':'

        // Inside template interpolation at top-level → format spec separator
        if let Some(depth) = self.template_depth.last() {
            if depth.is_top_level() {
                return self.format_spec(start);
            }
        }

        if self.cursor.current() == b':' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::ColonColon,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Colon,
                len: self.cursor.pos() - start,
            }
        }
    }

    /// Scan a format spec after `:` in a template interpolation.
    ///
    /// Consumes everything between `:` (already consumed) and `}` (not consumed).
    /// The `}` will be handled by the normal `right_brace` → `template_middle_or_tail`
    /// path on the next call to `next_token()`.
    fn format_spec(&mut self, start: u32) -> RawToken {
        // Scan forward until `}` at brace depth 0.
        // Track nested `{}`  in the spec (unlikely but safe).
        let mut brace_depth: u32 = 0;
        loop {
            match self.cursor.current() {
                b'}' if brace_depth == 0 => {
                    // Don't consume the `}` — it triggers template_middle_or_tail
                    return RawToken {
                        tag: RawTag::FormatSpec,
                        len: self.cursor.pos() - start,
                    };
                }
                b'}' => {
                    brace_depth -= 1;
                    self.cursor.advance();
                }
                b'{' => {
                    brace_depth += 1;
                    self.cursor.advance();
                }
                0 if self.cursor.is_eof() => {
                    // Unterminated — return what we have
                    return RawToken {
                        tag: RawTag::FormatSpec,
                        len: self.cursor.pos() - start,
                    };
                }
                _ => {
                    self.cursor.advance();
                }
            }
        }
    }

    fn hash(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '#'
        match self.cursor.current() {
            b'[' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::HashBracket,
                    len: self.cursor.pos() - start,
                }
            }
            b'!' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::HashBang,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Hash,
                len: self.cursor.pos() - start,
            },
        }
    }

    // ─── Delimiters (template-aware) ────────────────────────────────

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

    // ─── Numeric Literals ──────────────────────────────────────────

    #[inline]
    fn number(&mut self, start: u32) -> RawToken {
        let first = self.cursor.current();
        self.cursor.advance();

        // Check for hex prefix: 0x or 0X
        if first == b'0' && matches!(self.cursor.current(), b'x' | b'X') {
            return self.hex_number(start);
        }

        // Check for binary prefix: 0b or 0B followed by binary digit or underscore.
        // Without the peek, `0b` (0 bytes size literal) would be misclassified.
        if first == b'0'
            && matches!(self.cursor.current(), b'b' | b'B')
            && matches!(self.cursor.peek(), b'0' | b'1' | b'_')
        {
            return self.bin_number(start);
        }

        // Decimal digits and underscores
        self.eat_decimal_digits();

        // Check for float (dot followed by digit — not `..` range)
        if self.cursor.current() == b'.' && self.cursor.peek().is_ascii_digit() {
            self.cursor.advance(); // consume '.'
            self.eat_decimal_digits();
            self.eat_exponent();
            return self.check_suffix(start, true);
        }

        // Check for exponent without dot (e.g., 1e5)
        if matches!(self.cursor.current(), b'e' | b'E') {
            self.eat_exponent();
            return self.check_suffix(start, true);
        }

        // Integer — check for duration/size suffix
        self.check_suffix(start, false)
    }

    fn hex_number(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume 'x' or 'X'
        self.cursor
            .eat_while(|b| b.is_ascii_hexdigit() || b == b'_');
        RawToken {
            tag: RawTag::HexInt,
            len: self.cursor.pos() - start,
        }
    }

    fn bin_number(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume 'b' or 'B'
        self.cursor
            .eat_while(|b| b == b'0' || b == b'1' || b == b'_');
        RawToken {
            tag: RawTag::BinInt,
            len: self.cursor.pos() - start,
        }
    }

    fn eat_decimal_digits(&mut self) {
        self.cursor.eat_while(|b| b.is_ascii_digit() || b == b'_');
    }

    fn eat_exponent(&mut self) {
        if matches!(self.cursor.current(), b'e' | b'E') {
            self.cursor.advance();
            if matches!(self.cursor.current(), b'+' | b'-') {
                self.cursor.advance();
            }
            self.eat_decimal_digits();
        }
    }

    /// Check for duration/size suffix after a numeric literal.
    /// `is_float` indicates whether a decimal point was consumed.
    fn check_suffix(&mut self, start: u32, is_float: bool) -> RawToken {
        let default_tag = if is_float { RawTag::Float } else { RawTag::Int };

        match self.cursor.current() {
            // ns, us — 2-char duration suffixes
            b'n' | b'u'
                if self.cursor.peek() == b's' && !is_ident_continue(self.cursor.peek2()) =>
            {
                self.cursor.advance_n(2);
                RawToken {
                    tag: RawTag::Duration,
                    len: self.cursor.pos() - start,
                }
            }
            // m, ms, mb — minutes / milliseconds / megabytes
            b'm' => match self.cursor.peek() {
                b's' if !is_ident_continue(self.cursor.peek2()) => {
                    self.cursor.advance_n(2);
                    RawToken {
                        tag: RawTag::Duration,
                        len: self.cursor.pos() - start,
                    }
                }
                b'b' if !is_ident_continue(self.cursor.peek2()) => {
                    self.cursor.advance_n(2);
                    RawToken {
                        tag: RawTag::Size,
                        len: self.cursor.pos() - start,
                    }
                }
                next if !is_ident_continue(next) => {
                    self.cursor.advance();
                    RawToken {
                        tag: RawTag::Duration,
                        len: self.cursor.pos() - start,
                    }
                }
                _ => RawToken {
                    tag: default_tag,
                    len: self.cursor.pos() - start,
                },
            },
            // s, h — 1-char duration suffixes
            b's' | b'h' if !is_ident_continue(self.cursor.peek()) => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::Duration,
                    len: self.cursor.pos() - start,
                }
            }
            // b — bytes (1-char size suffix)
            b'b' if !is_ident_continue(self.cursor.peek()) => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::Size,
                    len: self.cursor.pos() - start,
                }
            }
            // kb, gb, tb — 2-char size suffixes
            b'k' | b'g' | b't'
                if self.cursor.peek() == b'b' && !is_ident_continue(self.cursor.peek2()) =>
            {
                self.cursor.advance_n(2);
                RawToken {
                    tag: RawTag::Size,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: default_tag,
                len: self.cursor.pos() - start,
            },
        }
    }

    // ─── String & Char Literals ────────────────────────────────────

    fn string(&mut self, start: u32) -> RawToken {
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
                    if self.cursor.current() != 0 || !self.cursor.is_eof() {
                        self.cursor.advance(); // skip escaped char
                    }
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

    fn char_literal(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume opening '\''

        // Handle char content — must advance the full UTF-8 code point,
        // not just one byte. 'λ' is 2 bytes (CE BB), '😀' is 4 bytes.
        match self.cursor.current() {
            b'\\' => {
                self.cursor.advance(); // consume '\'
                if self.cursor.current() != 0 || !self.cursor.is_eof() {
                    self.cursor.advance(); // skip escaped char (always ASCII)
                }
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
            RawToken {
                tag: RawTag::UnterminatedChar,
                len: self.cursor.pos() - start,
            }
        }
    }

    // ─── Template Literals ─────────────────────────────────────────

    fn template_literal(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume opening '`'
        loop {
            // SIMD-accelerated skip past ordinary template content
            let b = self.cursor.skip_to_template_delim();
            match b {
                b'`' => {
                    self.cursor.advance();
                    return RawToken {
                        tag: RawTag::TemplateComplete,
                        len: self.cursor.pos() - start,
                    };
                }
                b'{' => {
                    if self.cursor.peek() == b'{' {
                        // Escaped brace `{{`
                        self.cursor.advance();
                        self.cursor.advance();
                        continue;
                    }
                    self.cursor.advance(); // consume '{'
                    self.template_depth.push(InterpolationDepth::default());
                    return RawToken {
                        tag: RawTag::TemplateHead,
                        len: self.cursor.pos() - start,
                    };
                }
                b'}' => {
                    if self.cursor.peek() == b'}' {
                        // Escaped brace `}}`
                        self.cursor.advance();
                        self.cursor.advance();
                        continue;
                    }
                    // Lone `}` in template text — consume it
                    self.cursor.advance();
                }
                b'\\' => {
                    self.cursor.advance(); // consume '\'
                    if self.cursor.current() != 0 || !self.cursor.is_eof() {
                        self.cursor.advance(); // skip escaped char
                    }
                }
                b'\n' | b'\r' => {
                    // Templates can span multiple lines
                    self.cursor.advance();
                }
                0 => {
                    if self.cursor.is_eof() {
                        return RawToken {
                            tag: RawTag::UnterminatedTemplate,
                            len: self.cursor.pos() - start,
                        };
                    }
                    self.cursor.advance(); // interior null
                }
                _ => unreachable!("skip_to_template_delim returned unexpected byte"),
            }
        }
    }

    fn template_middle_or_tail(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume closing '}'
        loop {
            // SIMD-accelerated skip past ordinary template content
            let b = self.cursor.skip_to_template_delim();
            match b {
                b'`' => {
                    self.cursor.advance();
                    return RawToken {
                        tag: RawTag::TemplateTail,
                        len: self.cursor.pos() - start,
                    };
                }
                b'{' => {
                    if self.cursor.peek() == b'{' {
                        self.cursor.advance();
                        self.cursor.advance();
                        continue;
                    }
                    self.cursor.advance(); // consume '{'
                    self.template_depth.push(InterpolationDepth::default());
                    return RawToken {
                        tag: RawTag::TemplateMiddle,
                        len: self.cursor.pos() - start,
                    };
                }
                b'}' => {
                    if self.cursor.peek() == b'}' {
                        self.cursor.advance();
                        self.cursor.advance();
                        continue;
                    }
                    self.cursor.advance();
                }
                b'\\' => {
                    self.cursor.advance();
                    if self.cursor.current() != 0 || !self.cursor.is_eof() {
                        self.cursor.advance();
                    }
                }
                b'\n' | b'\r' => {
                    self.cursor.advance();
                }
                0 => {
                    if self.cursor.is_eof() {
                        return RawToken {
                            tag: RawTag::UnterminatedTemplate,
                            len: self.cursor.pos() - start,
                        };
                    }
                    self.cursor.advance();
                }
                _ => unreachable!("skip_to_template_delim returned unexpected byte"),
            }
        }
    }

    // ─── Error tokens ──────────────────────────────────────────────

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
mod tests {
    use super::*;
    use crate::SourceBuffer;

    /// Helper: scan a source string and collect all tokens (excluding Eof).
    fn scan(source: &str) -> Vec<RawToken> {
        let buf = SourceBuffer::new(source);
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

    /// Helper: scan and return tags only.
    fn scan_tags(source: &str) -> Vec<RawTag> {
        scan(source).iter().map(|t| t.tag).collect()
    }

    /// Helper: scan and verify the scanner produced Eof.
    fn scan_with_eof(source: &str) -> Vec<RawToken> {
        let buf = SourceBuffer::new(source);
        let mut scanner = RawScanner::new(buf.cursor());
        let mut tokens = Vec::new();
        loop {
            let tok = scanner.next_token();
            tokens.push(tok);
            if tok.tag == RawTag::Eof {
                break;
            }
        }
        tokens
    }

    // ─── Property Tests ────────────────────────────────────────────

    #[test]
    fn total_len_equals_source_len() {
        let sources = [
            "",
            "x",
            "hello world",
            "let x = 42\nlet y = x + 1",
            "\"hello\" 'c' 123 0xFF",
            "..= ... ?? :: << ->",
            "`template {x} middle {y} tail`",
            "  \t\n  \r\n  ",
            "#[attr] #!file @main $var",
        ];
        for source in sources {
            let tokens = scan(source);
            let total_len: u32 = tokens.iter().map(|t| t.len).sum();
            assert_eq!(
                total_len,
                u32::try_from(source.len()).expect("test source fits in u32"),
                "total token length mismatch for {source:?}",
            );
        }
    }

    #[test]
    fn every_token_has_positive_length() {
        let sources = ["let x = 42", "+-*/%", "\"str\" 'c'", "`tmpl`", "  \t\n\r\n"];
        for source in sources {
            for tok in scan(source) {
                assert!(tok.len > 0, "zero-length token {tok:?} in {source:?}");
            }
        }
    }

    #[test]
    fn eof_has_zero_length() {
        let tokens = scan_with_eof("");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].tag, RawTag::Eof);
        assert_eq!(tokens[0].len, 0);
    }

    #[test]
    fn eof_is_always_last() {
        let tokens = scan_with_eof("hello world");
        let last = tokens
            .last()
            .expect("scan_with_eof should produce at least one token");
        assert_eq!(last.tag, RawTag::Eof);
    }

    #[test]
    fn repeated_eof_returns_eof() {
        let buf = SourceBuffer::new("");
        let mut scanner = RawScanner::new(buf.cursor());
        for _ in 0..5 {
            let tok = scanner.next_token();
            assert_eq!(tok.tag, RawTag::Eof);
            assert_eq!(tok.len, 0);
        }
    }

    #[test]
    fn template_depth_empty_after_complete_scan() {
        let sources = [
            "`hello`",
            "`{x}`",
            "`{a} and {b}`",
            "`outer {`inner {x}`}`",
            "let x = `{1 + 2}`",
        ];
        for source in sources {
            let buf = SourceBuffer::new(source);
            let mut scanner = RawScanner::new(buf.cursor());
            loop {
                let tok = scanner.next_token();
                if tok.tag == RawTag::Eof {
                    break;
                }
            }
            assert!(
                scanner.template_depth.is_empty(),
                "template_depth not empty after scanning {source:?}",
            );
        }
    }

    // ─── Byte Coverage ─────────────────────────────────────────────

    #[test]
    fn all_256_bytes_produce_valid_token() {
        for byte in 0u8..=255 {
            let source = [byte];
            // We need valid UTF-8 for SourceBuffer, so use from_utf8_lossy
            // For non-UTF-8 bytes, we test via raw cursor construction instead
            if let Ok(s) = std::str::from_utf8(&source) {
                let buf = SourceBuffer::new(s);
                let mut scanner = RawScanner::new(buf.cursor());
                let tok = scanner.next_token();
                // Should not panic and should produce a token
                assert!(
                    tok.tag == RawTag::Eof || tok.len > 0,
                    "byte {byte} produced invalid token: {tok:?}",
                );
            }
        }
    }

    #[test]
    fn all_printable_ascii_produce_valid_tokens() {
        for byte in 32u8..=126 {
            let bytes = [byte];
            let source = std::str::from_utf8(&bytes).expect("printable ASCII is valid UTF-8");
            let tokens = scan(source);
            let total_len: u32 = tokens.iter().map(|t| t.len).sum();
            assert_eq!(
                total_len, 1,
                "byte {:?} ({}) produced total_len={}, tokens={:?}",
                byte as char, byte, total_len, tokens
            );
        }
    }

    // ─── Whitespace & Newlines ─────────────────────────────────────

    #[test]
    fn whitespace_spaces_and_tabs() {
        assert_eq!(scan_tags("   "), vec![RawTag::Whitespace]);
        assert_eq!(scan("   ")[0].len, 3);

        assert_eq!(scan_tags("\t\t"), vec![RawTag::Whitespace]);
        assert_eq!(scan_tags("  \t  "), vec![RawTag::Whitespace]);
    }

    #[test]
    fn newline_lf() {
        assert_eq!(scan_tags("\n"), vec![RawTag::Newline]);
        assert_eq!(scan("\n")[0].len, 1);
    }

    #[test]
    fn newline_crlf_normalized() {
        assert_eq!(scan_tags("\r\n"), vec![RawTag::Newline]);
        assert_eq!(scan("\r\n")[0].len, 2);
    }

    #[test]
    fn lone_cr_is_whitespace() {
        assert_eq!(scan_tags("\r"), vec![RawTag::Whitespace]);
        assert_eq!(scan("\r")[0].len, 1);
    }

    #[test]
    fn mixed_whitespace_and_newlines() {
        let tags = scan_tags("  \n\t\t\r\n  ");
        assert_eq!(
            tags,
            vec![
                RawTag::Whitespace, // "  "
                RawTag::Newline,    // "\n"
                RawTag::Whitespace, // "\t\t"
                RawTag::Newline,    // "\r\n"
                RawTag::Whitespace, // "  "
            ]
        );
    }

    #[test]
    fn empty_source() {
        assert_eq!(scan_tags(""), vec![]);
        let tokens = scan_with_eof("");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].tag, RawTag::Eof);
    }

    // ─── Comments ──────────────────────────────────────────────────

    #[test]
    fn line_comment() {
        assert_eq!(scan_tags("// hello"), vec![RawTag::LineComment]);
        assert_eq!(scan("// hello")[0].len, 8);
    }

    #[test]
    fn line_comment_does_not_consume_newline() {
        let tags = scan_tags("// hello\n");
        assert_eq!(tags, vec![RawTag::LineComment, RawTag::Newline]);
    }

    #[test]
    fn slash_alone() {
        assert_eq!(scan_tags("/"), vec![RawTag::Slash]);
        assert_eq!(scan("/")[0].len, 1);
    }

    #[test]
    fn slash_followed_by_non_slash() {
        let tags = scan_tags("/x");
        assert_eq!(tags, vec![RawTag::Slash, RawTag::Ident]);
    }

    // ─── Identifiers ───────────────────────────────────────────────

    #[test]
    fn simple_identifiers() {
        assert_eq!(scan_tags("foo"), vec![RawTag::Ident]);
        assert_eq!(scan("foo")[0].len, 3);

        assert_eq!(scan_tags("_foo"), vec![RawTag::Ident]);
        assert_eq!(scan("_foo")[0].len, 4);

        assert_eq!(scan_tags("foo_bar"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("FooBar"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("x1"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("_"), vec![RawTag::Underscore]);
    }

    #[test]
    fn underscore_alone_is_underscore() {
        assert_eq!(scan_tags("_"), vec![RawTag::Underscore]);
        assert_eq!(scan("_")[0].len, 1);
    }

    #[test]
    fn underscore_followed_by_space() {
        let tags = scan_tags("_ x");
        assert_eq!(
            tags,
            vec![RawTag::Underscore, RawTag::Whitespace, RawTag::Ident]
        );
    }

    #[test]
    fn underscore_followed_by_ident() {
        assert_eq!(scan_tags("_x"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("__"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("_0"), vec![RawTag::Ident]);
    }

    #[test]
    fn keywords_are_ident() {
        // Raw scanner does not resolve keywords
        assert_eq!(scan_tags("let"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("if"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("fn"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("true"), vec![RawTag::Ident]);
        assert_eq!(scan_tags("false"), vec![RawTag::Ident]);
    }

    // ─── Operators (single-char) ───────────────────────────────────

    #[test]
    fn single_char_operators() {
        assert_eq!(scan_tags("+"), vec![RawTag::Plus]);
        assert_eq!(scan_tags("-"), vec![RawTag::Minus]);
        assert_eq!(scan_tags("*"), vec![RawTag::Star]);
        assert_eq!(scan_tags("/"), vec![RawTag::Slash]);
        assert_eq!(scan_tags("%"), vec![RawTag::Percent]);
        assert_eq!(scan_tags("^"), vec![RawTag::Caret]);
        assert_eq!(scan_tags("&"), vec![RawTag::Ampersand]);
        assert_eq!(scan_tags("|"), vec![RawTag::Pipe]);
        assert_eq!(scan_tags("~"), vec![RawTag::Tilde]);
        assert_eq!(scan_tags("!"), vec![RawTag::Bang]);
        assert_eq!(scan_tags("="), vec![RawTag::Equal]);
        assert_eq!(scan_tags("<"), vec![RawTag::Less]);
        assert_eq!(scan_tags(">"), vec![RawTag::Greater]);
        assert_eq!(scan_tags("."), vec![RawTag::Dot]);
        assert_eq!(scan_tags("?"), vec![RawTag::Question]);
    }

    // ─── Operators (compound) ──────────────────────────────────────

    #[test]
    fn compound_operators() {
        assert_eq!(scan_tags("=="), vec![RawTag::EqualEqual]);
        assert_eq!(scan_tags("!="), vec![RawTag::BangEqual]);
        assert_eq!(scan_tags("<="), vec![RawTag::LessEqual]);
        assert_eq!(scan_tags("&&"), vec![RawTag::AmpersandAmpersand]);
        assert_eq!(scan_tags("||"), vec![RawTag::PipePipe]);
        assert_eq!(scan_tags("->"), vec![RawTag::Arrow]);
        assert_eq!(scan_tags("=>"), vec![RawTag::FatArrow]);
        assert_eq!(scan_tags(".."), vec![RawTag::DotDot]);
        assert_eq!(scan_tags("..="), vec![RawTag::DotDotEqual]);
        assert_eq!(scan_tags("..."), vec![RawTag::DotDotDot]);
        assert_eq!(scan_tags("::"), vec![RawTag::ColonColon]);
        assert_eq!(scan_tags("<<"), vec![RawTag::Shl]);
        assert_eq!(scan_tags("??"), vec![RawTag::QuestionQuestion]);
    }

    #[test]
    fn greater_is_always_single() {
        // `>` is always a single token — parser synthesizes >= and >>
        assert_eq!(scan_tags(">="), vec![RawTag::Greater, RawTag::Equal]);
        assert_eq!(scan_tags(">>"), vec![RawTag::Greater, RawTag::Greater]);
    }

    #[test]
    fn no_compound_assignment() {
        // Ori has no compound assignment operators
        assert_eq!(scan_tags("+="), vec![RawTag::Plus, RawTag::Equal]);
        assert_eq!(scan_tags("-="), vec![RawTag::Minus, RawTag::Equal]);
        assert_eq!(scan_tags("*="), vec![RawTag::Star, RawTag::Equal]);
        assert_eq!(scan_tags("/="), vec![RawTag::Slash, RawTag::Equal]);
    }

    // ─── Delimiters ────────────────────────────────────────────────

    #[test]
    fn delimiters() {
        assert_eq!(scan_tags("("), vec![RawTag::LeftParen]);
        assert_eq!(scan_tags(")"), vec![RawTag::RightParen]);
        assert_eq!(scan_tags("["), vec![RawTag::LeftBracket]);
        assert_eq!(scan_tags("]"), vec![RawTag::RightBracket]);
        assert_eq!(scan_tags("{"), vec![RawTag::LeftBrace]);
        assert_eq!(scan_tags("}"), vec![RawTag::RightBrace]);
        assert_eq!(scan_tags(","), vec![RawTag::Comma]);
        assert_eq!(scan_tags(":"), vec![RawTag::Colon]);
        assert_eq!(scan_tags(";"), vec![RawTag::Semicolon]);
        assert_eq!(scan_tags("@"), vec![RawTag::At]);
        assert_eq!(scan_tags("$"), vec![RawTag::Dollar]);
    }

    #[test]
    fn hash_variants() {
        assert_eq!(scan_tags("#"), vec![RawTag::Hash]);
        assert_eq!(scan_tags("#["), vec![RawTag::HashBracket]);
        assert_eq!(scan_tags("#!"), vec![RawTag::HashBang]);
        assert_eq!(scan_tags("#x"), vec![RawTag::Hash, RawTag::Ident]);
    }

    #[test]
    fn backslash_is_error_detection() {
        assert_eq!(scan_tags("\\"), vec![RawTag::Backslash]);
    }

    // ─── Numeric Literals ──────────────────────────────────────────

    #[test]
    fn integer_literals() {
        assert_eq!(scan_tags("42"), vec![RawTag::Int]);
        assert_eq!(scan("42")[0].len, 2);
        assert_eq!(scan_tags("0"), vec![RawTag::Int]);
        assert_eq!(scan_tags("1_000_000"), vec![RawTag::Int]);
    }

    #[test]
    fn float_literals() {
        assert_eq!(scan_tags("3.14"), vec![RawTag::Float]);
        assert_eq!(scan("3.14")[0].len, 4);
        assert_eq!(scan_tags("0.5"), vec![RawTag::Float]);
        assert_eq!(scan_tags("1.0e10"), vec![RawTag::Float]);
        assert_eq!(scan_tags("1.0E-5"), vec![RawTag::Float]);
    }

    #[test]
    fn hex_literals() {
        assert_eq!(scan_tags("0xFF"), vec![RawTag::HexInt]);
        assert_eq!(scan_tags("0x00"), vec![RawTag::HexInt]);
        assert_eq!(scan_tags("0xDEAD_BEEF"), vec![RawTag::HexInt]);
        assert_eq!(scan_tags("0X1A"), vec![RawTag::HexInt]);
    }

    #[test]
    fn binary_literals() {
        assert_eq!(scan_tags("0b1010"), vec![RawTag::BinInt]);
        assert_eq!(scan_tags("0b00"), vec![RawTag::BinInt]);
        assert_eq!(scan_tags("0b1111_0000"), vec![RawTag::BinInt]);
        assert_eq!(scan_tags("0B10"), vec![RawTag::BinInt]);
        assert_eq!(scan_tags("0b_1010"), vec![RawTag::BinInt]);
    }

    #[test]
    fn zero_bytes_vs_binary_disambiguation() {
        // `0b` alone (not followed by binary digit) = size literal (0 bytes)
        assert_eq!(scan_tags("0b"), vec![RawTag::Size]);
        // `0b` followed by binary digit = binary integer
        assert_eq!(scan_tags("0b1"), vec![RawTag::BinInt]);
        assert_eq!(scan_tags("0b0"), vec![RawTag::BinInt]);
    }

    #[test]
    fn dot_after_int_is_not_float() {
        // `42..` should be Int then DotDot, not Float
        let tags = scan_tags("42..");
        assert_eq!(tags, vec![RawTag::Int, RawTag::DotDot]);
    }

    #[test]
    fn int_dot_ident_is_not_float() {
        // `42.foo` should be Int, Dot, Ident — not Float
        let tags = scan_tags("42.foo");
        assert_eq!(tags, vec![RawTag::Int, RawTag::Dot, RawTag::Ident]);
    }

    #[test]
    fn exponent_without_dot() {
        assert_eq!(scan_tags("1e5"), vec![RawTag::Float]);
        assert_eq!(scan_tags("1E10"), vec![RawTag::Float]);
        assert_eq!(scan_tags("1e+5"), vec![RawTag::Float]);
        assert_eq!(scan_tags("1e-5"), vec![RawTag::Float]);
    }

    // ─── Duration Literals ─────────────────────────────────────────

    #[test]
    fn duration_integer() {
        assert_eq!(scan_tags("100ns"), vec![RawTag::Duration]);
        assert_eq!(scan("100ns")[0].len, 5);
        assert_eq!(scan_tags("50us"), vec![RawTag::Duration]);
        assert_eq!(scan_tags("200ms"), vec![RawTag::Duration]);
        assert_eq!(scan_tags("5s"), vec![RawTag::Duration]);
        assert_eq!(scan_tags("10m"), vec![RawTag::Duration]);
        assert_eq!(scan_tags("2h"), vec![RawTag::Duration]);
    }

    #[test]
    fn duration_decimal() {
        // Decimal durations are valid per grammar.ebnf lines 136-137
        assert_eq!(scan_tags("0.5s"), vec![RawTag::Duration]);
        assert_eq!(scan("0.5s")[0].len, 4);
        assert_eq!(scan_tags("1.5ms"), vec![RawTag::Duration]);
        assert_eq!(scan_tags("0.25h"), vec![RawTag::Duration]);
    }

    #[test]
    fn duration_suffix_not_consumed_if_followed_by_ident() {
        // `10sec` should be Int + Ident, not Duration
        let tags = scan_tags("10sec");
        assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);

        // `10min` should be Int + Ident
        let tags = scan_tags("10min");
        assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);

        // `10hours` should be Int + Ident
        let tags = scan_tags("10hours");
        assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);
    }

    // ─── Size Literals ─────────────────────────────────────────────

    #[test]
    fn size_integer() {
        assert_eq!(scan_tags("100b"), vec![RawTag::Size]);
        assert_eq!(scan_tags("10kb"), vec![RawTag::Size]);
        assert_eq!(scan_tags("5mb"), vec![RawTag::Size]);
        assert_eq!(scan_tags("2gb"), vec![RawTag::Size]);
        assert_eq!(scan_tags("1tb"), vec![RawTag::Size]);
    }

    #[test]
    fn size_decimal() {
        assert_eq!(scan_tags("1.5kb"), vec![RawTag::Size]);
        assert_eq!(scan("1.5kb")[0].len, 5);
        assert_eq!(scan_tags("0.5mb"), vec![RawTag::Size]);
    }

    #[test]
    fn size_suffix_not_consumed_if_followed_by_ident() {
        // `10bytes` should be Int + Ident, not Size
        let tags = scan_tags("10bytes");
        assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);

        // `10kbps` should be Int + Ident
        let tags = scan_tags("10kbps");
        assert_eq!(tags, vec![RawTag::Int, RawTag::Ident]);
    }

    // ─── String Literals ───────────────────────────────────────────

    #[test]
    fn simple_string() {
        assert_eq!(scan_tags("\"hello\""), vec![RawTag::String]);
        assert_eq!(scan("\"hello\"")[0].len, 7);
    }

    #[test]
    fn empty_string() {
        assert_eq!(scan_tags("\"\""), vec![RawTag::String]);
        assert_eq!(scan("\"\"")[0].len, 2);
    }

    #[test]
    fn string_with_escape() {
        assert_eq!(scan_tags("\"he\\\"llo\""), vec![RawTag::String]);
        assert_eq!(scan_tags("\"\\n\\t\\\\\""), vec![RawTag::String]);
    }

    #[test]
    fn unterminated_string_newline() {
        assert_eq!(
            scan_tags("\"hello\n"),
            vec![RawTag::UnterminatedString, RawTag::Newline]
        );
    }

    #[test]
    fn unterminated_string_eof() {
        assert_eq!(scan_tags("\"hello"), vec![RawTag::UnterminatedString]);
    }

    #[test]
    fn adjacent_strings() {
        assert_eq!(
            scan_tags("\"a\"\"b\""),
            vec![RawTag::String, RawTag::String]
        );
    }

    // ─── Character Literals ────────────────────────────────────────

    #[test]
    fn simple_char() {
        assert_eq!(scan_tags("'x'"), vec![RawTag::Char]);
        assert_eq!(scan("'x'")[0].len, 3);
    }

    #[test]
    fn char_with_escape() {
        assert_eq!(scan_tags("'\\n'"), vec![RawTag::Char]);
        assert_eq!(scan("'\\n'")[0].len, 4);
        assert_eq!(scan_tags("'\\''"), vec![RawTag::Char]);
    }

    #[test]
    fn unterminated_char_eof() {
        assert_eq!(scan_tags("'x"), vec![RawTag::UnterminatedChar]);
    }

    #[test]
    fn empty_char_literal() {
        // '' — opening ' consumed, then immediate ' is "empty char" -> UnterminatedChar(1)
        // Then second ' starts a new char_literal, consumes opening ', hits EOF -> UnterminatedChar(1)
        let tags = scan_tags("''");
        assert_eq!(
            tags,
            vec![RawTag::UnterminatedChar, RawTag::UnterminatedChar]
        );
    }

    #[test]
    fn char_unicode_2byte() {
        // λ = U+03BB = 2 bytes (CE BB)
        assert_eq!(scan_tags("'λ'"), vec![RawTag::Char]);
        assert_eq!(scan("'λ'")[0].len, 4); // ' + 2-byte char + '
    }

    #[test]
    fn char_unicode_3byte() {
        // ñ = U+00F1 when encoded differently, use CJK: 中 = U+4E2D = 3 bytes
        assert_eq!(scan_tags("'中'"), vec![RawTag::Char]);
        assert_eq!(scan("'中'")[0].len, 5); // ' + 3-byte char + '
    }

    #[test]
    fn char_unicode_4byte() {
        // 😀 = U+1F600 = 4 bytes (F0 9F 98 80)
        assert_eq!(scan_tags("'😀'"), vec![RawTag::Char]);
        assert_eq!(scan("'😀'")[0].len, 6); // ' + 4-byte char + '
    }

    // ─── Template Literals ─────────────────────────────────────────

    #[test]
    fn template_complete() {
        assert_eq!(scan_tags("`hello`"), vec![RawTag::TemplateComplete]);
        assert_eq!(scan("`hello`")[0].len, 7);
    }

    #[test]
    fn template_empty() {
        assert_eq!(scan_tags("``"), vec![RawTag::TemplateComplete]);
        assert_eq!(scan("``")[0].len, 2);
    }

    #[test]
    fn template_single_interpolation() {
        let tags = scan_tags("`{x}`");
        assert_eq!(
            tags,
            vec![RawTag::TemplateHead, RawTag::Ident, RawTag::TemplateTail]
        );
    }

    #[test]
    fn template_with_text_and_interpolation() {
        let tags = scan_tags("`hello {name}`");
        assert_eq!(
            tags,
            vec![RawTag::TemplateHead, RawTag::Ident, RawTag::TemplateTail]
        );
    }

    #[test]
    fn template_multiple_interpolations() {
        let tags = scan_tags("`{a} and {b}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident,
                RawTag::TemplateMiddle,
                RawTag::Ident,
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_nested_braces() {
        // `{if x then {a: 1} else {b: 2}}`
        let tags = scan_tags("`{x + {a: 1}}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident,
                RawTag::Whitespace,
                RawTag::Plus,
                RawTag::Whitespace,
                RawTag::LeftBrace,
                RawTag::Ident,
                RawTag::Colon,
                RawTag::Whitespace,
                RawTag::Int,
                RawTag::RightBrace,
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_nested_templates() {
        // `outer {`inner {x}`}`
        let tags = scan_tags("`outer {`inner {x}`}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead, // `outer {
                RawTag::TemplateHead, // `inner {
                RawTag::Ident,        // x
                RawTag::TemplateTail, // }`
                RawTag::TemplateTail, // }`
            ]
        );
    }

    #[test]
    fn template_escaped_braces() {
        assert_eq!(scan_tags("`{{literal}}`"), vec![RawTag::TemplateComplete]);
    }

    #[test]
    fn template_escaped_backtick() {
        assert_eq!(
            scan_tags(r"`hello \` world`"),
            vec![RawTag::TemplateComplete]
        );
    }

    #[test]
    fn template_multiline() {
        assert_eq!(scan_tags("`line1\nline2`"), vec![RawTag::TemplateComplete]);
    }

    #[test]
    fn template_unterminated() {
        assert_eq!(scan_tags("`hello"), vec![RawTag::UnterminatedTemplate]);
    }

    #[test]
    fn template_unterminated_in_interpolation() {
        // `{x  — template opens, interpolation starts, then EOF
        // After TemplateHead + Ident, the scanner sees EOF.
        // The `}` that would trigger template_middle_or_tail never arrives.
        // The template_depth stack is orphaned — the cooking layer detects this.
        let tags = scan_tags("`{x");
        assert_eq!(tags, vec![RawTag::TemplateHead, RawTag::Ident]);

        // Verify template_depth is NOT empty (orphaned)
        let buf = SourceBuffer::new("`{x");
        let mut scanner = RawScanner::new(buf.cursor());
        loop {
            let tok = scanner.next_token();
            if tok.tag == RawTag::Eof {
                break;
            }
        }
        assert!(
            !scanner.template_depth.is_empty(),
            "template_depth should be non-empty for unterminated interpolation"
        );
    }

    // ─── Format Spec in Template Interpolation ─────────────────────

    #[test]
    fn template_format_spec_simple() {
        // `{value:x}` — simple format spec
        let tags = scan_tags("`{value:x}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident,
                RawTag::FormatSpec,
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_format_spec_complex() {
        // `{value:>10.2f}` — alignment, width, precision, type
        let tags = scan_tags("`{value:>10.2f}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident,
                RawTag::FormatSpec,
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_format_spec_zero_padded() {
        // `{value:08x}` — zero-padded hex
        let tags = scan_tags("`{value:08x}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident,
                RawTag::FormatSpec,
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_no_format_spec() {
        // `{value}` — no format spec, no FormatSpec token
        let tags = scan_tags("`{value}`");
        assert_eq!(
            tags,
            vec![RawTag::TemplateHead, RawTag::Ident, RawTag::TemplateTail]
        );
    }

    #[test]
    fn template_format_spec_empty() {
        // `{value:}` — empty format spec (all components optional)
        let tags = scan_tags("`{value:}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident,
                RawTag::FormatSpec,
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_colon_inside_parens_not_format_spec() {
        // `{func(a: b):x}` — colon inside parens is NOT format spec
        let tags = scan_tags("`{func(a: b):x}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident, // func
                RawTag::LeftParen,
                RawTag::Ident, // a
                RawTag::Colon, // : (inside parens, regular colon)
                RawTag::Whitespace,
                RawTag::Ident, // b
                RawTag::RightParen,
                RawTag::FormatSpec, // :x (at top level, format spec)
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_colon_inside_brackets_not_format_spec() {
        // `{map[k:v]:x}` — colon inside brackets is NOT format spec
        let tags = scan_tags("`{map[k:v]:x}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident, // map
                RawTag::LeftBracket,
                RawTag::Ident, // k
                RawTag::Colon, // : (inside brackets, regular colon)
                RawTag::Ident, // v
                RawTag::RightBracket,
                RawTag::FormatSpec, // :x (at top level, format spec)
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_format_spec_with_multiple_interpolations() {
        // `{a:x} and {b:>10}`
        let tags = scan_tags("`{a:x} and {b:>10}`");
        assert_eq!(
            tags,
            vec![
                RawTag::TemplateHead,
                RawTag::Ident,      // a
                RawTag::FormatSpec, // :x
                RawTag::TemplateMiddle,
                RawTag::Ident,      // b
                RawTag::FormatSpec, // :>10
                RawTag::TemplateTail,
            ]
        );
    }

    #[test]
    fn template_format_spec_length_correct() {
        // Verify FormatSpec token length includes the leading ':'
        let tokens = scan("`{x:>10.2f}`");
        // Tokens: TemplateHead(`{), Ident(x), FormatSpec(:>10.2f), TemplateTail(}`)
        let format_spec = &tokens[2];
        assert_eq!(format_spec.tag, RawTag::FormatSpec);
        assert_eq!(format_spec.len, 7); // ":>10.2f" = 7 bytes
    }

    // ─── Invalid Bytes ─────────────────────────────────────────────

    #[test]
    fn non_ascii_byte_is_invalid() {
        // Non-ASCII in UTF-8 context — the SourceBuffer accepts &str so
        // we can test with a multi-byte UTF-8 char
        let tags = scan_tags("\u{00E9}"); // é (2 bytes: 0xC3 0xA9)
                                          // Each non-ASCII byte produces InvalidByte
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().all(|t| *t == RawTag::InvalidByte));
    }

    #[test]
    fn control_chars_are_invalid() {
        // Control char 0x01
        let tags = scan_tags("\x01");
        assert_eq!(tags, vec![RawTag::InvalidByte]);
    }

    // ─── Adjacent Tokens ───────────────────────────────────────────

    #[test]
    fn adjacent_no_whitespace() {
        let tags = scan_tags("a+b");
        assert_eq!(tags, vec![RawTag::Ident, RawTag::Plus, RawTag::Ident]);
    }

    #[test]
    fn adjacent_numbers_and_operators() {
        let tags = scan_tags("1+2");
        assert_eq!(tags, vec![RawTag::Int, RawTag::Plus, RawTag::Int]);
    }

    #[test]
    fn complex_expression() {
        let tags = scan_tags("x + y * (z - 1)");
        assert_eq!(
            tags,
            vec![
                RawTag::Ident,
                RawTag::Whitespace,
                RawTag::Plus,
                RawTag::Whitespace,
                RawTag::Ident,
                RawTag::Whitespace,
                RawTag::Star,
                RawTag::Whitespace,
                RawTag::LeftParen,
                RawTag::Ident,
                RawTag::Whitespace,
                RawTag::Minus,
                RawTag::Whitespace,
                RawTag::Int,
                RawTag::RightParen,
            ]
        );
    }

    // ─── Iterator impl ────────────────────────────────────────────

    #[test]
    fn iterator_yields_tokens_then_none() {
        let buf = SourceBuffer::new("a b");
        let scanner = RawScanner::new(buf.cursor());
        let tokens: Vec<_> = scanner.collect();
        assert_eq!(tokens.len(), 3); // Ident, Whitespace, Ident
        assert_eq!(tokens[0].tag, RawTag::Ident);
        assert_eq!(tokens[1].tag, RawTag::Whitespace);
        assert_eq!(tokens[2].tag, RawTag::Ident);
    }

    // ─── Tokenize convenience function ─────────────────────────────

    #[test]
    fn tokenize_convenience() {
        let tokens = tokenize("1 + 2");
        assert_eq!(tokens.len(), 5); // Int, WS, Plus, WS, Int
        assert_eq!(tokens[0].tag, RawTag::Int);
        assert_eq!(tokens[2].tag, RawTag::Plus);
        assert_eq!(tokens[4].tag, RawTag::Int);
    }

    // ─── Realistic Ori Code ────────────────────────────────────────

    #[test]
    fn realistic_let_binding() {
        let source = "let x = 42";
        let tags = scan_tags(source);
        assert_eq!(
            tags,
            vec![
                RawTag::Ident, // let
                RawTag::Whitespace,
                RawTag::Ident, // x
                RawTag::Whitespace,
                RawTag::Equal, // =
                RawTag::Whitespace,
                RawTag::Int, // 42
            ]
        );
    }

    #[test]
    fn realistic_function_def() {
        let source = "fn add(a: int, b: int) -> int";
        let tags = scan_tags(source);
        assert_eq!(
            tags,
            vec![
                RawTag::Ident, // fn
                RawTag::Whitespace,
                RawTag::Ident, // add
                RawTag::LeftParen,
                RawTag::Ident, // a
                RawTag::Colon,
                RawTag::Whitespace,
                RawTag::Ident, // int
                RawTag::Comma,
                RawTag::Whitespace,
                RawTag::Ident, // b
                RawTag::Colon,
                RawTag::Whitespace,
                RawTag::Ident, // int
                RawTag::RightParen,
                RawTag::Whitespace,
                RawTag::Arrow, // ->
                RawTag::Whitespace,
                RawTag::Ident, // int
            ]
        );
    }

    #[test]
    fn realistic_attribute_and_test() {
        let source = "@test tests\n@target () -> void";
        let tags = scan_tags(source);
        assert_eq!(
            tags,
            vec![
                RawTag::At,
                RawTag::Ident, // test
                RawTag::Whitespace,
                RawTag::Ident, // tests
                RawTag::Newline,
                RawTag::At,
                RawTag::Ident, // target
                RawTag::Whitespace,
                RawTag::LeftParen,
                RawTag::RightParen,
                RawTag::Whitespace,
                RawTag::Arrow,
                RawTag::Whitespace,
                RawTag::Ident, // void
            ]
        );
    }
}
