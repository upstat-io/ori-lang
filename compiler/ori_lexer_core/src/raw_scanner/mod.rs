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

    fn plus(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '+'
        if self.cursor.current() == b'=' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::PlusEq,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Plus,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn minus_or_arrow(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '-'
        match self.cursor.current() {
            b'>' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::Arrow,
                    len: self.cursor.pos() - start,
                }
            }
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::MinusEq,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Minus,
                len: self.cursor.pos() - start,
            },
        }
    }

    fn star(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '*'
        if self.cursor.current() == b'=' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::StarEq,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Star,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn percent(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '%'
        if self.cursor.current() == b'=' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::PercentEq,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Percent,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn caret(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '^'
        if self.cursor.current() == b'=' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::CaretEq,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::Caret,
                len: self.cursor.pos() - start,
            }
        }
    }

    fn at(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '@'
        if self.cursor.current() == b'=' {
            self.cursor.advance();
            RawToken {
                tag: RawTag::AtEq,
                len: self.cursor.pos() - start,
            }
        } else {
            RawToken {
                tag: RawTag::At,
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
                // Check for <<= (shift-left-assign)
                if self.cursor.current() == b'=' {
                    self.cursor.advance();
                    RawToken {
                        tag: RawTag::ShlEq,
                        len: self.cursor.pos() - start,
                    }
                } else {
                    RawToken {
                        tag: RawTag::Shl,
                        len: self.cursor.pos() - start,
                    }
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
        match self.cursor.current() {
            b'|' => {
                self.cursor.advance();
                // Check for ||=
                if self.cursor.current() == b'=' {
                    self.cursor.advance();
                    RawToken {
                        tag: RawTag::PipePipeEq,
                        len: self.cursor.pos() - start,
                    }
                } else {
                    RawToken {
                        tag: RawTag::PipePipe,
                        len: self.cursor.pos() - start,
                    }
                }
            }
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::PipeEq,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Pipe,
                len: self.cursor.pos() - start,
            },
        }
    }

    fn ampersand(&mut self, start: u32) -> RawToken {
        self.cursor.advance(); // consume '&'
        match self.cursor.current() {
            b'&' => {
                self.cursor.advance();
                // Check for &&=
                if self.cursor.current() == b'=' {
                    self.cursor.advance();
                    RawToken {
                        tag: RawTag::AmpersandAmpersandEq,
                        len: self.cursor.pos() - start,
                    }
                } else {
                    RawToken {
                        tag: RawTag::AmpersandAmpersand,
                        len: self.cursor.pos() - start,
                    }
                }
            }
            b'=' => {
                self.cursor.advance();
                RawToken {
                    tag: RawTag::AmpersandEq,
                    len: self.cursor.pos() - start,
                }
            }
            _ => RawToken {
                tag: RawTag::Ampersand,
                len: self.cursor.pos() - start,
            },
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
mod tests;
