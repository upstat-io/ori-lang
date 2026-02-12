//! Token cooking layer for the V2 lexer.
//!
//! Transforms `(RawTag, len)` pairs from the raw scanner into the parser's
//! `TokenKind` values with string interning, keyword resolution, escape
//! processing, and numeric parsing.
//!
//! # Architecture
//!
//! The cooker sits between the raw scanner (`ori_lexer_core`) and the parser:
//!
//! ```text
//! source → RawScanner → (RawTag, len) → TokenCooker → TokenKind
//! ```
//!
//! Each `RawTag` category has a dedicated cooking path:
//! - **Operators/delimiters**: Direct 1:1 mapping (no data)
//! - **Identifiers**: Keyword lookup → intern
//! - **Numerics**: Parse value, detect overflow
//! - **Strings/chars**: Unescape + intern
//! - **Templates**: Unescape + intern
//! - **Duration/size**: Parse value + detect suffix
//! - **Errors**: Push `LexError`, return `TokenKind::Error`

use ori_ir::{DurationUnit, SizeUnit, StringInterner, TokenKind};
use ori_lexer_core::RawTag;

use crate::cook_escape::{unescape_char_v2, unescape_string_v2, unescape_template_v2};
use crate::keywords;
use crate::lex_error::{LexError, LexSuggestion};
use crate::parse_helpers::{parse_float_skip_underscores, parse_int_skip_underscores};
use crate::unicode_confusables;
use crate::what_is_next::{self, NextContext};

/// Cooks raw tokens into parser-ready `TokenKind` values.
///
/// Stateless with respect to individual tokens — each `cook()` call is
/// independent. Accumulates errors for the entire file.
pub(crate) struct TokenCooker<'src> {
    source: &'src [u8],
    interner: &'src StringInterner,
    errors: Vec<LexError>,
    /// Number of errors before the current `cook()` call.
    /// Used by `last_cook_had_error()` to detect errors added during cooking.
    errors_before_cook: usize,
    /// Set to `true` when the current `cook()` resolves a context-sensitive keyword.
    contextual_kw: bool,
}

impl<'src> TokenCooker<'src> {
    /// Create a new cooker for the given source.
    pub(crate) fn new(source: &'src [u8], interner: &'src StringInterner) -> Self {
        Self {
            source,
            interner,
            errors: Vec::new(),
            errors_before_cook: 0,
            contextual_kw: false,
        }
    }

    /// Consume the cooker, returning accumulated errors.
    pub(crate) fn into_errors(self) -> Vec<LexError> {
        self.errors
    }

    /// Get a reference to accumulated errors.
    #[cfg(test)]
    pub(crate) fn errors(&self) -> &[LexError] {
        &self.errors
    }

    /// Check if the most recent `cook()` call added any errors.
    ///
    /// Used by the driver loop to set `TokenFlags::HAS_ERROR` on the token.
    pub(crate) fn last_cook_had_error(&self) -> bool {
        self.errors.len() > self.errors_before_cook
    }

    /// Check if the most recent `cook()` resolved a context-sensitive keyword.
    ///
    /// Used by the driver loop to set `TokenFlags::CONTEXTUAL_KW` on the token.
    pub(crate) fn last_cook_was_contextual_kw(&self) -> bool {
        self.contextual_kw
    }

