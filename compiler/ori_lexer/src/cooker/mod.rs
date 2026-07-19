//! Conversion from raw scanner spans to parser tokens.
//!
//! Operators map directly; identifiers resolve keywords before interning;
//! numeric, duration, and size tokens parse values with overflow checks; text
//! tokens decode escapes before interning. Invalid raw tokens record a lexical
//! error and produce `TokenKind::Error` for recovery.

mod duration_size;
mod escape_cooking;
mod identifier;
mod numeric;
mod source_slice;

use ori_ir::{StringInterner, TokenKind};

// Re-exported for tests (DurationUnit/SizeUnit needed by test assertions)
#[cfg(test)]
pub(crate) use ori_ir::{DurationUnit, SizeUnit};
use ori_lexer_core::RawTag;

#[cfg(test)]
pub(crate) use duration_size::{
    detect_duration_suffix, detect_size_suffix, parse_decimal_unit_value, DetectedUnit,
};

use crate::keywords;
use crate::lex_error::{LexError, LexSuggestion};
use crate::unicode_confusables;
use crate::what_is_next::{self, NextContext};

use identifier::IdentCache;
use source_slice::slice_source;

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
    status: CookStatus,
}

/// Outcome metadata for one cooked token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CookStatus {
    Plain,
    Error,
    ContextualKeyword,
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
            status: CookStatus::Plain,
        }
    }

    /// Trivial token whose discriminant tag was computed by the raw fast path.
    #[inline]
    pub(crate) fn trivial(kind: TokenKind, tag: u8) -> Self {
        Self {
            kind,
            tag,
            status: CookStatus::Plain,
        }
    }

    /// Token that pushed an error during cooking.
    #[inline]
    fn with_error(kind: TokenKind) -> Self {
        let tag = kind.discriminant_index();
        Self {
            kind,
            tag,
            status: CookStatus::Error,
        }
    }

    /// Context-sensitive keyword (soft keyword with valid lookahead).
    #[inline]
    fn contextual(kind: TokenKind) -> Self {
        let tag = kind.discriminant_index();
        Self {
            kind,
            tag,
            status: CookStatus::ContextualKeyword,
        }
    }

    /// Whether cooking emitted a lexer error.
    #[inline]
    pub(crate) fn had_error(&self) -> bool {
        matches!(self.status, CookStatus::Error)
    }

    /// Whether the token was resolved as a contextual keyword.
    #[inline]
    pub(crate) fn is_contextual_keyword(&self) -> bool {
        matches!(self.status, CookStatus::ContextualKeyword)
    }
}

/// Cooks raw tokens into parser-ready `TokenKind` values.
///
/// Stateless with respect to individual tokens — each `cook()` call is
/// independent. Accumulates errors for the entire file.
pub(crate) struct TokenCooker<'src> {
    source: &'src str,
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

// The shared interner and identifier cache are intentionally opaque; neither
// implements `Debug`, while the source and accumulated diagnostics identify
// the cooker's observable state.
impl std::fmt::Debug for TokenCooker<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenCooker")
            .field("source_len", &self.source.len())
            .field("errors", &self.errors)
            .field("last_non_trivia_raw", &self.last_non_trivia_raw)
            .finish_non_exhaustive()
    }
}

impl<'src> TokenCooker<'src> {
    /// Create a new cooker for the given source.
    pub(crate) fn new(source: &'src str, interner: &'src StringInterner) -> Self {
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
    /// `Semicolon` is the exception and always reaches `cook`.
    #[inline]
    pub(crate) fn cook(&mut self, tag: RawTag, offset: u32, len: u32) -> CookResult {
        // Why: Unit tests pass every `RawTag`, including tags the driver normally intercepts.
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
            RawTag::UnterminatedChar => self.cook_unterminated_char(offset, len),
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
            _ => Self::cook_trivial_fallback(tag),
        }
    }

    // Error cooking helpers

    fn cook_unterminated_char(&mut self, offset: u32, len: u32) -> CookResult {
        let err_span = span(offset, len);
        let text = slice_source(self.source, offset, len);
        if looks_like_single_quote_string(text) {
            self.errors.push(LexError::single_quote_string(err_span));
        } else {
            self.errors.push(LexError::unterminated_char(err_span));
        }
        CookResult::with_error(TokenKind::Error)
    }

    fn cook_trivial_fallback(tag: RawTag) -> CookResult {
        let Some((kind, tag_byte)) = crate::trivial::try_trivial(tag) else {
            panic!(
                "raw token {tag:?} has no cooker route; add it to TokenCooker::cook or \
                 trivial::try_trivial"
            );
        };
        CookResult {
            kind,
            tag: tag_byte,
            status: CookStatus::Plain,
        }
    }

    /// Cook an invalid byte into a context-aware diagnostic, detecting Unicode
    /// confusables and cross-language patterns.
    #[cold]
    fn cook_invalid_byte(&mut self, offset: u32, len: u32) -> CookResult {
        let byte = self.source.as_bytes()[offset as usize];
        let err_span = span(offset, len);

        // Try to decode as UTF-8 for Unicode confusable detection
        if byte >= 0x80 {
            if let Ok(s) = std::str::from_utf8(&self.source.as_bytes()[offset as usize..]) {
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
        let ctx = what_is_next::what_is_next(self.source.as_bytes(), offset);
        let mut err = LexError::invalid_byte(err_span, byte);
        match ctx {
            NextContext::UnsupportedOperator(operator) => {
                self.errors
                    .push(LexError::unsupported_operator(err_span, operator));
                return CookResult::with_error(TokenKind::Error);
            }
            NextContext::Unicode(ch) => {
                err = err.with_suggestion(LexSuggestion::text(
                    format!("unexpected Unicode character `{ch}`"),
                    0,
                ));
            }
            _ => {}
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
            let rest = &self.source.as_bytes()[(offset + len) as usize..];
            if let Some(kw) = keywords::soft_keyword_lookup(text, rest) {
                return CookResult::contextual(kw);
            }
        }

        // Reserved-future words remain identifiers for recovery; method
        // position is unambiguous and exempt. Cached raw context avoids a scan.
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
            status: if had_error {
                CookStatus::Error
            } else {
                CookStatus::Plain
            },
        }
    }

    // String, char, and template cooking methods are in `escape_cooking.rs`.
}

fn looks_like_single_quote_string(text: &str) -> bool {
    if text.len() < 4 || !text.starts_with('\'') || !text.ends_with('\'') {
        return false;
    }

    let inner = &text[1..text.len() - 1];
    !inner.contains('\\') && inner.chars().count() > 1
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
