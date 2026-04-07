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

mod duration_size;
mod escape_cooking;
mod identifier;
mod numeric;

use ori_ir::{StringInterner, TokenKind};

// Re-exported for tests (DurationUnit/SizeUnit needed by test assertions)
#[cfg(test)]
pub(crate) use ori_ir::{DurationUnit, SizeUnit};
use ori_lexer_core::RawTag;

#[cfg(test)]
pub(crate) use duration_size::{
    detect_duration_suffix, detect_size_suffix, parse_decimal_unit_value,
};

use crate::keywords;
use crate::lex_error::{LexError, LexSuggestion};
use crate::unicode_confusables;
use crate::what_is_next::{self, NextContext};

use identifier::IdentCache;

/// Result of cooking a single raw token.
///
/// Carries all metadata the driver loop needs in a single return value,
/// eliminating post-cook state polling (`last_cook_had_error()`,
/// `last_cook_was_contextual_kw()`) and redundant `discriminant_index()` calls.
#[derive(Debug)]
pub(crate) struct CookResult {
    /// The cooked token kind.
    pub kind: TokenKind,
    /// Pre-computed discriminant tag for `TokenList::push_with_tag()`.
    pub tag: u8,
    /// Whether this `cook()` call added errors to the error vec.
    pub had_error: bool,
    /// Whether this token was resolved as a contextual keyword.
    pub contextual_kw: bool,
}

const _: () = assert!(std::mem::size_of::<CookResult>() <= 24);

impl CookResult {
    /// Normal token: no error, not contextual.
    #[inline]
    fn new(kind: TokenKind) -> Self {
        let tag = kind.discriminant_index();
        Self {
            kind,
            tag,
            had_error: false,
            contextual_kw: false,
        }
    }

    /// Token that pushed an error during cooking.
    #[inline]
    fn with_error(kind: TokenKind) -> Self {
        let tag = kind.discriminant_index();
        Self {
            kind,
            tag,
            had_error: true,
            contextual_kw: false,
        }
    }

    /// Context-sensitive keyword (soft keyword with valid lookahead).
    #[inline]
    fn contextual(kind: TokenKind) -> Self {
        let tag = kind.discriminant_index();
        Self {
            kind,
            tag,
            had_error: false,
            contextual_kw: true,
        }
    }
}

/// Cooks raw tokens into parser-ready `TokenKind` values.
///
/// Stateless with respect to individual tokens — each `cook()` call is
/// independent. Accumulates errors for the entire file.
pub(crate) struct TokenCooker<'src> {
    source: &'src [u8],
    interner: &'src StringInterner,
    errors: Vec<LexError>,
    /// Direct-mapped cache of `cook_ident()` results for repeated identifiers.
    /// Fixed 256-entry array indexed by a simple hash of the identifier text.
    /// On hit, bypasses keyword lookup AND the interner entirely.
    /// Soft keywords are NOT cached (context-sensitive).
    ident_cache: IdentCache<'src>,
    /// Last non-trivia `RawTag` for O(1) method-position detection.
    /// Set by the driver loop after each non-trivia token.
    last_non_trivia_raw: Option<RawTag>,
}

impl<'src> TokenCooker<'src> {
    /// Create a new cooker for the given source.
    pub(crate) fn new(source: &'src [u8], interner: &'src StringInterner) -> Self {
        Self {
            source,
            interner,
            errors: Vec::new(),
            ident_cache: IdentCache::new(),
            last_non_trivia_raw: None,
        }
    }

    /// Record the last non-trivia raw tag for method-position detection.
    ///
    /// Called by the driver loop after processing each non-trivia token.
    /// Enables O(1) method-position checks in `cook_ident()` instead of
    /// backward source scanning.
    pub(crate) fn set_last_non_trivia(&mut self, tag: RawTag) {
        self.last_non_trivia_raw = Some(tag);
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

    /// Cook a single raw token into a `CookResult`.
    ///
    /// `offset` is the byte position of the token in source.
    /// `len` is the byte length of the token.
    ///
    /// Trivial tokens (operators, delimiters) are normally intercepted by
    /// `try_trivial()` in the driver loop before reaching this method.
    /// `Semicolon` is the exception — always reaches here.
    #[inline]
    pub(crate) fn cook(&mut self, tag: RawTag, offset: u32, len: u32) -> CookResult {
        // NOTE: In the driver loop, trivial tokens (operators, delimiters) are
        // intercepted by try_trivial() before reaching cook(). Semicolon is the
        // exception — it always reaches here. Unit tests may call cook() directly
        // with any tag; the match arms below handle all variants correctly.

        match tag {
            // Semicolon: not in try_trivial() but still a direct mapping
            RawTag::Semicolon => CookResult::new(TokenKind::Semicolon),

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
                CookResult::with_error(TokenKind::Error)
            }
            RawTag::UnterminatedChar => {
                self.errors
                    .push(LexError::unterminated_char(span(offset, len)));
                CookResult::with_error(TokenKind::Error)
            }
            RawTag::UnterminatedTemplate => {
                self.errors
                    .push(LexError::unterminated_template(span(offset, len)));
                CookResult::with_error(TokenKind::Error)
            }
            RawTag::Backslash => {
                self.errors
                    .push(LexError::standalone_backslash(span(offset, len)));
                CookResult::with_error(TokenKind::Error)
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
                CookResult::with_error(TokenKind::Error)
            }
            // Trivia and interior nulls (should not reach cook — handled by driver)
            RawTag::Whitespace | RawTag::Newline | RawTag::LineComment | RawTag::InteriorNull => {
                debug_assert!(
                    false,
                    "Trivia/InteriorNull tags should be handled by the driver loop, not cook()"
                );
                CookResult::new(TokenKind::Error)
            }

            // EOF (should not reach cook — handled by driver)
            RawTag::Eof => {
                debug_assert!(
                    false,
                    "Eof should be handled by the driver loop, not cook()"
                );
                CookResult::new(TokenKind::Eof)
            }

            // Trivial tokens (operators, delimiters, HashBang): normally intercepted
            // by try_trivial() in the driver loop, but unit tests may call cook()
            // directly. Fall through to try_trivial() as a safe catch-all.
            _ => {
                if let Some((kind, tag_byte)) = crate::trivial::try_trivial(tag) {
                    CookResult {
                        kind,
                        tag: tag_byte,
                        had_error: false,
                        contextual_kw: false,
                    }
                } else {
                    debug_assert!(false, "unhandled RawTag variant in cook(): {tag:?}");
                    CookResult::new(TokenKind::Error)
                }
            }
        }
    }