    /// Cook a single raw token into a `TokenKind`.
    ///
    /// `offset` is the byte position of the token in source.
    /// `len` is the byte length of the token.
    #[inline]
    pub(crate) fn cook(&mut self, tag: RawTag, offset: u32, len: u32) -> TokenKind {
        self.errors_before_cook = self.errors.len();
        self.contextual_kw = false;
        match tag {
            // Direct-map operators
            RawTag::Plus => TokenKind::Plus,
            RawTag::Minus => TokenKind::Minus,
            RawTag::Star => TokenKind::Star,
            RawTag::Slash => TokenKind::Slash,
            RawTag::Percent => TokenKind::Percent,
            RawTag::Caret => TokenKind::Caret,
            RawTag::Ampersand => TokenKind::Amp,
            RawTag::Pipe => TokenKind::Pipe,
            RawTag::Tilde => TokenKind::Tilde,
            RawTag::Bang => TokenKind::Bang,
            RawTag::Equal => TokenKind::Eq,
            RawTag::Less => TokenKind::Lt,
            RawTag::Greater => TokenKind::Gt,
            RawTag::Dot => TokenKind::Dot,
            RawTag::Question => TokenKind::Question,

            // Compound operators
            RawTag::EqualEqual => TokenKind::EqEq,
            RawTag::BangEqual => TokenKind::NotEq,
            RawTag::LessEqual => TokenKind::LtEq,
            RawTag::AmpersandAmpersand => TokenKind::AmpAmp,
            RawTag::PipePipe => TokenKind::PipePipe,
            RawTag::Arrow => TokenKind::Arrow,
            RawTag::FatArrow => TokenKind::FatArrow,
            RawTag::DotDot => TokenKind::DotDot,
            RawTag::DotDotEqual => TokenKind::DotDotEq,
            RawTag::DotDotDot => TokenKind::DotDotDot,
            RawTag::ColonColon => TokenKind::DoubleColon,
            RawTag::Shl => TokenKind::Shl,
            RawTag::QuestionQuestion => TokenKind::DoubleQuestion,

            // Delimiters
            RawTag::LeftParen => TokenKind::LParen,
            RawTag::RightParen => TokenKind::RParen,
            RawTag::LeftBracket => TokenKind::LBracket,
            RawTag::RightBracket => TokenKind::RBracket,
            RawTag::LeftBrace => TokenKind::LBrace,
            RawTag::RightBrace => TokenKind::RBrace,
            RawTag::Comma => TokenKind::Comma,
            RawTag::Colon => TokenKind::Colon,
            RawTag::Semicolon => {
                self.errors.push(LexError::semicolon(span(offset, len)));
                TokenKind::Semicolon
            }
            RawTag::At => TokenKind::At,
            RawTag::Hash => TokenKind::Hash,
            RawTag::Underscore => TokenKind::Underscore,
            RawTag::Dollar => TokenKind::Dollar,
            RawTag::HashBracket => TokenKind::HashBracket,
            RawTag::HashBang => TokenKind::HashBang,

            // Identifiers
            RawTag::Ident => self.cook_ident(offset, len),

            // Numeric literals
            RawTag::Int => self.cook_int(offset, len),
            RawTag::HexInt => self.cook_hex_int(offset, len),
            RawTag::BinInt => self.cook_bin_int(offset, len),
            RawTag::Float => self.cook_float(offset, len),

            // Duration/size
            RawTag::Duration => self.cook_duration(offset, len),
            RawTag::Size => self.cook_size(offset, len),

            // String/char
            RawTag::String => self.cook_string(offset, len),
            RawTag::Char => self.cook_char(offset, len),

            // Template literals
            RawTag::TemplateHead => self.cook_template_head(offset, len),
            RawTag::TemplateMiddle => self.cook_template_middle(offset, len),
            RawTag::TemplateTail => self.cook_template_tail(offset, len),
            RawTag::TemplateComplete => self.cook_template_complete(offset, len),
            RawTag::FormatSpec => self.cook_format_spec(offset, len),

            // Error tags
            RawTag::InvalidByte => self.cook_invalid_byte(offset, len),
            RawTag::UnterminatedString => {
                self.errors
                    .push(LexError::unterminated_string(span(offset, len)));
                TokenKind::Error
            }
            RawTag::UnterminatedChar => {
                self.errors
                    .push(LexError::unterminated_char(span(offset, len)));
                TokenKind::Error
            }
            RawTag::UnterminatedTemplate => {
                self.errors
                    .push(LexError::unterminated_template(span(offset, len)));
                TokenKind::Error
            }
            RawTag::Backslash => {
                self.errors
                    .push(LexError::standalone_backslash(span(offset, len)));
                TokenKind::Error
            }
            // Defensive: the raw scanner does not currently emit InvalidEscape
            // (escape validation is deferred to the cooking layer's unescape_*_v2
            // functions), but this arm handles the reserved variant for forward
            // compatibility.
            RawTag::InvalidEscape => {
                let text = slice_source(self.source, offset, len);
                let esc_char = text.chars().nth(1).unwrap_or('?');
                self.errors
                    .push(LexError::invalid_string_escape(span(offset, len), esc_char));
                TokenKind::Error
            }
            // Trivia (should not reach cook — handled by driver)
            RawTag::Whitespace | RawTag::Newline | RawTag::LineComment => {
                debug_assert!(
                    false,
                    "Trivia tags should be handled by the driver loop, not cook()"
                );
                TokenKind::Error
            }

            // EOF (should not reach cook — handled by driver)
            RawTag::Eof => {
                debug_assert!(
                    false,
                    "Eof should be handled by the driver loop, not cook()"
                );
                TokenKind::Eof
            }

            // Future variants (non_exhaustive)
            _ => TokenKind::Error,
        }
    }

    // Error cooking helpers

