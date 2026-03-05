//! String, char, and template escape cooking for `TokenCooker`.
//!
//! Handles unescape processing + interning for string/char/template tokens.
//! Each method strips surrounding delimiters, runs the appropriate unescape
//! function, interns the result, and reports errors.

use ori_ir::TokenKind;

use super::{slice_source, CookResult, TokenCooker};
use crate::cook_escape::{unescape_char_v2, unescape_string_v2, unescape_template_v2};

impl TokenCooker<'_> {
    /// Cook a string literal: strip quotes, unescape, intern.
    pub(super) fn cook_string(&mut self, offset: u32, len: u32) -> CookResult {
        let errors_before = self.errors.len();
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
        let kind = TokenKind::String(name);
        if self.errors.len() > errors_before {
            CookResult::with_error(kind)
        } else {
            CookResult::new(kind)
        }
    }

    /// Cook a char literal: strip quotes, unescape, return char value.
    pub(super) fn cook_char(&mut self, offset: u32, len: u32) -> CookResult {
        let errors_before = self.errors.len();
        let text = slice_source(self.source, offset, len);
        // Strip surrounding quotes
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let c = unescape_char_v2(content, content_offset, &mut self.errors);
        let kind = TokenKind::Char(c);
        if self.errors.len() > errors_before {
            CookResult::with_error(kind)
        } else {
            CookResult::new(kind)
        }
    }

    /// Cook a template head: strip `` ` `` and `{`, unescape, intern.
    pub(super) fn cook_template_head(&mut self, offset: u32, len: u32) -> CookResult {
        let errors_before = self.errors.len();
        let text = slice_source(self.source, offset, len);
        // Strip leading ` and trailing {
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => self.interner.intern(content),
        };
        let kind = TokenKind::TemplateHead(name);
        if self.errors.len() > errors_before {
            CookResult::with_error(kind)
        } else {
            CookResult::new(kind)
        }
    }

    /// Cook a template middle segment: strip `}` and `{`, unescape, intern.
    pub(super) fn cook_template_middle(&mut self, offset: u32, len: u32) -> CookResult {
        let errors_before = self.errors.len();
        let text = slice_source(self.source, offset, len);
        // Strip leading } and trailing {
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => self.interner.intern(content),
        };
        let kind = TokenKind::TemplateMiddle(name);
        if self.errors.len() > errors_before {
            CookResult::with_error(kind)
        } else {
            CookResult::new(kind)
        }
    }

    /// Cook a template tail: strip `}` and `` ` ``, unescape, intern.
    pub(super) fn cook_template_tail(&mut self, offset: u32, len: u32) -> CookResult {
        let errors_before = self.errors.len();
        let text = slice_source(self.source, offset, len);
        // Strip leading } and trailing `
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => self.interner.intern(content),
        };
        let kind = TokenKind::TemplateTail(name);
        if self.errors.len() > errors_before {
            CookResult::with_error(kind)
        } else {
            CookResult::new(kind)
        }
    }

    /// Cook a format spec: strip leading `:`, intern the spec content.
    pub(super) fn cook_format_spec(&self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);
        // The format spec token includes the leading `:` from the scanner.
        // Strip it to get just the spec content.
        let content = &text[1..];
        CookResult::new(TokenKind::FormatSpec(self.interner.intern(content)))
    }

    /// Cook a complete template (no interpolation): strip both backticks, unescape, intern.
    pub(super) fn cook_template_complete(&mut self, offset: u32, len: u32) -> CookResult {
        let errors_before = self.errors.len();
        let text = slice_source(self.source, offset, len);
        // Strip both backticks
        let content = &text[1..text.len() - 1];
        let content_offset = offset + 1;

        let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
            Some(unescaped) => self.interner.intern_owned(unescaped),
            None => self.interner.intern(content),
        };
        let kind = TokenKind::TemplateFull(name);
        if self.errors.len() > errors_before {
            CookResult::with_error(kind)
        } else {
            CookResult::new(kind)
        }
    }
}
