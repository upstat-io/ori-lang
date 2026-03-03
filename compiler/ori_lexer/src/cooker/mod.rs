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

use crate::cook_escape::{unescape_char_v2, unescape_string_v2, unescape_template_v2};
use crate::keywords;
use crate::lex_error::{LexError, LexSuggestion};
use crate::unicode_confusables;
use crate::what_is_next::{self, NextContext};

/// Number of slots in the direct-mapped identifier cache.
const IDENT_CACHE_SIZE: usize = 256;
const IDENT_CACHE_MASK: usize = IDENT_CACHE_SIZE - 1;

/// Direct-mapped identifier cache: fixed 256-entry array indexed by hash.
///
/// On hit (text matches), returns the cached `TokenKind` — bypassing both
/// keyword lookup and the string interner. On miss or collision, the slot
/// is overwritten with the new entry (no probing, no chaining).
///
/// This is ~10x cheaper than `FxHashMap` per lookup because it avoids
/// `HashMap` bookkeeping, probing, and dynamic resizing. The tradeoff is
/// that collisions silently evict entries, but the hot identifiers (`int`,
/// `let`, `x`, etc.) that appear thousands of times naturally dominate
/// their slots.
struct IdentCache<'src> {
    slots: [Option<(&'src str, TokenKind)>; IDENT_CACHE_SIZE],
}

impl<'src> IdentCache<'src> {
    fn new() -> Self {
        Self {
            slots: [(); IDENT_CACHE_SIZE].map(|()| None),
        }
    }

    /// Look up a cached identifier result.
    #[expect(
        clippy::inline_always,
        reason = "hot inner loop: 3-instruction cache probe"
    )]
    #[inline(always)]
    fn get(&self, text: &str) -> Option<&TokenKind> {
        let slot = Self::hash(text);
        if let Some((cached, kind)) = &self.slots[slot] {
            if *cached == text {
                return Some(kind);
            }
        }
        None
    }

    /// Insert or overwrite a cache entry.
    #[expect(
        clippy::inline_always,
        reason = "hot inner loop: 2-instruction cache store"
    )]
    #[inline(always)]
    fn insert(&mut self, text: &'src str, kind: TokenKind) {
        let slot = Self::hash(text);
        self.slots[slot] = Some((text, kind));
    }

    /// Simple hash: first byte * 31 ^ last byte ^ length.
    /// Distributes identifiers well because last byte varies even for
    /// prefixed names (func0, func1, ...) and length separates keywords.
    #[expect(clippy::inline_always, reason = "hot inner loop: 4-instruction hash")]
    #[inline(always)]
    fn hash(text: &str) -> usize {
        let bytes = text.as_bytes();
        debug_assert!(!bytes.is_empty(), "identifiers are never empty");
        let len = bytes.len();
        let first = bytes[0] as usize;
        let last = bytes[len - 1] as usize;
        (first.wrapping_mul(31) ^ last ^ len) & IDENT_CACHE_MASK
    }
}

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
}

