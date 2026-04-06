//! Identifier and keyword acceptance for the parser cursor.
//!
//! Handles the three public parser APIs that consume identifier-like tokens:
//! - [`Cursor::expect_ident`] — identifiers + soft keywords
//! - [`Cursor::expect_member_name`] — identifiers + all keywords + integers
//! - [`Cursor::expect_ident_or_keyword`] — identifiers + soft + positional keywords
//!
//! The canonical classification of which keywords are usable as identifiers
//! lives in [`soft_keyword_str`] and [`positional_keyword_str`].

use ori_diagnostic::ErrorCode;
use ori_ir::{Name, TokenKind};

use super::{Cursor, ParseError};

// Identifier acceptance methods

impl Cursor<'_> {
    /// Check if current token is a context-sensitive keyword that can be used as an identifier.
    /// These are only treated as keywords in specific contexts (e.g., when followed by `(`).
    /// Per spec, context-sensitive keywords: by cache catch for max parallel recurse run spawn timeout try with without
    /// Returns the string form if the current token is a soft keyword, None otherwise.
    ///
    /// Delegates to [`soft_keyword_str`] — the canonical source for soft-keyword
    /// classification.
    pub fn soft_keyword_to_name(&self) -> Option<&'static str> {
        soft_keyword_str(self.current_kind())
    }

    /// Try to consume an identifier or soft keyword, returning the interned name.
    ///
    /// This is the shared prefix of [`expect_ident`], [`expect_member_name`], and
    /// [`expect_ident_or_keyword`]. Returns `None` without advancing if the current
    /// token is neither an identifier nor a soft keyword.
    #[inline]
    fn take_ident_or_soft_keyword(&mut self) -> Option<Name> {
        if let TokenKind::Ident(name) = *self.current_kind() {
            self.advance();
            Some(name)
        } else if let Some(name_str) = self.soft_keyword_to_name() {
            let name = self.interner().intern(name_str);
            self.advance();
            Some(name)
        } else {
            None
        }
    }

    /// Expect and consume an identifier, returning its interned name.
    /// Also accepts soft keywords (print, panic, by, etc.) as identifiers.
    ///
    /// Split into inline happy path + `#[cold]` error path for inlining.
    #[inline]
    pub fn expect_ident(&mut self) -> Result<Name, ParseError> {
        self.take_ident_or_soft_keyword()
            .ok_or_else(|| self.make_expect_ident_error())
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
        if let Some(name) = self.take_ident_or_soft_keyword() {
            return Ok(name);
        }
        // Accept any keyword (then, if, for, type, etc.)
        if let Some(kw_str) = self.current_kind().keyword_str() {
            let name = self.interner().intern(kw_str);
            self.advance();
            return Ok(name);
        }
        // Accept integer literals for tuple field access: t.0, t.1
        if let TokenKind::Int(value) = *self.current_kind() {
            let name = self.interner().intern(&value.to_string());
            self.advance();
            return Ok(name);
        }
        Err(self.make_expect_member_name_error())
    }

    /// Accept an identifier or a keyword that can be used as a named argument name.
    /// This handles cases like `where:` in the find pattern where `where` is a keyword.
    pub fn expect_ident_or_keyword(&mut self) -> Result<Name, ParseError> {
        if let Some(name) = self.take_ident_or_soft_keyword() {
            return Ok(name);
        }
        if let Some(name_str) = self.keyword_as_name() {
            let name = self.interner().intern(name_str);
            self.advance();
            return Ok(name);
        }
        Err(self.make_expect_ident_or_keyword_error())
    }

    /// Map keywords usable as named argument names to their string form.
    ///
    /// Delegates to [`positional_keyword_str`] — the canonical source for
    /// positional-keyword classification.
    pub(super) fn keyword_as_name(&self) -> Option<&'static str> {
        positional_keyword_str(self.current_kind())
    }

    // Error factories

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

    /// Build the error for a failed `expect_member_name()` call.
    ///
    /// Member names accept identifiers, keywords, and integers (for tuple
    /// field access), so the error message is broader than `expect_ident`.
    #[cold]
    #[inline(never)]
    fn make_expect_member_name_error(&self) -> ParseError {
        ParseError::new(
            ErrorCode::E1004,
            format!(
                "expected member name, found {}",
                self.current_kind().display_name()
            ),
            self.current_span(),
        )
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

// Keyword classification functions

/// Map a soft keyword token to its string form.
///
/// Soft keywords are context-sensitive: they act as keywords in certain syntactic
/// positions but are valid identifiers elsewhere.
///
/// This is the **canonical source** for soft-keyword classification — both
/// [`Cursor::soft_keyword_to_name`] and [`is_keyword_usable_as_ident`] delegate here.
pub(super) fn soft_keyword_str(kind: &TokenKind) -> Option<&'static str> {
    match kind {
        // I/O primitives
        TokenKind::Print => Some("print"),
        TokenKind::Panic => Some("panic"),
        // Context-sensitive pattern keywords (still always-resolved)
        TokenKind::By => Some("by"),
        TokenKind::Run => Some("run"),
        TokenKind::Try => Some("try"),
        TokenKind::With => Some("with"),
        // Lexer-soft keywords (resolved only with `(` lookahead)
        TokenKind::Cache => Some("cache"),
        TokenKind::Catch => Some("catch"),
        TokenKind::Parallel => Some("parallel"),
        TokenKind::Recurse => Some("recurse"),
        TokenKind::Spawn => Some("spawn"),
        TokenKind::Timeout => Some("timeout"),
        _ => None,
    }
}

/// Map a positional keyword to its string form.
///
/// Positional keywords can appear as field names, named arguments, etc.
/// They are a strict subset of reserved keywords that are unambiguous in
/// identifier position.
///
/// This is the **canonical source** for positional-keyword classification — both
/// [`Cursor::keyword_as_name`] and [`is_keyword_usable_as_ident`] delegate here.
pub(super) fn positional_keyword_str(kind: &TokenKind) -> Option<&'static str> {
    match kind {
        TokenKind::Where => Some("where"),
        TokenKind::Match => Some("match"),
        TokenKind::For => Some("for"),
        TokenKind::In => Some("in"),
        TokenKind::If => Some("if"),
        TokenKind::Type => Some("type"),
        _ => None,
    }
}

/// Check if a keyword token can be used as an identifier (named arg, field name, etc.).
///
/// Derived from [`soft_keyword_str`] and [`positional_keyword_str`] — no independent
/// match list. Adding a new keyword-as-identifier only requires updating the
/// corresponding canonical function above.
///
/// Used by [`Cursor::is_named_arg_at`] for lookahead.
/// A test (`keyword_as_ident_consistency`) enforces this stays in sync.
pub(super) fn is_keyword_usable_as_ident(kind: &TokenKind) -> bool {
    soft_keyword_str(kind).is_some() || positional_keyword_str(kind).is_some()
}
