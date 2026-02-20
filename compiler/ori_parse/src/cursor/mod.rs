//! Token cursor for navigating the token stream.
//!
//! Provides low-level token access, lookahead, and consumption methods.

use super::ParseError;
use ori_diagnostic::ErrorCode;
use ori_ir::{Name, Span, StringInterner, Token, TokenCapture, TokenFlags, TokenKind, TokenList};
use tracing::trace;

/// Cursor for navigating tokens.
///
/// Provides methods for accessing, consuming, and checking tokens
/// during parsing. Tracks current position in the token stream.
///
/// Includes a `tags` slice for fast O(1) discriminant checks without
/// touching the full 16-byte `TokenKind`.
pub struct Cursor<'a> {
    tokens: &'a TokenList,
    /// Dense array of discriminant tags, parallel to `tokens`.
    tags: &'a [u8],
    /// Dense array of per-token metadata flags, parallel to `tokens`.
    flags: &'a [TokenFlags],
    interner: &'a StringInterner,
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Create a new cursor at the start of the token stream.
    pub fn new(tokens: &'a TokenList, interner: &'a StringInterner) -> Self {
        Cursor {
            tokens,
            tags: tokens.tags(),
            flags: tokens.flags(),
            interner,
            pos: 0,
        }
    }

    /// Get the total number of tokens in the stream.
    #[inline]
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    /// Get a reference to the string interner.
    pub fn interner(&self) -> &'a StringInterner {
        self.interner
    }

    /// Get the current position in the token stream.
    ///
    /// Used for progress tracking - compare positions before and after
    /// parsing to determine if tokens were consumed.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Set the cursor position directly.
    ///
    /// Used by `ParserSnapshot::restore()` to roll back the parser state
    /// after speculative parsing. The position must be valid (within bounds
    /// of the token stream).
    ///
    /// # Panics
    ///
    /// Panics if `pos` is greater than the token count.
    pub fn set_position(&mut self, pos: usize) {
        debug_assert!(
            pos <= self.tokens.len(),
            "cursor position {} out of bounds (max {})",
            pos,
            self.tokens.len()
        );
        self.pos = pos;
    }

    /// Get the current token.
    ///
    /// Invariant: cursor position is always valid (`0..tokens.len()`).
    /// The last token is always EOF.
    #[inline]
    pub fn current(&self) -> &Token {
        debug_assert!(
            self.pos < self.tokens.len(),
            "cursor position out of bounds"
        );
        // Direct index - bounds check optimized out in release due to invariant
        &self.tokens[self.pos]
    }

    /// Get the current token's kind.
    #[inline]
    pub fn current_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    /// Get the current token's span.
    #[inline]
    pub fn current_span(&self) -> Span {
        self.current().span
    }

    /// Get the previous token's span.
    #[inline]
    pub fn previous_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            Span::DUMMY
        }
    }

    /// Get the previous token's kind.
    ///
    /// Returns `TokenKind::Eof` if at the beginning of the stream (no previous token).
    /// Used to check whether a parsed expression ended with `}` (block body detection).
    #[inline]
    pub fn previous_kind(&self) -> &TokenKind {
        static EOF: TokenKind = TokenKind::Eof;
        if self.pos > 0 {
            &self.tokens[self.pos - 1].kind
        } else {
            &EOF
        }
    }

    /// Get the discriminant tag of the current token.
    ///
    /// Reads from the dense `u8` tag array — a single byte load
    /// instead of accessing the full 16-byte `TokenKind`.
    #[inline]
    pub fn current_tag(&self) -> u8 {
        // Safety: tags.len() == tokens.len(), and pos is always valid.
        self.tags[self.pos]
    }

    /// Check if the current token's tag matches a specific tag value.
    #[inline]
    pub fn check_tag(&self, tag: u8) -> bool {
        self.current_tag() == tag
    }

    /// Check if at end of token stream.
    #[inline]
    pub fn is_at_end(&self) -> bool {
        self.current_tag() == TokenKind::TAG_EOF
    }

    /// Check if the current token matches the given kind.
    #[inline]
    pub fn check(&self, kind: &TokenKind) -> bool {
        self.current_tag() == kind.discriminant_index()
    }

    /// Check if the current token is an identifier.
    #[inline]
    pub fn check_ident(&self) -> bool {
        self.current_tag() == TokenKind::TAG_IDENT
    }

    /// Check if the current token is a type keyword.
    #[inline]
    pub fn check_type_keyword(&self) -> bool {
        let tag = self.current_tag();
        (TokenKind::TAG_INT_TYPE..=TokenKind::TAG_NEVER_TYPE).contains(&tag)
            || tag == TokenKind::TAG_VOID
    }

    /// Peek at the next token's kind (one-token lookahead).
    /// Returns `TokenKind::Eof` if at the end of the stream.
    #[inline]
    pub fn peek_next_kind(&self) -> &TokenKind {
        self.peek_kind_at(1)
    }

    /// Peek at the token kind at offset `n` from current position.
    ///
    /// `peek_kind_at(0)` is the current token, `peek_kind_at(1)` is the next, etc.
    /// Returns `TokenKind::Eof` if past the end of the stream.
    #[inline]
    pub fn peek_kind_at(&self, n: usize) -> &TokenKind {
        static EOF: TokenKind = TokenKind::Eof;
        if self.pos + n < self.tokens.len() {
            &self.tokens[self.pos + n].kind
        } else {
            &EOF
        }
    }

    /// Peek at the next token (one-token lookahead).
    /// Returns the EOF token if at the end of the stream.
    pub fn peek_next_token(&self) -> &Token {
        self.tokens
            .get(self.pos + 1)
            .unwrap_or(&self.tokens[self.tokens.len() - 1])
    }

    /// Get the next token's span.
    pub fn peek_next_span(&self) -> Span {
        self.peek_next_token().span
    }

    /// Check if the next token is adjacent to the current one (no whitespace).
    ///
    /// Uses the pre-computed `TokenFlags::ADJACENT` flag from the lexer,
    /// which is more efficient than comparing span endpoints.
    #[inline]
    pub fn next_is_adjacent(&self) -> bool {
        self.pos + 1 < self.flags.len() && self.flags[self.pos + 1].is_adjacent()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // TokenFlags Access
    // ─────────────────────────────────────────────────────────────────────────

    /// Get the flags for the current token.
    #[inline]
    pub fn current_flags(&self) -> TokenFlags {
        self.flags[self.pos]
    }

    /// True if the current token was preceded by a newline.
    ///
    /// Used for implicit line continuation detection and
    /// newline-significant grammar rules.
    #[inline]
    pub fn has_newline_before(&self) -> bool {
        self.flags[self.pos].has_newline_before()
    }

    /// True if the current token is the first non-trivia token on its line.
    ///
    /// Used for layout-sensitive constructs where indentation matters.
    #[inline]
    pub fn at_line_start(&self) -> bool {
        self.flags[self.pos].is_line_start()
    }

    /// True if a doc comment preceded the current token (`IS_DOC` flag).
    ///
    /// Doc comments use markers `#` (description), `*` (member), `!` (warning),
    /// `>` (example). This flag is set on the first non-trivia token after
    /// the doc comment, typically a declaration keyword like `def` or `type`.
    ///
    /// Only available when tokens are produced by [`lex_with_comments()`] —
    /// the fast [`lex()`] path does not classify comments.
    #[inline]
    pub fn has_doc_before(&self) -> bool {
        self.flags[self.pos].is_doc()
    }

    /// True if the current token is adjacent to the previous token
    /// (no whitespace, newline, or trivia between them).
    ///
    /// Useful for distinguishing `foo(` (call) from `foo (` (grouping).
    #[inline]
    pub fn is_adjacent(&self) -> bool {
        self.flags[self.pos].is_adjacent()
    }

    /// True if the current token was resolved as a context-sensitive keyword
    /// (soft keyword with `(` lookahead).
    #[inline]
    pub fn is_contextual_kw(&self) -> bool {
        self.flags[self.pos].is_contextual_kw()
    }

    /// Check if looking at `>` followed immediately by `>` (no whitespace).
    /// Used for detecting `>>` shift operator in expression context.
    pub fn is_shift_right(&self) -> bool {
        self.current_tag() == TokenKind::TAG_GT
            && self.pos + 1 < self.tags.len()
            && self.tags[self.pos + 1] == TokenKind::TAG_GT
            && self.next_is_adjacent()
    }

    /// Check if looking at `>` followed immediately by `=` (no whitespace).
    /// Used for detecting `>=` comparison operator in expression context.
    pub fn is_greater_equal(&self) -> bool {
        self.current_tag() == TokenKind::TAG_GT
            && self.pos + 1 < self.tags.len()
            && self.tags[self.pos + 1] == TokenKind::TAG_EQ
            && self.next_is_adjacent()
    }

    /// Consume two adjacent tokens as a compound operator.
    /// Returns the combined span.
    /// Panics if not at the expected tokens.
    pub fn consume_compound(&mut self) -> Span {
        let start = self.current_span();
        self.advance();
        let end = self.current_span();
        self.advance();
        start.merge(end)
    }

    /// Check if the next token (lookahead) is a left paren.
    #[inline]
    pub fn next_is_lparen(&self) -> bool {
        self.pos + 1 < self.tags.len() && self.tags[self.pos + 1] == TokenKind::TAG_LPAREN
    }

    /// Check if the next token (lookahead) is a colon.
    #[inline]
    pub fn next_is_colon(&self) -> bool {
        self.pos + 1 < self.tags.len() && self.tags[self.pos + 1] == TokenKind::TAG_COLON
    }

    /// Check if this is capability provision syntax: `with Ident =`
    /// Current position should be at `with`.
    pub fn is_with_capability_syntax(&self) -> bool {
        // Need at least 3 tokens ahead: with Ident =
        if self.pos + 2 >= self.tokens.len() {
            return false;
        }
        // Token at pos+1 should be an identifier
        let next_is_ident = matches!(self.tokens[self.pos + 1].kind, TokenKind::Ident(_));
        // Token at pos+2 should be =
        let then_is_eq = matches!(self.tokens[self.pos + 2].kind, TokenKind::Eq);

        next_is_ident && then_is_eq
    }

    /// Check if looking at named argument pattern: identifier followed by colon.
    /// Used to distinguish `name: value` (named arg) from `value` (positional).
    pub fn is_named_arg_start(&self) -> bool {
        let is_ident = matches!(self.current_kind(), TokenKind::Ident(_))
            || self.soft_keyword_to_name().is_some()
            || self.keyword_as_name().is_some();
        is_ident && self.next_is_colon()
    }

    /// Check if current token is a context-sensitive keyword that can be used as an identifier.
    /// These are only treated as keywords in specific contexts (e.g., when followed by `(`).
    /// Per spec, context-sensitive keywords: by cache catch for max parallel recurse run spawn timeout try with without
    /// Returns the interned name if it's a soft keyword, None otherwise.
    ///
    /// Note: `cache`, `catch`, `parallel`, `recurse`, `spawn`, `timeout` are handled
    /// at the lexer level via `(` lookahead — they appear as `Ident` tokens when not
    /// in keyword position, so they don't need conversion here.
    pub fn soft_keyword_to_name(&self) -> Option<&'static str> {
        match self.current_kind() {
            // I/O primitives
            TokenKind::Print => Some("print"),
            TokenKind::Panic => Some("panic"),
            // Context-sensitive pattern keywords (still always-resolved)
            TokenKind::By => Some("by"),
            TokenKind::Run => Some("run"),
            TokenKind::Try => Some("try"),
            TokenKind::With => Some("with"),
            _ => None,
        }
    }

    /// Advance to the next token and return the consumed token.
    ///
    /// # Safety invariant
    ///
    /// The lexer always appends an EOF token, and grammar rules always check
    /// the current token kind before calling `advance()`. This means the parser
    /// can never advance past the last token. The unconditional increment avoids
    /// a branch on every token consumption.
    #[inline]
    pub fn advance(&mut self) -> &Token {
        let current = self.pos;
        debug_assert!(
            self.pos < self.tokens.len(),
            "advance past end of token stream"
        );
        let token = &self.tokens[current];
        trace!(
            pos = current,
            kind = %token.kind.display_name(),
            span_start = token.span.start,
            span_end = token.span.end,
            "advance"
        );
        self.pos += 1;
        token
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Token Capture
    // ─────────────────────────────────────────────────────────────────────────

    /// Mark the current position for starting a token capture.
    ///
    /// Use with `complete_capture()` to capture a range of tokens:
    /// ```ignore
    /// let start = cursor.start_capture();
    /// // ... parse some tokens ...
    /// let capture = cursor.complete_capture(start);
    /// ```
    #[inline]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Token count cannot exceed u32::MAX (4 billion tokens would require ~100GB of source)"
    )]
    pub fn start_capture(&self) -> u32 {
        self.pos as u32
    }

    /// Complete a token capture from a start position.
    ///
    /// Returns `TokenCapture::None` if no tokens were consumed.
    /// Returns `TokenCapture::Range { start, end }` otherwise.
    #[inline]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Token count cannot exceed u32::MAX (4 billion tokens would require ~100GB of source)"
    )]
    pub fn complete_capture(&self, start: u32) -> TokenCapture {
        TokenCapture::new(start, self.pos as u32)
    }

    /// Get the token list reference for accessing captured ranges.
    #[inline]
    pub fn tokens(&self) -> &'a TokenList {
        self.tokens
    }

    /// Skip all newline tokens.
    ///
    /// Uses tag-based check for maximum speed on this hot path.
    #[inline]
    pub fn skip_newlines(&mut self) {
        while self.current_tag() == TokenKind::TAG_NEWLINE {
            self.advance();
        }
    }

    /// Expect the current token to be of the given kind, advance and return it.
    /// Returns an error if the token kind doesn't match.
    ///
    /// Split into inline happy path + `#[cold]` error path so that
    /// `format!()` allocations don't prevent LLVM from inlining the fast case.
    #[inline]
    pub fn expect(&mut self, kind: &TokenKind) -> Result<&Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.make_expect_error(kind))
        }
    }

    /// Build the error for a failed `expect()` call.
    ///
    /// Separated as `#[cold]` so the `format!()` allocation doesn't
    /// prevent LLVM from inlining the hot `expect()` fast path.
    #[cold]
    #[inline(never)]
    fn make_expect_error(&self, kind: &TokenKind) -> ParseError {
        ParseError::new(
            ErrorCode::E1001,
            format!(
                "expected {}, found {}",
                kind.display_name(),
                self.current_kind().display_name()
            ),
            self.current_span(),
        )
        .with_context(format!("expected {}", kind.display_name()))
    }

    /// Expect and consume an identifier, returning its interned name.
    /// Also accepts soft keywords (len, min, max, etc.) as identifiers.
    ///
    /// Split into inline happy path + `#[cold]` error path for inlining.
    #[inline]
    pub fn expect_ident(&mut self) -> Result<Name, ParseError> {
        // Accept regular identifiers
        if let TokenKind::Ident(name) = *self.current_kind() {
            self.advance();
            Ok(name)
        // Also accept soft keywords as identifiers
        } else if let Some(name_str) = self.soft_keyword_to_name() {
            let name = self.interner.intern(name_str);
            self.advance();
            Ok(name)
        } else {
            Err(self.make_expect_ident_error())
        }
    }

    /// Expect and consume a member name (after `.`), returning its interned name.
    ///
    /// Accepts identifiers, soft keywords, reserved keywords, and integer
    /// literals (for tuple field access: `t.0`, `t.1`). Keywords and integers
    /// are valid in member position because the `.` prefix provides unambiguous
    /// context (e.g., `ordering.then(other: Less)`, `pair.0`).
    ///
    /// See grammar.ebnf § `member_name`.
    #[inline]
    pub fn expect_member_name(&mut self) -> Result<Name, ParseError> {
        // Accept regular identifiers
        if let TokenKind::Ident(name) = *self.current_kind() {
            self.advance();
            Ok(name)
        // Accept soft keywords
        } else if let Some(name_str) = self.soft_keyword_to_name() {
            let name = self.interner.intern(name_str);
            self.advance();
            Ok(name)
        // Accept any keyword (then, if, for, type, etc.)
        } else if let Some(kw_str) = self.current_kind().keyword_str() {
            let name = self.interner.intern(kw_str);
            self.advance();
            Ok(name)
        // Accept integer literals for tuple field access: t.0, t.1
        } else if let TokenKind::Int(value) = *self.current_kind() {
            let name = self.interner.intern(&value.to_string());
            self.advance();
            Ok(name)
        } else {
            Err(self.make_expect_ident_error())
        }
    }

    /// Build the error for a failed `expect_ident()` call.
    #[cold]
    #[inline(never)]
    fn make_expect_ident_error(&self) -> ParseError {
        ParseError::new(
            ErrorCode::E1004,
            format!(
                "expected identifier, found {}",
                self.current_kind().display_name()
            ),
            self.current_span(),
        )
    }

    /// Accept an identifier or a keyword that can be used as a named argument name.
    /// This handles cases like `where:` in the find pattern where `where` is a keyword.
    pub fn expect_ident_or_keyword(&mut self) -> Result<Name, ParseError> {
        if let TokenKind::Ident(name) = *self.current_kind() {
            self.advance();
            Ok(name)
        } else if let Some(name_str) = self.soft_keyword_to_name() {
            let name = self.interner.intern(name_str);
            self.advance();
            Ok(name)
        } else if let Some(name_str) = self.keyword_as_name() {
            let name = self.interner.intern(name_str);
            self.advance();
            Ok(name)
        } else {
            Err(self.make_expect_ident_or_keyword_error())
        }
    }

    /// Map keywords usable as named argument names to their string form.
    ///
    /// These keywords can appear as field names, named arguments, etc.
    /// Returns `None` for non-keyword tokens or keywords that cannot be
    /// used as names.
    fn keyword_as_name(&self) -> Option<&'static str> {
        match self.current_kind() {
            TokenKind::Where => Some("where"),
            TokenKind::Match => Some("match"),
            TokenKind::For => Some("for"),
            TokenKind::In => Some("in"),
            TokenKind::If => Some("if"),
            TokenKind::Type => Some("type"),
            _ => None,
        }
    }

    /// Build the error for a failed `expect_ident_or_keyword()` call.
    #[cold]
    #[inline(never)]
    fn make_expect_ident_or_keyword_error(&self) -> ParseError {
        ParseError::new(
            ErrorCode::E1004,
            format!(
                "expected identifier or keyword, found {}",
                self.current_kind().display_name()
            ),
            self.current_span(),
        )
    }
}

#[cfg(test)]
mod tests;
