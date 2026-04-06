//! Numeric literal cooking: integer (decimal, hex, binary) and float parsing.

use ori_ir::{Span, TokenKind};

use super::{slice_source, span, CookResult, TokenCooker};
use crate::lex_error::LexError;
use crate::parse_helpers::{parse_float_skip_underscores, parse_int_skip_underscores};

impl TokenCooker<'_> {
    /// Shared skeleton for integer cooking: slice source, strip prefix,
    /// parse with radix, and handle overflow.
    ///
    /// The three integer formats (decimal, hex `0x`, binary `0b`) share this
    /// exact algorithm and differ only in radix, prefix length, and error factory.
    #[inline]
    fn cook_int_radix(
        &mut self,
        offset: u32,
        len: u32,
        prefix_len: usize,
        radix: u32,
        overflow_error: fn(Span) -> LexError,
    ) -> CookResult {
        let text = slice_source(self.source, offset, len);
        let digits = &text[prefix_len..];
        if let Some(n) = parse_int_skip_underscores(digits, radix) {
            CookResult::new(TokenKind::Int(n))
        } else {
            self.errors.push(overflow_error(span(offset, len)));
            CookResult::with_error(TokenKind::Error)
        }
    }

    #[inline]
    pub(super) fn cook_int(&mut self, offset: u32, len: u32) -> CookResult {
        self.cook_int_radix(offset, len, 0, 10, LexError::int_overflow)
    }

    pub(super) fn cook_hex_int(&mut self, offset: u32, len: u32) -> CookResult {
        self.cook_int_radix(offset, len, 2, 16, LexError::hex_int_overflow)
    }

    pub(super) fn cook_bin_int(&mut self, offset: u32, len: u32) -> CookResult {
        self.cook_int_radix(offset, len, 2, 2, LexError::bin_int_overflow)
    }

    pub(super) fn cook_float(&mut self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);
        if let Some(f) = parse_float_skip_underscores(text) {
            CookResult::new(TokenKind::Float(f.to_bits()))
        } else {
            self.errors
                .push(LexError::float_parse_error(span(offset, len)));
            CookResult::with_error(TokenKind::Error)
        }
    }
}
