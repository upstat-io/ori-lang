//! Token capture, speculative parsing, and outcome handling for the Parser.
//!
//! Includes lazy token capture for formatters, snapshot-based speculative
//! parsing for disambiguation, multi-token matching utilities, and the
//! `handle_outcome` helper for collecting parsed items with error recovery.

use crate::outcome::ParseOutcome;
use crate::recovery::TokenSet;
use crate::{ParseError, Parser};

use ori_ir::TokenKind;

impl Parser<'_> {
    // --- Token Capture ---
    //
    // These methods support lazy token capture for formatters and future macros.
    // Instead of storing tokens directly, we capture index ranges into the
    // cached TokenList, which is very memory efficient.

    /// Execute a parser and capture its tokens.
    ///
    /// This is a convenience method that combines `start_capture()` and
    /// `complete_capture()` with a parsing closure. Use when you always
    /// need to capture tokens.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (expr, capture) = parser.with_capture(|p| p.parse_expr())?;
    /// ```
    #[inline]
    #[allow(
        dead_code,
        reason = "infrastructure for formatters and macro expansion"
    )]
    pub(crate) fn with_capture<T, F>(&mut self, f: F) -> (T, ori_ir::TokenCapture)
    where
        F: FnOnce(&mut Self) -> T,
    {
        let start = self.cursor.start_capture();
        let result = f(self);
        let capture = self.cursor.complete_capture(start);
        (result, capture)
    }

    /// Execute a parser and optionally capture its tokens.
    ///
    /// When `needs_capture` is false, returns `TokenCapture::None` without
    /// the overhead of tracking positions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let needs_tokens = self.context.has(ParseContext::CAPTURE_TOKENS);
    /// let (expr, capture) = parser.capture_if(needs_tokens, |p| p.parse_expr())?;
    /// ```
    #[inline]
    #[allow(
        dead_code,
        reason = "infrastructure for conditional token capture in formatters"
    )]
    pub(crate) fn capture_if<T, F>(
        &mut self,
        needs_capture: bool,
        f: F,
    ) -> (T, ori_ir::TokenCapture)
    where
        F: FnOnce(&mut Self) -> T,
    {
        if needs_capture {
            self.with_capture(f)
        } else {
            (f(self), ori_ir::TokenCapture::None)
        }
    }

    /// Check if the current token matches any kind in the set.
    ///
    /// Unlike `cursor.check()`, this tests against multiple token kinds at once.
    /// Returns `true` if any match is found.
    #[inline]
    #[allow(dead_code, reason = "infrastructure for multi-token error recovery")]
    pub(crate) fn check_one_of(&self, expected: &TokenSet) -> bool {
        expected.contains(self.cursor.current_kind())
    }

    /// Expect one of several token kinds, generating a helpful error if none match.
    ///
    /// Uses `TokenSet::format_expected()` to generate messages like
    /// "expected `,`, `)`, or `}`, found `+`".
    ///
    /// Returns the matched token kind on success.
    #[cold]
    #[allow(
        dead_code,
        reason = "infrastructure for multi-token expect with rich errors"
    )]
    pub(crate) fn expect_one_of(&mut self, expected: &TokenSet) -> Result<TokenKind, ParseError> {
        let current = self.cursor.current_kind();
        if expected.contains(current) {
            let matched = current.clone();
            self.cursor.advance();
            Ok(matched)
        } else {
            Err(ParseError::new(
                ori_diagnostic::ErrorCode::E1001,
                format!(
                    "expected {}, found `{}`",
                    expected.format_expected(),
                    current.display_name()
                ),
                self.cursor.current_span(),
            ))
        }
    }

    // --- Speculative Parsing (Snapshots) ---
    //
    // These methods enable speculative parsing for disambiguation.
    // Use when you need to try a parse, examine the result, and decide
    // whether to keep or discard it.
    //
    // Complements progress tracking:
    // - Progress: simple alternatives (`parse_a().or_else(|| parse_b())`)
    // - Snapshots: complex disambiguation requiring full parse attempts

    /// Create a snapshot of the current parser state.
    ///
    /// The snapshot captures cursor position and context flags. Arena state
    /// is NOT captured—speculative parsing should examine tokens, not allocate.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let snapshot = self.snapshot();
    /// // Try parsing as type
    /// if self.parse_type().is_ok() && self.cursor.check(&TokenKind::Eq) {
    ///     // Commit: this is a type annotation
    /// } else {
    ///     // Rollback and try as expression
    ///     self.restore(snapshot);
    ///     return self.parse_expr();
    /// }
    /// ```
    #[inline]
    pub(crate) fn snapshot(&self) -> crate::snapshot::ParserSnapshot {
        crate::snapshot::ParserSnapshot::new(self.cursor.position(), self.context)
    }

    /// Restore parser state from a snapshot.
    ///
    /// Resets cursor position and context flags to their values when the
    /// snapshot was taken. Does NOT restore arena state.
    #[inline]
    pub(crate) fn restore(&mut self, snapshot: crate::snapshot::ParserSnapshot) {
        self.cursor.set_position(snapshot.cursor_pos);
        self.context = snapshot.context;
    }

    /// Try parsing speculatively, restoring state on failure.
    ///
    /// If the parse function succeeds, returns `Some(result)`.
    /// If it fails, restores parser state and returns `None`.
    ///
    /// This is the primary method for speculative parsing. Use when you
    /// need to try an interpretation and fall back if it doesn't work.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Try parsing as type annotation first
    /// if let Some(ty) = self.try_parse(|p| p.parse_type()) {
    ///     return Ok(TypeOrExpr::Type(ty));
    /// }
    /// // Fall back to expression
    /// let expr = self.parse_expr()?;
    /// Ok(TypeOrExpr::Expr(expr))
    /// ```
    #[inline]
    #[allow(
        dead_code,
        reason = "reserved for grammar disambiguation with backtracking"
    )]
    pub(crate) fn try_parse<T, F>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce(&mut Self) -> Result<T, ParseError>,
    {
        let snapshot = self.snapshot();
        if let Ok(result) = f(self) {
            Some(result)
        } else {
            self.restore(snapshot);
            None
        }
    }

    /// Look ahead without side effects.
    ///
    /// Executes the function and then always restores state, returning
    /// whatever the function returned. Use for peeking ahead to make
    /// parsing decisions without consuming tokens.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Check if this looks like a type annotation
    /// let is_type_annotation = self.look_ahead(|p| {
    ///     p.parse_type().is_ok() && p.cursor.check(&TokenKind::Eq)
    /// });
    ///
    /// if is_type_annotation {
    ///     // Parse as type annotation
    /// } else {
    ///     // Parse as expression
    /// }
    /// ```
    #[inline]
    #[allow(
        dead_code,
        reason = "reserved for non-consuming lookahead in grammar disambiguation"
    )]
    pub(crate) fn look_ahead<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let snapshot = self.snapshot();
        let result = f(self);
        self.restore(snapshot);
        result
    }

    /// Handle a `ParseOutcome` by pushing to a collection on success, or recording error and recovering.
    ///
    /// Like `handle_parse_result` but for `ParseOutcome`:
    /// - `ConsumedOk` / `EmptyOk`: push value to collection
    /// - `ConsumedErr`: recover to sync point, then record error
    /// - `EmptyErr`: convert to `ParseError` and record (no recovery needed — no tokens consumed)
    pub(super) fn handle_outcome<T>(
        &mut self,
        outcome: ParseOutcome<T>,
        collection: &mut Vec<T>,
        errors: &mut Vec<ParseError>,
        recover: impl FnOnce(&mut Self),
    ) {
        match outcome {
            ParseOutcome::ConsumedOk { value } | ParseOutcome::EmptyOk { value } => {
                collection.push(value);
            }
            ParseOutcome::ConsumedErr { error, .. } => {
                recover(self);
                errors.push(error);
            }
            ParseOutcome::EmptyErr { expected, position } => {
                errors.push(ParseError::from_expected_tokens(&expected, position));
            }
        }
    }
}