    // Error cooking helpers

    /// Cook an invalid byte, detecting Unicode confusables and cross-language
    /// patterns. This replaces the simple `InvalidByte` handling with
    /// context-aware diagnostics.
    #[cold]
    fn cook_invalid_byte(&mut self, offset: u32, len: u32) -> CookResult {
        let byte = self.source[offset as usize];
        let err_span = span(offset, len);

        // Try to decode as UTF-8 for Unicode confusable detection
        if byte >= 0x80 {
            if let Ok(s) = std::str::from_utf8(&self.source[offset as usize..]) {
                if let Some(ch) = s.chars().next() {
                    if let Some((suggested, name)) = unicode_confusables::lookup_confusable(ch) {
                        // Span should cover the full multi-byte character
                        // char::len_utf8() is always 1..=4, safe to truncate
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "char::len_utf8() is 1..=4, fits u32"
                        )]
                        let char_len = ch.len_utf8() as u32;
                        let full_span = span(offset, char_len);
                        self.errors
                            .push(LexError::unicode_confusable(full_span, ch, suggested, name));
                        return CookResult::with_error(TokenKind::Error);
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
        CookResult::with_error(TokenKind::Error)
    }

    // Cooking helpers

    #[inline]
    fn cook_ident(&mut self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);

        // Fast path: direct-mapped cache hit bypasses keyword lookup + interner.
        if let Some(kind) = self.ident_cache.get(text) {
            return CookResult::new(kind);
        }

        // Keyword lookup
        if let Some(kw) = keywords::lookup(text) {
            self.ident_cache.insert(text, kw.clone());
            return CookResult::new(kw);
        }

        // Soft keywords are NOT cached — they are context-sensitive
        // (same text can be keyword or identifier depending on lookahead).
        if keywords::could_be_soft_keyword(text) {
            let rest = &self.source[(offset + len) as usize..];
            if let Some(kw) = keywords::soft_keyword_lookup(text, rest) {
                return CookResult::contextual(kw);
            }
        }

        // Reserved-future check (still lex as identifier so parser can continue).
        // Skip the error in method position (after `.`) — the dot provides
        // unambiguous context, e.g. `set.union(other)` is clearly a method call.
        // Uses O(1) `last_non_trivia_raw` instead of backward source scanning.
        let had_error = if keywords::could_be_reserved_future(text) {
            if let Some(keyword) = keywords::reserved_future_lookup(text) {
                let in_method_position = self.last_non_trivia_raw == Some(RawTag::Dot);
                if in_method_position {
                    false
                } else {
                    self.errors.push(LexError::reserved_future_keyword(
                        span(offset, len),
                        keyword,
                    ));
                    true
                }
            } else {
                false
            }
        } else {
            false
        };

        // Intern and cache (skip cache for soft keyword candidates — they are
        // context-sensitive and must be re-evaluated on every occurrence).
        let kind = TokenKind::Ident(self.interner.intern(text));
        if !keywords::could_be_soft_keyword(text) {
            self.ident_cache.insert(text, kind.clone());
        }
        let tag = kind.discriminant_index();
        CookResult {
            kind,
            tag,
            had_error,
            contextual_kw: false,
        }
    }

    // String, char, and template cooking methods are in `escape_cooking.rs`.
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
#[expect(
    unsafe_code,
    reason = "hot path: source is &str, scanner splits on ASCII boundaries"
)]
pub(super) fn slice_source(source: &[u8], offset: u32, len: u32) -> &str {
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
pub(crate) fn span(offset: u32, len: u32) -> ori_ir::Span {
    ori_ir::Span::new(offset, offset + len)
}

#[cfg(test)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "test code: source lengths always fit u32"
)]
mod tests;