    /// Cook an invalid byte, detecting Unicode confusables and cross-language
    /// patterns. This replaces the simple `InvalidByte` handling with
    /// context-aware diagnostics.
    fn cook_invalid_byte(&mut self, offset: u32, len: u32) -> TokenKind {
        let byte = self.source[offset as usize];
        let err_span = span(offset, len);

        // Try to decode as UTF-8 for Unicode confusable detection
        if byte >= 0x80 {
            if let Ok(s) = std::str::from_utf8(&self.source[offset as usize..]) {
                if let Some(ch) = s.chars().next() {
                    if let Some((suggested, name)) = unicode_confusables::lookup_confusable(ch) {
                        // Span should cover the full multi-byte character
                        // char::len_utf8() is always 1..=4, safe to truncate
                        #[allow(
                            clippy::cast_possible_truncation,
                            reason = "char::len_utf8() is 1..=4, fits u32"
                        )]
                        let char_len = ch.len_utf8() as u32;
                        let full_span = span(offset, char_len);
                        self.errors
                            .push(LexError::unicode_confusable(full_span, ch, suggested, name));
                        return TokenKind::Error;
                    }
                }
            }
        }

        // Use what_is_next to provide context-aware suggestions
        let ctx = what_is_next::what_is_next(self.source, offset);
        let mut err = LexError::invalid_byte(err_span, byte);
        if let NextContext::Unicode(ch) = ctx {
            err = err.with_suggestion(LexSuggestion::text(
                format!("unexpected Unicode character `{ch}`"),
                0,
            ));
        }

        self.errors.push(err);
        TokenKind::Error
    }

    // Cooking helpers

    #[inline]
    fn cook_ident(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        if let Some(kw) = keywords::lookup(text) {
            return kw;
        }
        // Pre-filter: only attempt soft keyword lookup when length + first byte
        // match one of the 6 soft keywords. Eliminates >99% of binary searches.
        if keywords::could_be_soft_keyword(text) {
            let rest = &self.source[(offset + len) as usize..];
            if let Some(kw) = keywords::soft_keyword_lookup(text, rest) {
                self.contextual_kw = true;
                return kw;
            }
        }
        // Pre-filter: only attempt reserved-future lookup when length + first byte
        // match one of the 5 reserved-future keywords.
        if keywords::could_be_reserved_future(text) {
            if let Some(keyword) = keywords::reserved_future_lookup(text) {
                self.errors.push(LexError::reserved_future_keyword(
                    span(offset, len),
                    keyword,
                ));
                // Still lex as an identifier so the parser can continue
            }
        }
        TokenKind::Ident(self.interner.intern(text))
    }

    #[inline]
    fn cook_int(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        if let Some(n) = parse_int_skip_underscores(text, 10) {
            TokenKind::Int(n)
        } else {
            self.errors.push(LexError::int_overflow(span(offset, len)));
            TokenKind::Error
        }
    }

    fn cook_hex_int(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // Strip the 0x/0X prefix
        let hex_part = &text[2..];
        if let Some(n) = parse_int_skip_underscores(hex_part, 16) {
            TokenKind::Int(n)
        } else {
            self.errors
                .push(LexError::hex_int_overflow(span(offset, len)));
            TokenKind::Error
        }
    }

    fn cook_bin_int(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // Strip the 0b/0B prefix
        let bin_part = &text[2..];
        if let Some(n) = parse_int_skip_underscores(bin_part, 2) {
            TokenKind::Int(n)
        } else {
            self.errors
                .push(LexError::bin_int_overflow(span(offset, len)));
            TokenKind::Error
        }
    }

    fn cook_float(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        if let Some(f) = parse_float_skip_underscores(text) {
            TokenKind::Float(f.to_bits())
        } else {
            self.errors
                .push(LexError::float_parse_error(span(offset, len)));
            TokenKind::Error
        }
    }

    fn cook_duration(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);

        // Detect suffix by matching from the end
        let (suffix_len, unit) = detect_duration_suffix(text);
        if suffix_len == 0 {
            // Shouldn't happen with valid raw tokens, but be safe
            self.errors.push(LexError::int_overflow(span(offset, len)));
            return TokenKind::Error;
        }

        let num_part = &text[..text.len() - suffix_len];

        if num_part.contains('.') {
            // Decimal duration: convert to nanoseconds via integer arithmetic.
            // Spec: "Decimal syntax is compile-time sugar computed via integer
            // arithmetic — no floating-point operations are involved."
            if let Some(nanos) = parse_decimal_unit_value(num_part, unit.nanos_multiplier()) {
                TokenKind::Duration(nanos, DurationUnit::Nanoseconds)
            } else {
                self.errors
                    .push(LexError::decimal_not_representable(span(offset, len)));
                TokenKind::Error
            }
        } else if let Ok(value) = num_part.parse::<u64>() {
            TokenKind::Duration(value, unit)
        } else {
            self.errors.push(LexError::int_overflow(span(offset, len)));
            TokenKind::Error
        }
    }

    fn cook_size(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);

        let (suffix_len, unit) = detect_size_suffix(text);
        if suffix_len == 0 {
            self.errors.push(LexError::int_overflow(span(offset, len)));
            return TokenKind::Error;
        }

        let num_part = &text[..text.len() - suffix_len];

        if num_part.contains('.') {
            // Decimal size: convert to bytes via integer arithmetic.
            if let Some(bytes) = parse_decimal_unit_value(num_part, unit.bytes_multiplier()) {
                TokenKind::Size(bytes, SizeUnit::Bytes)
            } else {
                self.errors
                    .push(LexError::decimal_not_representable(span(offset, len)));
                TokenKind::Error
            }
        } else if let Ok(value) = num_part.parse::<u64>() {
            TokenKind::Size(value, unit)
        } else {
            self.errors.push(LexError::int_overflow(span(offset, len)));
            TokenKind::Error
        }
    }

    fn cook_string(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // Strip surrounding quotes
        let content = &text[1..text.len() - 1];
        // base_offset is one past the opening quote
        let content_offset = offset + 1;

        let name = match unescape_string_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => {
                // Fast path: no escapes, intern source slice directly
                self.interner.intern(content)
            }
        };
        TokenKind::String(name)
    }

    fn cook_char(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // Strip surrounding quotes
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let c = unescape_char_v2(content, content_offset, &mut self.errors);
        TokenKind::Char(c)
    }

    fn cook_template_head(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // Strip leading ` and trailing {
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => self.interner.intern(content),
        };
        TokenKind::TemplateHead(name)
    }

    fn cook_template_middle(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // Strip leading } and trailing {
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => self.interner.intern(content),
        };
        TokenKind::TemplateMiddle(name)
    }

    fn cook_template_tail(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // Strip leading } and trailing `
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => self.interner.intern(content),
        };
        TokenKind::TemplateTail(name)
    }

    fn cook_format_spec(&self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // The format spec token includes the leading `:` from the scanner.
        // Strip it to get just the spec content.
        let content = &text[1..];
        TokenKind::FormatSpec(self.interner.intern(content))
    }

    fn cook_template_complete(&mut self, offset: u32, len: u32) -> TokenKind {
        let text = slice_source(self.source, offset, len);
        // Strip both backticks
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => self.interner.intern(content),
        };
        TokenKind::TemplateFull(name)
    }
}

