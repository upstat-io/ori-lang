//! Numeric literal cooking: integer (decimal, hex, binary) and float parsing.

use ori_ir::TokenKind;

use super::{slice_source, span, CookResult, TokenCooker};
use crate::lex_error::LexError;
use crate::parse_helpers::{parse_float_skip_underscores, parse_int_skip_underscores};

impl TokenCooker<'_> {
    #[inline]
    pub(super) fn cook_int(&mut self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);
        if let Some(n) = parse_int_skip_underscores(text, 10) {
            CookResult::new(TokenKind::Int(n))
        } else {
            self.errors.push(LexError::int_overflow(span(offset, len)));
            CookResult::with_error(TokenKind::Error)
        }
    }

    pub(super) fn cook_hex_int(&mut self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);
        // Strip the 0x/0X prefix
        let hex_part = &text[2..];
        if let Some(n) = parse_int_skip_underscores(hex_part, 16) {
            CookResult::new(TokenKind::Int(n))
        } else {
            self.errors
                .push(LexError::hex_int_overflow(span(offset, len)));
            CookResult::with_error(TokenKind::Error)
        }
    }

    pub(super) fn cook_bin_int(&mut self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);
        // Strip the 0b/0B prefix
        let bin_part = &text[2..];
        if let Some(n) = parse_int_skip_underscores(bin_part, 2) {
            CookResult::new(TokenKind::Int(n))
        } else {
            self.errors
                .push(LexError::bin_int_overflow(span(offset, len)));
            CookResult::with_error(TokenKind::Error)
        }
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
