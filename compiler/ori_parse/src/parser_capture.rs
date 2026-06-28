//! Speculative parsing and outcome handling for the Parser.
//!
//! Includes snapshot-based speculative parsing for disambiguation and the
//! `handle_outcome` helper for collecting parsed items with error recovery.

use crate::outcome::ParseOutcome;
use crate::{ParseError, Parser};

impl Parser<'_> {
    // Speculative parsing (snapshots).

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
    /// Production grammar disambiguates via `snapshot()` / `restore()` inside
    /// the `one_of!` macro; this convenience wrapper is exercised by the
    /// snapshot test suite.
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
    // test-only
    #[cfg(test)]
    #[inline]
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
    // test-only
    #[cfg(test)]
    #[inline]
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