/// Parse a decimal number string and convert to base units using integer arithmetic.
///
/// Given `num_part` (e.g., `"1.5"`) and `multiplier` (e.g., `1_000_000_000` for seconds→ns),
/// computes the exact integer result. Returns `None` if the result is not a whole number
/// (e.g., `1.5` nanoseconds) or on overflow.
///
/// Algorithm: parse integer and fractional parts separately, then combine:
///   `result = integer_part * multiplier + fractional_digits * multiplier / 10^(num_frac_digits)`
///
/// The fractional contribution must divide evenly (no remainder) to be representable.
fn parse_decimal_unit_value(num_part: &str, multiplier: u64) -> Option<u64> {
    let mut integer_part: u64 = 0;
    let mut frac_digits: u64 = 0;
    let mut frac_digit_count: u32 = 0;
    let mut in_fraction = false;

    for &byte in num_part.as_bytes() {
        match byte {
            b'0'..=b'9' => {
                let digit = u64::from(byte - b'0');
                if in_fraction {
                    frac_digits = frac_digits.checked_mul(10)?.checked_add(digit)?;
                    frac_digit_count += 1;
                } else {
                    integer_part = integer_part.checked_mul(10)?.checked_add(digit)?;
                }
            }
            b'.' => {
                in_fraction = true;
            }
            b'_' => {}  // skip underscores
            _ => break, // hit suffix (shouldn't happen — caller strips suffix)
        }
    }

    // integer_contribution = integer_part * multiplier
    let integer_contribution = integer_part.checked_mul(multiplier)?;

    if frac_digit_count == 0 {
        return Some(integer_contribution);
    }

    // frac_divisor = 10^frac_digit_count
    let frac_divisor = 10u64.checked_pow(frac_digit_count)?;

    // frac_contribution = frac_digits * multiplier / frac_divisor
    // Must divide evenly for the result to be a whole number of base units.
    let frac_numerator = frac_digits.checked_mul(multiplier)?;
    if frac_numerator % frac_divisor != 0 {
        return None; // not representable as whole base units
    }
    let frac_contribution = frac_numerator / frac_divisor;

    integer_contribution.checked_add(frac_contribution)
}

