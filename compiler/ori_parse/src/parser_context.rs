//! Context management and error context methods for the Parser.
//!
//! Includes `ParseContext` flag manipulation (context-sensitive parsing)
//! and Elm-style `ErrorContext` wrapping for better error messages.

use crate::context::ParseContext;
use crate::error;
use crate::outcome::ParseOutcome;
use crate::Parser;

impl Parser<'_> {
    // Context Management
    //
    // These methods support context-sensitive parsing. `with_context` and
    // `allows_struct_lit` drive production grammar; the test-only helpers
    // (`context`, `without_context`, `has_context`) are exercised by tests only.

    /// Get the current parsing context.
    // test-only
    #[cfg(test)]
    #[inline]
    pub(crate) fn context(&self) -> ParseContext {
        self.context
    }

    /// Execute a closure with additional context flags, then restore the original context.
    ///
    /// This is the primary way to temporarily modify parsing context.
    ///
    /// # Example
    ///
    /// This is a schematic parser-internal fragment, not a standalone program:
    ///
    /// ```text
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
    ///
    /// This is a schematic parser-internal fragment, not a standalone program:
    ///
    /// ```text
    /// // Parse body allowing struct literals again
    /// let body = self.without_context(ParseContext::NO_STRUCT_LIT, |p| {
    ///     p.parse_expr()
    /// })?;
    /// ```
    // test-only
    #[cfg(test)]
    #[inline]
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
    // test-only
    #[cfg(test)]
    #[inline]
    pub(crate) fn has_context(&self, flag: ParseContext) -> bool {
        self.context.has(flag)
    }

    /// Check if struct literals are allowed in the current context.
    #[inline]
    pub(crate) fn allows_struct_lit(&self) -> bool {
        self.context.allows_struct_lit()
    }

    // Error context (distinct from `ParseContext`): wraps hard errors with a
    // "while parsing X" annotation via `ErrorContext`.

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
    /// This is a schematic parser-internal fragment, not a standalone program:
    ///
    /// ```text
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