impl<'src> TokenCooker<'src> {
    /// Create a new cooker for the given source.
    pub(crate) fn new(source: &'src [u8], interner: &'src StringInterner) -> Self {
        Self {
            source,
            interner,
            errors: Vec::new(),
            ident_cache: IdentCache::new(),
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

    /// Cook a single raw token into a `CookResult`.
    ///
    /// `offset` is the byte position of the token in source.
    /// `len` is the byte length of the token.
    #[inline]
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive RawTag → CookResult cooking dispatch"
    )]
    pub(crate) fn cook(&mut self, tag: RawTag, offset: u32, len: u32) -> CookResult {
        match tag {
            // Direct-map operators
            RawTag::Plus => CookResult::new(TokenKind::Plus),
            RawTag::Minus => CookResult::new(TokenKind::Minus),
            RawTag::Star => CookResult::new(TokenKind::Star),
            RawTag::Slash => CookResult::new(TokenKind::Slash),
            RawTag::Percent => CookResult::new(TokenKind::Percent),
            RawTag::Caret => CookResult::new(TokenKind::Caret),
            RawTag::Ampersand => CookResult::new(TokenKind::Amp),
            RawTag::Pipe => CookResult::new(TokenKind::Pipe),
            RawTag::Tilde => CookResult::new(TokenKind::Tilde),
            RawTag::Bang => CookResult::new(TokenKind::Bang),
            RawTag::Equal => CookResult::new(TokenKind::Eq),
            RawTag::Less => CookResult::new(TokenKind::Lt),
            RawTag::Greater => CookResult::new(TokenKind::Gt),
            RawTag::Dot => CookResult::new(TokenKind::Dot),
            RawTag::Question => CookResult::new(TokenKind::Question),

            // Compound operators
            RawTag::EqualEqual => CookResult::new(TokenKind::EqEq),
            RawTag::BangEqual => CookResult::new(TokenKind::NotEq),
            RawTag::LessEqual => CookResult::new(TokenKind::LtEq),
            RawTag::AmpersandAmpersand => CookResult::new(TokenKind::AmpAmp),
            RawTag::PipePipe => CookResult::new(TokenKind::PipePipe),
            RawTag::Arrow => CookResult::new(TokenKind::Arrow),
            RawTag::FatArrow => CookResult::new(TokenKind::FatArrow),
            RawTag::DotDot => CookResult::new(TokenKind::DotDot),
            RawTag::DotDotEqual => CookResult::new(TokenKind::DotDotEq),
            RawTag::DotDotDot => CookResult::new(TokenKind::DotDotDot),
            RawTag::ColonColon => CookResult::new(TokenKind::DoubleColon),
            RawTag::Shl => CookResult::new(TokenKind::Shl),
            RawTag::QuestionQuestion => CookResult::new(TokenKind::DoubleQuestion),

            // Compound assignment operators
            RawTag::PlusEq => CookResult::new(TokenKind::PlusEq),
            RawTag::MinusEq => CookResult::new(TokenKind::MinusEq),
            RawTag::StarEq => CookResult::new(TokenKind::StarEq),
            RawTag::SlashEq => CookResult::new(TokenKind::SlashEq),
            RawTag::PercentEq => CookResult::new(TokenKind::PercentEq),
            RawTag::AtEq => CookResult::new(TokenKind::AtEq),
            RawTag::AmpersandEq => CookResult::new(TokenKind::AmpEq),
            RawTag::PipeEq => CookResult::new(TokenKind::PipeEq),
            RawTag::CaretEq => CookResult::new(TokenKind::CaretEq),
            RawTag::ShlEq => CookResult::new(TokenKind::ShlEq),
            RawTag::AmpersandAmpersandEq => CookResult::new(TokenKind::AmpAmpEq),
            RawTag::PipePipeEq => CookResult::new(TokenKind::PipePipeEq),

            // Delimiters
            RawTag::LeftParen => CookResult::new(TokenKind::LParen),
            RawTag::RightParen => CookResult::new(TokenKind::RParen),
            RawTag::LeftBracket => CookResult::new(TokenKind::LBracket),
            RawTag::RightBracket => CookResult::new(TokenKind::RBracket),
            RawTag::LeftBrace => CookResult::new(TokenKind::LBrace),
            RawTag::RightBrace => CookResult::new(TokenKind::RBrace),
            RawTag::Comma => CookResult::new(TokenKind::Comma),
            RawTag::Colon => CookResult::new(TokenKind::Colon),
            RawTag::Semicolon => CookResult::new(TokenKind::Semicolon),
            RawTag::At => CookResult::new(TokenKind::At),
            RawTag::Hash => CookResult::new(TokenKind::Hash),
            RawTag::Underscore => CookResult::new(TokenKind::Underscore),
            RawTag::Dollar => CookResult::new(TokenKind::Dollar),
            RawTag::HashBracket => CookResult::new(TokenKind::HashBracket),
            RawTag::HashBang => CookResult::new(TokenKind::HashBang),

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

            // Future variants (non_exhaustive)
            _ => CookResult::new(TokenKind::Error),
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
                        #[allow(
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
            return CookResult::new(kind.clone());
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
        // This mirrors how soft keywords use lookahead for context sensitivity.
        let had_error = if keywords::could_be_reserved_future(text) {
            if let Some(keyword) = keywords::reserved_future_lookup(text) {
                let preceding: &[u8] = &self.source[..offset as usize];
                let in_method_position = preceding
                    .iter()
                    .rposition(|b: &u8| !b.is_ascii_whitespace())
                    .is_some_and(|i| preceding[i] == b'.');
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

        // Intern and cache
        let kind = TokenKind::Ident(self.interner.intern(text));
        self.ident_cache.insert(text, kind.clone());
        let tag = kind.discriminant_index();
        CookResult {
            kind,
            tag,
            had_error,
            contextual_kw: false,
        }
    }

    fn cook_string(&mut self, offset: u32, len: u32) -> CookResult {
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

    fn cook_char(&mut self, offset: u32, len: u32) -> CookResult {
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

    fn cook_template_head(&mut self, offset: u32, len: u32) -> CookResult {
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

    fn cook_template_middle(&mut self, offset: u32, len: u32) -> CookResult {
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

    fn cook_template_tail(&mut self, offset: u32, len: u32) -> CookResult {
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

    fn cook_format_spec(&self, offset: u32, len: u32) -> CookResult {
        let text = slice_source(self.source, offset, len);
        // The format spec token includes the leading `:` from the scanner.
        // Strip it to get just the spec content.
        let content = &text[1..];
        CookResult::new(TokenKind::FormatSpec(self.interner.intern(content)))
    }

    fn cook_template_complete(&mut self, offset: u32, len: u32) -> CookResult {
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

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    reason = "test code: source lengths always fit u32"
)]
mod tests;