/// Extract a str slice from source bytes at the given offset and length.
///
/// # Safety
///
/// Source originates from `SourceBuffer` (`&str` → `&[u8]`), so all bytes are
/// valid UTF-8. The raw scanner only splits at ASCII byte boundaries (operators,
/// whitespace, delimiters), which are always valid UTF-8 codepoint boundaries.
/// String/template content is a substring of the original valid UTF-8 at
/// codepoint boundaries. `debug_assert!` catches scanner bugs in debug builds.
#[inline]
#[allow(
    unsafe_code,
    reason = "hot path: source is &str, scanner splits on ASCII boundaries"
)]
fn slice_source(source: &[u8], offset: u32, len: u32) -> &str {
    let start = offset as usize;
    let end = start + len as usize;
    debug_assert!(
        std::str::from_utf8(&source[start..end]).is_ok(),
        "non-UTF-8 token at {start}..{end}"
    );
    // SAFETY: source was a &str; scanner only produces token boundaries
    // at valid UTF-8 codepoint boundaries.
    unsafe { std::str::from_utf8_unchecked(&source[start..end]) }
}

/// Create a span from offset and length.
#[inline]
fn span(offset: u32, len: u32) -> ori_ir::Span {
    ori_ir::Span::new(offset, offset + len)
}

/// Detect duration suffix and return (`suffix_len`, unit).
fn detect_duration_suffix(text: &str) -> (usize, DurationUnit) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    if n >= 2 {
        match (bytes[n - 2], bytes[n - 1]) {
            (b'n', b's') => return (2, DurationUnit::Nanoseconds),
            (b'u', b's') => return (2, DurationUnit::Microseconds),
            (b'm', b's') => return (2, DurationUnit::Milliseconds),
            _ => {}
        }
    }
    if n >= 1 {
        match bytes[n - 1] {
            b's' => return (1, DurationUnit::Seconds),
            b'm' => return (1, DurationUnit::Minutes),
            b'h' => return (1, DurationUnit::Hours),
            _ => {}
        }
    }
    (0, DurationUnit::Seconds)
}

/// Detect size suffix and return (`suffix_len`, unit).
fn detect_size_suffix(text: &str) -> (usize, SizeUnit) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    if n >= 2 {
        match (bytes[n - 2], bytes[n - 1]) {
            (b'k', b'b') => return (2, SizeUnit::Kilobytes),
            (b'm', b'b') => return (2, SizeUnit::Megabytes),
            (b'g', b'b') => return (2, SizeUnit::Gigabytes),
            (b't', b'b') => return (2, SizeUnit::Terabytes),
            _ => {}
        }
    }
    if n >= 1 && bytes[n - 1] == b'b' {
        return (1, SizeUnit::Bytes);
    }
    (0, SizeUnit::Bytes)
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    reason = "test code: source lengths always fit u32"
)]
mod tests {
    use super::*;

    // === Operator mapping ===

    #[test]
    fn direct_map_operators() {
        let source = "+-*/%^&|~!=<>.?";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);

