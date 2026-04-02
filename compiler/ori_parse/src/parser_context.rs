//! Context management and error context methods for the Parser.
//!
//! Includes `ParseContext` flag manipulation (context-sensitive parsing)
//! and Elm-style `ErrorContext` wrapping for better error messages.

use crate::context::ParseContext;
use crate::error;
use crate::outcome::ParseOutcome;
use crate::Parser;

impl Parser<'_> {
    // --- Context Management ---
    //
    // These methods support context-sensitive parsing. Some are not yet used
    // internally but are part of the public API for parser extensions and testing.

    /// Get the current parsing context.
    #[inline]
    #[allow(dead_code, reason = "API for tests and future parser extensions")]
    pub(crate) fn context(&self) -> ParseContext {
        self.context
    }

    /// Execute a closure with additional context flags, then restore the original context.
    ///
    /// This is the primary way to temporarily modify parsing context.
    ///
    /// # Example
    /// ```ignore
    /// // Parse condition without allowing struct literals
    /// let cond = self.with_context(ParseContext::NO_STRUCT_LIT, |p| {
    ///     p.parse_expr()
    /// })?;
    /// ```
    #[inline]
    pub(crate) fn with_context<T, F>(&mut self, add: ParseContext, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let old = self.context;
        self.context = self.context.with(add);
        let result = f(self);
        self.context = old;
        result
    }

    /// Execute a closure with context flags removed, then restore the original context.
    ///
    /// # Example
    /// ```ignore
    /// // Parse body allowing struct literals again
    /// let body = self.without_context(ParseContext::NO_STRUCT_LIT, |p| {
    ///     p.parse_expr()
    /// })?;
    /// ```
    #[inline]
    #[allow(dead_code, reason = "API for tests and future parser extensions")]
    pub(crate) fn without_context<T, F>(&mut self, remove: ParseContext, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let old = self.context;
        self.context = self.context.without(remove);
        let result = f(self);
        self.context = old;
        result
    }

    /// Check if a context flag is set.
    #[inline]
    #[allow(dead_code, reason = "API for tests and future parser extensions")]
    pub(crate) fn has_context(&self, flag: ParseContext) -> bool {
        self.context.has(flag)
    }

    /// Check if struct literals are allowed in the current context.
    #[inline]
    pub(crate) fn allows_struct_lit(&self) -> bool {
        self.context.allows_struct_lit()
    }

    // --- Error Context ---
    //
    // These methods support Elm-style error context for better error messages.
    // `ErrorContext` describes what was being parsed when an error occurred,
    // enabling messages like "while parsing an if expression".
    //
    // Note: This is distinct from `ParseContext` (the bitfield for context-sensitive
    // parsing behavior like NO_STRUCT_LIT).

    /// Execute a parser and wrap any hard errors with context.
    ///
    /// This is the Elm-style `in_context` pattern. It:
    /// 1. Runs the provided parser
    /// 2. If it returns `ConsumedErr`, wraps the error with context
    /// 3. Passes through all other outcomes unchanged
    ///
    /// Use this to provide better error messages like "while parsing an if expression".
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
    ///     self.in_error_context(ErrorContext::IfExpression, |p| {
    ///         p.cursor.expect(&TokenKind::If)?;
    ///         let cond = p.parse_expr()?;
    ///         // ...
    ///     })
    /// }
    /// ```
    ///
    /// # Error Messages
    ///
    /// Without context: "expected expression, found `}`"
    /// With context: "expected expression, found `}` (while parsing an if expression)"
    #[inline]
    pub(crate) fn in_error_context<T, F>(
        &mut self,
        context: error::ErrorContext,
        f: F,
    ) -> ParseOutcome<T>
    where
        F: FnOnce(&mut Self) -> ParseOutcome<T>,
    {
        tracing::debug!(context = context.label(), "entering parse context");
        f(self).with_error_context(context)
    }

    /// Attach error context to a `Result`-returning parser function.
    ///
    /// Like [`in_error_context`](Self::in_error_context) but for functions that
    /// return `Result<T, ParseError>` (e.g., postfix operations called via `?`).
    #[inline]
    pub(crate) fn in_error_context_result<T, F>(
        &mut self,
        context: error::ErrorContext,
        f: F,
    ) -> Result<T, crate::ParseError>
    where
        F: FnOnce(&mut Self) -> Result<T, crate::ParseError>,
    {
        tracing::debug!(context = context.label(), "entering parse context");
        f(self).map_err(|mut e| {
            if e.context.is_none() {
                e.context = Some(format!("while parsing {}", context.description()));
            }
            e
        })
    }
}