        assert_eq!(cooker.cook(RawTag::Plus, 0, 1), TokenKind::Plus);
        assert_eq!(cooker.cook(RawTag::Minus, 1, 1), TokenKind::Minus);
        assert_eq!(cooker.cook(RawTag::Star, 2, 1), TokenKind::Star);
        assert_eq!(cooker.cook(RawTag::Slash, 3, 1), TokenKind::Slash);
        assert_eq!(cooker.cook(RawTag::Percent, 4, 1), TokenKind::Percent);
        assert_eq!(cooker.cook(RawTag::Caret, 5, 1), TokenKind::Caret);
        assert_eq!(cooker.cook(RawTag::Ampersand, 6, 1), TokenKind::Amp);
        assert_eq!(cooker.cook(RawTag::Pipe, 7, 1), TokenKind::Pipe);
        assert_eq!(cooker.cook(RawTag::Tilde, 8, 1), TokenKind::Tilde);
        assert_eq!(cooker.cook(RawTag::Bang, 9, 1), TokenKind::Bang);
        assert_eq!(cooker.cook(RawTag::Equal, 10, 1), TokenKind::Eq);
        assert_eq!(cooker.cook(RawTag::Less, 11, 1), TokenKind::Lt);
        assert_eq!(cooker.cook(RawTag::Greater, 12, 1), TokenKind::Gt);
        assert_eq!(cooker.cook(RawTag::Dot, 13, 1), TokenKind::Dot);
        assert_eq!(cooker.cook(RawTag::Question, 14, 1), TokenKind::Question);
        assert!(cooker.errors().is_empty());
    }

    #[test]
    fn compound_operators() {
        let source = "== != <= && || -> => .. ..= ... :: << ??";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);

        assert_eq!(cooker.cook(RawTag::EqualEqual, 0, 2), TokenKind::EqEq);
        assert_eq!(cooker.cook(RawTag::BangEqual, 3, 2), TokenKind::NotEq);
        assert_eq!(cooker.cook(RawTag::LessEqual, 6, 2), TokenKind::LtEq);
        assert_eq!(
            cooker.cook(RawTag::AmpersandAmpersand, 9, 2),
            TokenKind::AmpAmp
        );
        assert_eq!(cooker.cook(RawTag::PipePipe, 12, 2), TokenKind::PipePipe);
        assert_eq!(cooker.cook(RawTag::Arrow, 15, 2), TokenKind::Arrow);
        assert_eq!(cooker.cook(RawTag::FatArrow, 18, 2), TokenKind::FatArrow);
        assert_eq!(cooker.cook(RawTag::DotDot, 21, 2), TokenKind::DotDot);
        assert_eq!(cooker.cook(RawTag::DotDotEqual, 24, 3), TokenKind::DotDotEq);
        assert_eq!(cooker.cook(RawTag::DotDotDot, 28, 3), TokenKind::DotDotDot);
        assert_eq!(
            cooker.cook(RawTag::ColonColon, 32, 2),
            TokenKind::DoubleColon
        );
        assert_eq!(cooker.cook(RawTag::Shl, 35, 2), TokenKind::Shl);
        assert_eq!(
            cooker.cook(RawTag::QuestionQuestion, 38, 2),
            TokenKind::DoubleQuestion
        );
    }

    // === Identifiers and keywords ===

    #[test]
    fn identifier_interning() {
        let source = "foo";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        let cooked = cooker.cook(RawTag::Ident, 0, 3);
        match cooked {
            TokenKind::Ident(name) => assert_eq!(interner.lookup(name), "foo"),
            other => panic!("expected Ident, got {other:?}"),
        }
    }

    #[test]
    fn keyword_resolution() {
        let source = "if";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::Ident, 0, 2), TokenKind::If);
    }

    #[test]
    fn str_type_keyword() {
        let source = "str";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::Ident, 0, 3), TokenKind::StrType);
    }

    // === Numeric literals ===

    #[test]
    fn integer_literal() {
        let source = "42";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::Int, 0, 2), TokenKind::Int(42));
    }

    #[test]
    fn integer_with_underscores() {
        let source = "1_000_000";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::Int, 0, 9), TokenKind::Int(1_000_000));
    }

    #[test]
    fn hex_integer() {
        let source = "0xFF";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::HexInt, 0, 4), TokenKind::Int(255));
    }

    #[test]
    fn binary_integer() {
        let source = "0b1010";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::BinInt, 0, 6), TokenKind::Int(10));
    }

    #[test]
    fn binary_integer_with_underscores() {
        let source = "0b1111_0000";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::BinInt, 0, 11), TokenKind::Int(240));
    }

    #[test]
    #[expect(clippy::approx_constant, reason = "testing float parsing")]
    fn float_literal() {
        let source = "3.14";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Float, 0, 4),
            TokenKind::Float(3.14f64.to_bits())
        );
    }

    #[test]
    fn integer_overflow() {
        let source = "99999999999999999999999";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Int, 0, source.len() as u32),
            TokenKind::Error
        );
        assert_eq!(cooker.errors().len(), 1);
    }

    // === Duration/Size ===

    #[test]
    fn duration_milliseconds() {
        let source = "100ms";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Duration, 0, 5),
            TokenKind::Duration(100, DurationUnit::Milliseconds)
        );
    }

    #[test]
    fn duration_seconds() {
        let source = "5s";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Duration, 0, 2),
            TokenKind::Duration(5, DurationUnit::Seconds)
        );
    }

    #[test]
    fn duration_hours() {
        let source = "2h";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Duration, 0, 2),
            TokenKind::Duration(2, DurationUnit::Hours)
        );
    }

    #[test]
    fn size_kilobytes() {
        let source = "4kb";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Size, 0, 3),
            TokenKind::Size(4, SizeUnit::Kilobytes)
        );
    }

    #[test]
    fn size_bytes() {
        let source = "100b";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Size, 0, 4),
            TokenKind::Size(100, SizeUnit::Bytes)
        );
    }

    // === Decimal duration/size (spec: compile-time sugar) ===

    #[test]
    fn decimal_duration_seconds() {
        // 1.5s = 1,500,000,000 nanoseconds
        let source = "1.5s";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Duration, 0, 4),
            TokenKind::Duration(1_500_000_000, DurationUnit::Nanoseconds)
        );
        assert!(cooker.errors().is_empty());
    }

    #[test]
    fn decimal_duration_milliseconds() {
        // 2.5ms = 2,500,000 nanoseconds
        let source = "2.5ms";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Duration, 0, 5),
            TokenKind::Duration(2_500_000, DurationUnit::Nanoseconds)
        );
        assert!(cooker.errors().is_empty());
    }

    #[test]
    fn decimal_duration_hours() {
        // 2.25h = 8,100,000,000,000 nanoseconds (2h 15m)
        let source = "2.25h";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Duration, 0, 5),
            TokenKind::Duration(8_100_000_000_000, DurationUnit::Nanoseconds)
        );
        assert!(cooker.errors().is_empty());
    }

    #[test]
    fn decimal_duration_half_second() {
        // 0.5s = 500,000,000 nanoseconds
        let source = "0.5s";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Duration, 0, 4),
            TokenKind::Duration(500_000_000, DurationUnit::Nanoseconds)
        );
    }

    #[test]
    fn decimal_duration_many_digits() {
        // 1.123456789s = 1,123,456,789 nanoseconds (9 decimal places, still whole)
        let source = "1.123456789s";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Duration, 0, source.len() as u32),
            TokenKind::Duration(1_123_456_789, DurationUnit::Nanoseconds)
        );
    }

    #[test]
    fn decimal_duration_nanoseconds_error() {
        // 1.5ns = 1.5 nanoseconds — not a whole number → error
        let source = "1.5ns";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::Duration, 0, 5), TokenKind::Error);
        assert_eq!(cooker.errors().len(), 1);
    }

    #[test]
    fn decimal_size_kilobytes() {
        // 1.5kb = 1,500 bytes
        let source = "1.5kb";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Size, 0, 5),
            TokenKind::Size(1_500, SizeUnit::Bytes)
        );
        assert!(cooker.errors().is_empty());
    }

    #[test]
    fn decimal_size_megabytes() {
        // 0.25mb = 250,000 bytes
        let source = "0.25mb";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Size, 0, 6),
            TokenKind::Size(250_000, SizeUnit::Bytes)
        );
    }

    #[test]
    fn decimal_size_bytes_error() {
        // 0.5b = 0.5 bytes — not a whole number → error
        let source = "0.5b";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::Size, 0, 4), TokenKind::Error);
        assert_eq!(cooker.errors().len(), 1);
    }

    // === String literals ===

    #[test]
    fn string_simple() {
        let source = r#""hello""#;
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        let cooked = cooker.cook(RawTag::String, 0, source.len() as u32);
        match cooked {
            TokenKind::String(name) => assert_eq!(interner.lookup(name), "hello"),
            other => panic!("expected String, got {other:?}"),
        }
    }

    #[test]
    fn string_with_escapes() {
        let source = r#""hello\nworld""#;
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        let cooked = cooker.cook(RawTag::String, 0, source.len() as u32);
        match cooked {
            TokenKind::String(name) => assert_eq!(interner.lookup(name), "hello\nworld"),
            other => panic!("expected String, got {other:?}"),
        }
    }

    // === Char literals ===

    #[test]
    fn char_simple() {
        let source = "'a'";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Char, 0, source.len() as u32),
            TokenKind::Char('a')
        );
    }

    #[test]
    fn char_escape() {
        let source = r"'\n'";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(
            cooker.cook(RawTag::Char, 0, source.len() as u32),
            TokenKind::Char('\n')
        );
    }

    // === Error tokens ===

    #[test]
    fn error_tags_produce_error_kind() {
        let source = "\x01";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);

        assert_eq!(cooker.cook(RawTag::InvalidByte, 0, 1), TokenKind::Error);
        assert_eq!(cooker.errors().len(), 1);
    }

    // === Delimiter mapping ===

    #[test]
    fn delimiters() {
        let source = "()[]{},:;@#_$#[";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);

        assert_eq!(cooker.cook(RawTag::LeftParen, 0, 1), TokenKind::LParen);
        assert_eq!(cooker.cook(RawTag::RightParen, 1, 1), TokenKind::RParen);
        assert_eq!(cooker.cook(RawTag::LeftBracket, 2, 1), TokenKind::LBracket);
        assert_eq!(cooker.cook(RawTag::RightBracket, 3, 1), TokenKind::RBracket);
        assert_eq!(cooker.cook(RawTag::LeftBrace, 4, 1), TokenKind::LBrace);
        assert_eq!(cooker.cook(RawTag::RightBrace, 5, 1), TokenKind::RBrace);
        assert_eq!(cooker.cook(RawTag::Comma, 6, 1), TokenKind::Comma);
        assert_eq!(cooker.cook(RawTag::Colon, 7, 1), TokenKind::Colon);
        assert_eq!(cooker.cook(RawTag::Semicolon, 8, 1), TokenKind::Semicolon);
        assert_eq!(cooker.cook(RawTag::At, 9, 1), TokenKind::At);
        assert_eq!(cooker.cook(RawTag::Hash, 10, 1), TokenKind::Hash);
        assert_eq!(
            cooker.cook(RawTag::Underscore, 11, 1),
            TokenKind::Underscore
        );
        assert_eq!(cooker.cook(RawTag::Dollar, 12, 1), TokenKind::Dollar);
        assert_eq!(
            cooker.cook(RawTag::HashBracket, 13, 2),
            TokenKind::HashBracket
        );
    }

    #[test]
    fn hashbang_mapping() {
        let source = "#!foo";
        let interner = StringInterner::new();
        let mut cooker = TokenCooker::new(source.as_bytes(), &interner);
        assert_eq!(cooker.cook(RawTag::HashBang, 0, 2), TokenKind::HashBang);
        assert!(cooker.errors().is_empty());
    }

    // === Suffix detection ===

    #[test]
    fn duration_suffix_detection() {
        assert_eq!(
            detect_duration_suffix("100ns"),
            (2, DurationUnit::Nanoseconds)
        );
        assert_eq!(
            detect_duration_suffix("50us"),
            (2, DurationUnit::Microseconds)
        );
        assert_eq!(
            detect_duration_suffix("100ms"),
            (2, DurationUnit::Milliseconds)
        );
        assert_eq!(detect_duration_suffix("5s"), (1, DurationUnit::Seconds));
        assert_eq!(detect_duration_suffix("10m"), (1, DurationUnit::Minutes));
        assert_eq!(detect_duration_suffix("2h"), (1, DurationUnit::Hours));
    }

    #[test]
    fn size_suffix_detection() {
        assert_eq!(detect_size_suffix("100b"), (1, SizeUnit::Bytes));
        assert_eq!(detect_size_suffix("4kb"), (2, SizeUnit::Kilobytes));
        assert_eq!(detect_size_suffix("10mb"), (2, SizeUnit::Megabytes));
        assert_eq!(detect_size_suffix("1gb"), (2, SizeUnit::Gigabytes));
        assert_eq!(detect_size_suffix("1tb"), (2, SizeUnit::Terabytes));
    }

    // === Decimal unit value parsing ===

    #[test]
    fn parse_decimal_unit_value_basic() {
        // 1.5 * 1_000_000_000 (seconds→ns) = 1,500,000,000
        assert_eq!(
            parse_decimal_unit_value("1.5", 1_000_000_000),
            Some(1_500_000_000)
        );
    }

    #[test]
    fn parse_decimal_unit_value_quarter() {
        // 0.25 * 3_600_000_000_000 (hours→ns) = 900,000,000,000
        assert_eq!(
            parse_decimal_unit_value("0.25", 3_600_000_000_000),
            Some(900_000_000_000)
        );
    }

    #[test]
    fn parse_decimal_unit_value_not_representable() {
        // 1.5 * 1 (ns→ns) = 1.5 — not whole
        assert_eq!(parse_decimal_unit_value("1.5", 1), None);
    }

    #[test]
    fn parse_decimal_unit_value_no_fraction() {
        // 5. * 1000 = 5000 (degenerate: dot with no fractional digits)
        assert_eq!(parse_decimal_unit_value("5.", 1000), Some(5000));
    }

    #[test]
    fn parse_decimal_unit_value_many_digits() {
        // 1.123456789 * 1_000_000_000 = 1,123,456,789
        assert_eq!(
            parse_decimal_unit_value("1.123456789", 1_000_000_000),
            Some(1_123_456_789)
        );
    }

    #[test]
    fn parse_decimal_unit_value_with_underscores() {
        // 1_000.5 * 1_000 = 1,000,500
        assert_eq!(parse_decimal_unit_value("1_000.5", 1_000), Some(1_000_500));
    }
}
