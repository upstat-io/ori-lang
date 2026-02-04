//! Four-way parse outcome for Elm-style progress tracking.
//!
//! This module provides `ParseOutcome`, a more expressive alternative to `ParseResult`
//! that distinguishes between four parsing states:
//!
//! | Progress | Result | Variant | Meaning |
//! |----------|--------|---------|---------|
//! | Consumed | Ok | `ConsumedOk` | Committed to parse path, succeeded |
//! | Empty | Ok | `EmptyOk` | Optional content absent, succeeded |
//! | Consumed | Err | `ConsumedErr` | Real error, no backtracking |
//! | Empty | Err | `EmptyErr` | Try next alternative |
//!
//! ## Design Rationale
//!
//! The key insight from Elm/Roc is that the **combination of progress and result**
//! determines the correct parsing strategy:
//!
//! - `ConsumedErr`: We've committed to a parse path. Report the error, don't backtrack.
//! - `EmptyErr`: We haven't committed yet. Try alternative productions.
//!
//! This enables automatic backtracking without explicit lookahead in many cases.
//!
//! ## Usage
//!
//! ```ignore
//! fn parse_atom(&mut self) -> ParseOutcome<Expr> {
//!     one_of!(self,
//!         self.parse_literal(),    // Try literal first
//!         self.parse_ident(),      // Then identifier
//!         self.parse_paren_expr(), // Then parenthesized
//!     )
//! }
//! ```
//!
//! ## Migration
//!
//! `ParseOutcome` coexists with the existing `ParseResult`. Use `From` conversions
//! to bridge between the two types during gradual migration.

use crate::error::ErrorContext;
use crate::recovery::TokenSet;
use crate::ParseError;
use ori_ir::Span;

/// A four-way parse result distinguishing consumed vs empty and success vs failure.
///
/// This type encodes the Elm/Roc insight that progress information should be
/// tightly coupled with the result type to enable automatic backtracking decisions.
///
/// # Variants
///
/// - `ConsumedOk`: Successfully parsed after consuming input. The parser is committed
///   to this path.
/// - `EmptyOk`: Successfully parsed without consuming input. Used for optional elements.
/// - `ConsumedErr`: Failed after consuming input. This is a hard error; don't backtrack.
/// - `EmptyErr`: Failed without consuming input. Try the next alternative.
///
/// # Type Parameters
///
/// - `T`: The success value type (e.g., `ExprId`, `Type`)
#[derive(Debug)]
pub enum ParseOutcome<T> {
    /// Consumed input and succeeded.
    ///
    /// The parser has committed to this production and produced a value.
    ConsumedOk {
        /// The successfully parsed value.
        value: T,
    },

    /// No input consumed, but succeeded.
    ///
    /// Used for optional parsers (e.g., optional type annotation).
    /// The value is typically a default or `None`.
    EmptyOk {
        /// The value (often a default).
        value: T,
    },

    /// Consumed input then failed.
    ///
    /// This is a hard error. The parser committed to a production but
    /// couldn't complete it. Don't try alternatives; report the error.
    ConsumedErr {
        /// The parse error.
        error: ParseError,
        /// The span of input that was consumed before the error.
        consumed_span: Span,
    },

    /// No input consumed, failed.
    ///
    /// This is a soft error. The parser couldn't match this production
    /// but hasn't committed to it. Try the next alternative.
    EmptyErr {
        /// Set of token kinds that would have been valid here.
        expected: TokenSet,
        /// Position where the mismatch occurred.
        position: usize,
    },
}

impl<T> ParseOutcome<T> {
    // === Constructors ===

    /// Create a successful result that consumed input.
    #[inline]
    pub fn consumed_ok(value: T) -> Self {
        Self::ConsumedOk { value }
    }

    /// Create a successful result that consumed no input.
    #[inline]
    pub fn empty_ok(value: T) -> Self {
        Self::EmptyOk { value }
    }

    /// Create a hard error (consumed input before failing).
    #[inline]
    pub fn consumed_err(error: ParseError, consumed_span: Span) -> Self {
        Self::ConsumedErr {
            error,
            consumed_span,
        }
    }

    /// Create a soft error (no input consumed).
    #[inline]
    pub fn empty_err(expected: TokenSet, position: usize) -> Self {
        Self::EmptyErr { expected, position }
    }

    /// Create a soft error expecting a single token kind.
    #[inline]
    pub fn empty_err_expected(kind: &ori_ir::TokenKind, position: usize) -> Self {
        Self::EmptyErr {
            expected: TokenSet::single(kind.clone()),
            position,
        }
    }

    // === Predicates ===

    /// Returns `true` if the parse succeeded (either variant).
    #[inline]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::ConsumedOk { .. } | Self::EmptyOk { .. })
    }

    /// Returns `true` if the parse failed (either variant).
    #[inline]
    pub fn is_err(&self) -> bool {
        !self.is_ok()
    }

    /// Returns `true` if input was consumed (regardless of success).
    ///
    /// This is the key predicate for backtracking decisions:
    /// - `true`: We're committed to this parse path
    /// - `false`: We can try alternatives
    #[inline]
    pub fn made_progress(&self) -> bool {
        matches!(self, Self::ConsumedOk { .. } | Self::ConsumedErr { .. })
    }

    /// Returns `true` if no input was consumed (regardless of success).
    #[inline]
    pub fn no_progress(&self) -> bool {
        !self.made_progress()
    }

    /// Returns `true` if failed without consuming input.
    ///
    /// This is the condition for trying the next alternative.
    #[inline]
    pub fn failed_without_progress(&self) -> bool {
        matches!(self, Self::EmptyErr { .. })
    }

    /// Returns `true` if failed after consuming input.
    ///
    /// This is a hard error that should be reported, not retried.
    #[inline]
    pub fn failed_with_progress(&self) -> bool {
        matches!(self, Self::ConsumedErr { .. })
    }

    // === Transformations ===

    /// Map the success value, preserving the outcome variant.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ParseOutcome<U> {
        match self {
            Self::ConsumedOk { value } => ParseOutcome::ConsumedOk { value: f(value) },
            Self::EmptyOk { value } => ParseOutcome::EmptyOk { value: f(value) },
            Self::ConsumedErr {
                error,
                consumed_span,
            } => ParseOutcome::ConsumedErr {
                error,
                consumed_span,
            },
            Self::EmptyErr { expected, position } => ParseOutcome::EmptyErr { expected, position },
        }
    }

    /// Map the error, preserving the outcome variant.
    #[must_use]
    pub fn map_err<F: FnOnce(ParseError) -> ParseError>(self, f: F) -> Self {
        match self {
            Self::ConsumedErr {
                error,
                consumed_span,
            } => Self::ConsumedErr {
                error: f(error),
                consumed_span,
            },
            other => other,
        }
    }

    /// Attach error context to hard errors for better error messages.
    ///
    /// Adds "while parsing {context}" information to `ConsumedErr` errors.
    /// `EmptyErr` (soft errors) are not modified since they're used for
    /// backtracking and shouldn't accumulate context.
    ///
    /// # Example
    ///
    /// ```ignore
    /// self.parse_condition()
    ///     .with_error_context(ErrorContext::IfExpression)
    /// ```
    #[must_use]
    pub fn with_error_context(self, context: ErrorContext) -> Self {
        match self {
            Self::ConsumedErr {
                mut error,
                consumed_span,
            } => {
                // Only add context if there isn't already one
                if error.context.is_none() {
                    error.context = Some(format!("while parsing {}", context.description()));
                }
                Self::ConsumedErr {
                    error,
                    consumed_span,
                }
            }
            other => other,
        }
    }

    /// Chain parsing operations, upgrading progress if either consumed.
    ///
    /// If this outcome is successful, runs `f` and combines progress:
    /// - `ConsumedOk` + anything = consumed progress
    /// - `EmptyOk` + consumed = consumed progress
    /// - `EmptyOk` + empty = empty progress
    pub fn and_then<U, F: FnOnce(T) -> ParseOutcome<U>>(self, f: F) -> ParseOutcome<U> {
        match self {
            Self::ConsumedOk { value } => {
                // We've consumed; any result becomes consumed
                match f(value) {
                    ParseOutcome::ConsumedOk { value } | ParseOutcome::EmptyOk { value } => {
                        ParseOutcome::ConsumedOk { value }
                    }
                    ParseOutcome::ConsumedErr {
                        error,
                        consumed_span,
                    } => ParseOutcome::ConsumedErr {
                        error,
                        consumed_span,
                    },
                    ParseOutcome::EmptyErr { expected, position } => {
                        // Empty error after consumed ok becomes consumed error
                        // This should be upgraded to ConsumedErr with a proper error
                        // For now, we create a simple error
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "position fits in u32 for source files"
                        )]
                        ParseOutcome::ConsumedErr {
                            error: ParseError::from_expected_tokens(&expected, position),
                            consumed_span: Span::new(position as u32, 0),
                        }
                    }
                }
            }
            Self::EmptyOk { value } => f(value), // Pass through progress from f
            Self::ConsumedErr {
                error,
                consumed_span,
            } => ParseOutcome::ConsumedErr {
                error,
                consumed_span,
            },
            Self::EmptyErr { expected, position } => ParseOutcome::EmptyErr { expected, position },
        }
    }

    /// Try an alternative if this failed without progress.
    ///
    /// This is the key combinator for automatic backtracking:
    /// - If succeeded: return this result
    /// - If failed with progress (hard error): return this error
    /// - If failed without progress (soft error): try the alternative
    #[must_use]
    pub fn or_else<F: FnOnce() -> ParseOutcome<T>>(self, f: F) -> ParseOutcome<T> {
        match self {
            Self::ConsumedOk { .. } | Self::EmptyOk { .. } | Self::ConsumedErr { .. } => self,
            Self::EmptyErr { .. } => f(),
        }
    }

    /// Try an alternative, accumulating expected tokens on soft errors.
    ///
    /// Like `or_else`, but merges the expected token sets when both
    /// alternatives fail without progress. This produces better error
    /// messages like "expected `(`, `[`, or identifier".
    #[must_use]
    pub fn or_else_accumulate<F: FnOnce() -> ParseOutcome<T>>(self, f: F) -> ParseOutcome<T> {
        match self {
            Self::ConsumedOk { .. } | Self::EmptyOk { .. } | Self::ConsumedErr { .. } => self,
            Self::EmptyErr {
                mut expected,
                position,
            } => match f() {
                ok @ (ParseOutcome::ConsumedOk { .. } | ParseOutcome::EmptyOk { .. }) => ok,
                err @ ParseOutcome::ConsumedErr { .. } => err,
                ParseOutcome::EmptyErr {
                    expected: other_expected,
                    position: other_position,
                } => {
                    // Merge expected sets, use later position
                    expected.union_with(&other_expected);
                    ParseOutcome::EmptyErr {
                        expected,
                        position: other_position.max(position),
                    }
                }
            },
        }
    }

    /// Unwrap the success value, panicking on error.
    ///
    /// # Panics
    /// Panics if this is an error variant.
    #[track_caller]
    pub fn unwrap(self) -> T {
        match self {
            Self::ConsumedOk { value } | Self::EmptyOk { value } => value,
            Self::ConsumedErr { error, .. } => {
                panic!("called `ParseOutcome::unwrap()` on `ConsumedErr`: {error:?}")
            }
            Self::EmptyErr { expected, position } => {
                panic!(
                    "called `ParseOutcome::unwrap()` on `EmptyErr` at position {position}: expected {}",
                    expected.format_expected()
                )
            }
        }
    }

    /// Get the success value, or return a default on error.
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Self::ConsumedOk { value } | Self::EmptyOk { value } => value,
            Self::ConsumedErr { .. } | Self::EmptyErr { .. } => default,
        }
    }

    /// Get the success value, or compute a default on error.
    pub fn unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T {
        match self {
            Self::ConsumedOk { value } | Self::EmptyOk { value } => value,
            Self::ConsumedErr { .. } | Self::EmptyErr { .. } => f(),
        }
    }

    /// Convert to Option, discarding error information.
    pub fn ok(self) -> Option<T> {
        match self {
            Self::ConsumedOk { value } | Self::EmptyOk { value } => Some(value),
            Self::ConsumedErr { .. } | Self::EmptyErr { .. } => None,
        }
    }

    /// Convert to `Result`, converting `EmptyErr` to a `ParseError`.
    pub fn into_result(self) -> Result<T, ParseError> {
        match self {
            Self::ConsumedOk { value } | Self::EmptyOk { value } => Ok(value),
            Self::ConsumedErr { error, .. } => Err(error),
            Self::EmptyErr { expected, position } => {
                Err(ParseError::from_expected_tokens(&expected, position))
            }
        }
    }
}

// === Conversions ===

impl<T> From<ParseOutcome<T>> for Result<T, ParseError> {
    fn from(outcome: ParseOutcome<T>) -> Self {
        outcome.into_result()
    }
}

/// Extension methods to convert from existing `ParseResult`.
impl<T> From<crate::ParseResult<T>> for ParseOutcome<T> {
    fn from(result: crate::ParseResult<T>) -> Self {
        use crate::Progress;
        match (result.progress, result.result) {
            (Progress::Made, Ok(value)) => ParseOutcome::ConsumedOk { value },
            (Progress::None, Ok(value)) => ParseOutcome::EmptyOk { value },
            (Progress::Made, Err(error)) => ParseOutcome::ConsumedErr {
                consumed_span: error.span,
                error,
            },
            (Progress::None, Err(error)) => {
                // Convert to EmptyErr with empty expected set
                // In practice, callers should provide expected tokens
                ParseOutcome::EmptyErr {
                    expected: TokenSet::new(),
                    position: error.span.start as usize,
                }
            }
        }
    }
}

impl<T> From<ParseOutcome<T>> for crate::ParseResult<T> {
    fn from(outcome: ParseOutcome<T>) -> Self {
        use crate::Progress;
        match outcome {
            ParseOutcome::ConsumedOk { value } => crate::ParseResult {
                progress: Progress::Made,
                result: Ok(value),
            },
            ParseOutcome::EmptyOk { value } => crate::ParseResult {
                progress: Progress::None,
                result: Ok(value),
            },
            ParseOutcome::ConsumedErr { error, .. } => crate::ParseResult {
                progress: Progress::Made,
                result: Err(error),
            },
            ParseOutcome::EmptyErr { expected, position } => crate::ParseResult {
                progress: Progress::None,
                result: Err(ParseError::from_expected_tokens(&expected, position)),
            },
        }
    }
}

// === Backtracking Macros ===
//
// These macros implement Elm/Roc-style automatic backtracking using the
// four-way distinction in `ParseOutcome`. The key insight:
//
// - `ConsumedErr`: Hard error - don't backtrack, report immediately
// - `EmptyErr`: Soft error - try the next alternative
//
// This enables clean alternative parsing without explicit lookahead.

/// Try multiple parsing alternatives, using automatic backtracking.
///
/// The `one_of!` macro evaluates each parser in order. For each parser:
/// - `ConsumedOk` or `EmptyOk`: Return this result immediately
/// - `ConsumedErr`: Return this error immediately (hard error, committed)
/// - `EmptyErr`: Accumulate expected tokens and try the next alternative
///
/// If all alternatives fail with `EmptyErr`, returns a merged `EmptyErr` with
/// all accumulated expected tokens.
///
/// # Usage
///
/// ```ignore
/// fn parse_atom(&mut self) -> ParseOutcome<ExprId> {
///     one_of!(self,
///         self.parse_literal(),
///         self.parse_ident(),
///         self.parse_paren_expr(),
///     )
/// }
/// ```
///
/// # Note
///
/// The parser (`$self`) must have a `snapshot()` and `restore()` method for
/// rollback on soft errors. Each alternative is evaluated fresh from the
/// original position.
#[macro_export]
macro_rules! one_of {
    ($self:expr, $first:expr $(, $rest:expr)* $(,)?) => {{
        let original = $self.snapshot();
        let mut accumulated_expected = $crate::recovery::TokenSet::new();
        let mut last_position: usize = $self.position();

        // Try first alternative
        match $first {
            outcome @ $crate::ParseOutcome::ConsumedOk { .. } => outcome,
            outcome @ $crate::ParseOutcome::EmptyOk { .. } => outcome,
            outcome @ $crate::ParseOutcome::ConsumedErr { .. } => outcome,
            $crate::ParseOutcome::EmptyErr { expected, position } => {
                accumulated_expected.union_with(&expected);
                last_position = last_position.max(position);
                $self.restore(original.clone());

                // Try remaining alternatives
                one_of!(@rest $self, original, accumulated_expected, last_position $(, $rest)*)
            }
        }
    }};

    // Internal: process remaining alternatives
    (@rest $self:expr, $original:expr, $accumulated:expr, $last_pos:expr $(,)?) => {{
        // No more alternatives - return accumulated EmptyErr
        $crate::ParseOutcome::EmptyErr {
            expected: $accumulated,
            position: $last_pos,
        }
    }};

    (@rest $self:expr, $original:expr, $accumulated:expr, $last_pos:expr, $next:expr $(, $rest:expr)* $(,)?) => {{
        match $next {
            outcome @ $crate::ParseOutcome::ConsumedOk { .. } => outcome,
            outcome @ $crate::ParseOutcome::EmptyOk { .. } => outcome,
            outcome @ $crate::ParseOutcome::ConsumedErr { .. } => outcome,
            $crate::ParseOutcome::EmptyErr { expected, position } => {
                let mut acc = $accumulated;
                acc.union_with(&expected);
                let new_pos = $last_pos.max(position);
                $self.restore($original.clone());
                one_of!(@rest $self, $original, acc, new_pos $(, $rest)*)
            }
        }
    }};
}

/// Try to parse something optional, returning `Some(value)` on success or `None` on soft error.
///
/// Unlike `one_of!`, this macro is for single optional elements, not alternatives.
///
/// # Behavior
///
/// - `ConsumedOk` or `EmptyOk`: Return `Some(value)`
/// - `ConsumedErr`: Propagate the error (hard error)
/// - `EmptyErr`: Return `None` (soft error, nothing consumed)
///
/// # Usage
///
/// ```ignore
/// fn parse_optional_type_annotation(&mut self) -> ParseOutcome<Option<TypeId>> {
///     let ty = try_outcome!(self, self.parse_type_annotation());
///     ParseOutcome::consumed_ok(ty)
/// }
/// ```
///
/// # Note
///
/// This macro should be used inside a function returning `ParseOutcome<T>`.
/// On `ConsumedErr`, it returns early from the enclosing function.
/// On `EmptyErr`, it evaluates to `None` and continues execution.
#[macro_export]
macro_rules! try_outcome {
    ($self:expr, $parser:expr) => {{
        let snapshot = $self.snapshot();
        match $parser {
            $crate::ParseOutcome::ConsumedOk { value } => Some(value),
            $crate::ParseOutcome::EmptyOk { value } => Some(value),
            $crate::ParseOutcome::ConsumedErr {
                error,
                consumed_span,
            } => {
                // Hard error: propagate immediately
                return $crate::ParseOutcome::ConsumedErr {
                    error,
                    consumed_span,
                };
            }
            $crate::ParseOutcome::EmptyErr { .. } => {
                // Soft error: restore and return None
                $self.restore(snapshot);
                None
            }
        }
    }};
}

/// Require a successful parse, upgrading soft errors to hard errors with context.
///
/// This macro is for mandatory elements where failure should be reported
/// with context about what was being parsed.
///
/// # Behavior
///
/// - `ConsumedOk` or `EmptyOk`: Return the value
/// - `ConsumedErr`: Propagate unchanged (already a hard error)
/// - `EmptyErr`: Convert to `ConsumedErr` with enriched error message
///
/// # Usage
///
/// ```ignore
/// fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
///     self.expect(&TokenKind::If)?;  // Already consumed 'if'
///     let cond = require!(self, self.parse_expr(), "condition in if expression");
///     // ...
/// }
/// ```
///
/// # Note
///
/// Use this after you've committed to a parse path (consumed some tokens).
/// The context message helps users understand what the parser was expecting.
#[macro_export]
macro_rules! require {
    ($self:expr, $parser:expr, $context:expr) => {{
        match $parser {
            $crate::ParseOutcome::ConsumedOk { value } => value,
            $crate::ParseOutcome::EmptyOk { value } => value,
            $crate::ParseOutcome::ConsumedErr {
                error,
                consumed_span,
            } => {
                return $crate::ParseOutcome::ConsumedErr {
                    error,
                    consumed_span,
                };
            }
            $crate::ParseOutcome::EmptyErr { expected, position } => {
                // Convert soft error to hard error with context
                let error = $crate::ParseError::from_expected_tokens_with_context(
                    &expected, position, $context,
                );
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "position fits in u32 for source files"
                )]
                return $crate::ParseOutcome::ConsumedErr {
                    error,
                    consumed_span: ori_ir::Span::new(position as u32, 0),
                };
            }
        }
    }};
}

/// Chain a parse result with progress tracking.
///
/// Similar to `and_then`, but as a macro for use in complex parsing flows
/// where you need to sequence multiple parses while accumulating progress.
///
/// # Behavior
///
/// - Success: Binds the value to the pattern and continues
/// - Error: Returns early with the error
///
/// # Usage
///
/// ```ignore
/// fn parse_binary(&mut self) -> ParseOutcome<ExprId> {
///     let lhs = chain!(self, self.parse_atom());
///     let op = chain!(self, self.parse_operator());
///     let rhs = chain!(self, self.parse_atom());
///     ParseOutcome::consumed_ok(self.make_binary(lhs, op, rhs))
/// }
/// ```
#[macro_export]
macro_rules! chain {
    ($self:expr, $parser:expr) => {{
        match $parser {
            $crate::ParseOutcome::ConsumedOk { value }
            | $crate::ParseOutcome::EmptyOk { value } => value,
            $crate::ParseOutcome::ConsumedErr {
                error,
                consumed_span,
            } => {
                return $crate::ParseOutcome::ConsumedErr {
                    error,
                    consumed_span,
                };
            }
            $crate::ParseOutcome::EmptyErr { expected, position } => {
                return $crate::ParseOutcome::EmptyErr { expected, position };
            }
        }
    }};
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use ori_diagnostic::ErrorCode;
    use ori_ir::TokenKind;

    fn make_error() -> ParseError {
        ParseError::new(ErrorCode::E1001, "test error", Span::new(0, 1))
    }

    #[test]
    fn test_consumed_ok() {
        let outcome: ParseOutcome<i32> = ParseOutcome::consumed_ok(42);
        assert!(outcome.is_ok());
        assert!(outcome.made_progress());
        assert!(!outcome.no_progress());
        assert_eq!(outcome.unwrap(), 42);
    }

    #[test]
    fn test_empty_ok() {
        let outcome: ParseOutcome<i32> = ParseOutcome::empty_ok(42);
        assert!(outcome.is_ok());
        assert!(!outcome.made_progress());
        assert!(outcome.no_progress());
        assert_eq!(outcome.unwrap(), 42);
    }

    #[test]
    fn test_consumed_err() {
        let outcome: ParseOutcome<i32> = ParseOutcome::consumed_err(make_error(), Span::new(0, 10));
        assert!(outcome.is_err());
        assert!(outcome.made_progress());
        assert!(outcome.failed_with_progress());
        assert!(!outcome.failed_without_progress());
    }

    #[test]
    fn test_empty_err() {
        let expected = TokenSet::new().with(TokenKind::LParen);
        let outcome: ParseOutcome<i32> = ParseOutcome::empty_err(expected, 5);
        assert!(outcome.is_err());
        assert!(!outcome.made_progress());
        assert!(!outcome.failed_with_progress());
        assert!(outcome.failed_without_progress());
    }

    #[test]
    fn test_map() {
        let outcome = ParseOutcome::consumed_ok(42).map(|x| x * 2);
        assert_eq!(outcome.unwrap(), 84);

        let outcome = ParseOutcome::empty_ok(42).map(|x| x * 2);
        assert_eq!(outcome.unwrap(), 84);
    }

    #[test]
    fn test_and_then_consumed_ok() {
        let outcome = ParseOutcome::consumed_ok(42).and_then(|x| ParseOutcome::consumed_ok(x * 2));
        assert!(outcome.made_progress());
        assert_eq!(outcome.unwrap(), 84);
    }

    #[test]
    fn test_and_then_empty_ok_to_consumed() {
        // Empty ok followed by consumed becomes consumed
        let outcome = ParseOutcome::empty_ok(42).and_then(|x| ParseOutcome::consumed_ok(x * 2));
        assert!(outcome.made_progress());
        assert_eq!(outcome.unwrap(), 84);
    }

    #[test]
    fn test_and_then_consumed_ok_to_empty() {
        // Consumed followed by empty stays consumed
        let outcome = ParseOutcome::consumed_ok(42).and_then(|x| ParseOutcome::empty_ok(x * 2));
        assert!(outcome.made_progress());
        assert_eq!(outcome.unwrap(), 84);
    }

    #[test]
    fn test_or_else_success() {
        let outcome = ParseOutcome::consumed_ok(42).or_else(|| ParseOutcome::consumed_ok(0));
        assert_eq!(outcome.unwrap(), 42);
    }

    #[test]
    fn test_or_else_consumed_err() {
        // Consumed error doesn't try alternative
        let outcome = ParseOutcome::<i32>::consumed_err(make_error(), Span::new(0, 5))
            .or_else(|| ParseOutcome::consumed_ok(0));
        assert!(outcome.failed_with_progress());
    }

    #[test]
    fn test_or_else_empty_err() {
        // Empty error tries alternative
        let expected = TokenSet::new().with(TokenKind::LParen);
        let outcome =
            ParseOutcome::<i32>::empty_err(expected, 0).or_else(|| ParseOutcome::consumed_ok(99));
        assert_eq!(outcome.unwrap(), 99);
    }

    #[test]
    fn test_or_else_accumulate() {
        let expected1 = TokenSet::new().with(TokenKind::LParen);
        let expected2 = TokenSet::new().with(TokenKind::LBracket);

        let outcome = ParseOutcome::<i32>::empty_err(expected1, 0)
            .or_else_accumulate(|| ParseOutcome::empty_err(expected2, 0));

        if let ParseOutcome::EmptyErr { expected, .. } = outcome {
            assert!(expected.contains(&TokenKind::LParen));
            assert!(expected.contains(&TokenKind::LBracket));
        } else {
            panic!("Expected EmptyErr");
        }
    }

    #[test]
    fn test_into_result() {
        let outcome = ParseOutcome::consumed_ok(42);
        assert_eq!(outcome.into_result().unwrap(), 42);

        let outcome = ParseOutcome::<i32>::consumed_err(make_error(), Span::new(0, 1));
        assert!(outcome.into_result().is_err());
    }

    #[test]
    fn test_with_error_context_on_consumed_err() {
        let outcome: ParseOutcome<i32> = ParseOutcome::consumed_err(make_error(), Span::new(0, 5))
            .with_error_context(ErrorContext::IfExpression);

        if let ParseOutcome::ConsumedErr { error, .. } = outcome {
            assert!(error.context.is_some());
            assert!(error.context.unwrap().contains("if expression"));
        } else {
            panic!("Expected ConsumedErr");
        }
    }

    #[test]
    fn test_with_error_context_preserves_success() {
        let outcome = ParseOutcome::consumed_ok(42).with_error_context(ErrorContext::IfExpression);
        assert!(outcome.is_ok());
        assert_eq!(outcome.unwrap(), 42);
    }

    #[test]
    fn test_with_error_context_preserves_empty_err() {
        let expected = TokenSet::new().with(TokenKind::LParen);
        let outcome = ParseOutcome::<i32>::empty_err(expected, 5)
            .with_error_context(ErrorContext::IfExpression);

        // EmptyErr should not be modified (context only applies to hard errors)
        if let ParseOutcome::EmptyErr {
            expected: e,
            position,
        } = outcome
        {
            assert_eq!(position, 5);
            assert!(e.contains(&TokenKind::LParen));
        } else {
            panic!("Expected EmptyErr");
        }
    }

    #[test]
    fn test_with_error_context_doesnt_overwrite() {
        let mut error = make_error();
        error.context = Some("existing context".to_string());
        let outcome: ParseOutcome<i32> = ParseOutcome::consumed_err(error, Span::new(0, 5))
            .with_error_context(ErrorContext::IfExpression);

        if let ParseOutcome::ConsumedErr { error, .. } = outcome {
            assert_eq!(error.context, Some("existing context".to_string()));
        } else {
            panic!("Expected ConsumedErr");
        }
    }

    #[test]
    fn test_from_parse_result() {
        use crate::{ParseResult, Progress};

        let pr = ParseResult {
            progress: Progress::Made,
            result: Ok(42),
        };
        let outcome: ParseOutcome<i32> = pr.into();
        assert!(matches!(outcome, ParseOutcome::ConsumedOk { value: 42 }));

        let pr = ParseResult {
            progress: Progress::None,
            result: Ok(42),
        };
        let outcome: ParseOutcome<i32> = pr.into();
        assert!(matches!(outcome, ParseOutcome::EmptyOk { value: 42 }));
    }

    // === Macro Tests ===
    //
    // These tests verify the backtracking macros work correctly.
    // We use a simple mock parser that tracks position for snapshot/restore.

    /// Mock parser for testing macros
    struct MockParser {
        position: usize,
    }

    impl MockParser {
        fn new() -> Self {
            Self { position: 0 }
        }

        fn snapshot(&self) -> MockSnapshot {
            MockSnapshot {
                position: self.position,
            }
        }

        #[expect(
            clippy::needless_pass_by_value,
            reason = "matches macro API which clones"
        )]
        fn restore(&mut self, snap: MockSnapshot) {
            self.position = snap.position;
        }

        fn position(&self) -> usize {
            self.position
        }

        fn advance(&mut self) {
            self.position += 1;
        }

        /// Parse something that succeeds after consuming
        fn parse_consuming(&mut self) -> ParseOutcome<i32> {
            self.advance();
            ParseOutcome::consumed_ok(42)
        }

        /// Parse something that succeeds without consuming
        #[expect(
            clippy::unused_self,
            reason = "consistent API with other parse methods"
        )]
        fn parse_empty(&mut self) -> ParseOutcome<i32> {
            ParseOutcome::empty_ok(0)
        }

        /// Parse something that fails without consuming (soft error)
        fn parse_soft_fail(&mut self) -> ParseOutcome<i32> {
            ParseOutcome::empty_err(TokenSet::new().with(TokenKind::LParen), self.position)
        }

        /// Parse something that fails after consuming (hard error)
        #[expect(clippy::cast_possible_truncation, reason = "test position fits in u32")]
        fn parse_hard_fail(&mut self) -> ParseOutcome<i32> {
            self.advance();
            ParseOutcome::consumed_err(make_error(), Span::new(self.position as u32, 1))
        }

        /// Parse something that fails with a different expected token
        fn parse_soft_fail_bracket(&mut self) -> ParseOutcome<i32> {
            ParseOutcome::empty_err(TokenSet::new().with(TokenKind::LBracket), self.position)
        }
    }

    #[derive(Clone)]
    struct MockSnapshot {
        position: usize,
    }

    #[test]
    fn test_one_of_first_succeeds() {
        let mut parser = MockParser::new();
        let result = one_of!(parser, parser.parse_consuming(), parser.parse_soft_fail(),);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_one_of_empty_ok_succeeds() {
        let mut parser = MockParser::new();
        // EmptyOk should also be accepted
        let result = one_of!(parser, parser.parse_empty(), parser.parse_consuming(),);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // parse_empty returns 0
    }

    #[test]
    fn test_one_of_second_succeeds() {
        let mut parser = MockParser::new();
        // First fails soft, second succeeds
        let result = one_of!(parser, parser.parse_soft_fail(), parser.parse_consuming(),);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_one_of_hard_error_propagates() {
        let mut parser = MockParser::new();
        // First fails hard - should propagate, not try second
        let result = one_of!(parser, parser.parse_hard_fail(), parser.parse_consuming(),);
        assert!(result.failed_with_progress());
    }

    #[test]
    fn test_one_of_all_soft_fail_accumulates() {
        let mut parser = MockParser::new();
        // Both fail soft - should accumulate expected tokens
        let result: ParseOutcome<i32> = one_of!(
            parser,
            parser.parse_soft_fail(),
            parser.parse_soft_fail_bracket(),
        );

        if let ParseOutcome::EmptyErr { expected, .. } = result {
            // Should have both expected tokens
            assert!(expected.contains(&TokenKind::LParen));
            assert!(expected.contains(&TokenKind::LBracket));
        } else {
            panic!("Expected EmptyErr with accumulated tokens");
        }
    }

    #[test]
    fn test_one_of_restores_on_soft_fail() {
        let mut parser = MockParser::new();
        let start_pos = parser.position();

        // This will fail soft, should restore position before trying next
        let _result: ParseOutcome<i32> =
            one_of!(parser, parser.parse_soft_fail(), parser.parse_soft_fail(),);

        // Position should still be at start (restored after soft fails)
        assert_eq!(parser.position(), start_pos);
    }

    // Helper functions for chain tests (defined outside test functions per clippy)
    fn parse_with_chain(p: &mut MockParser) -> ParseOutcome<i32> {
        let a = chain!(p, p.parse_consuming());
        let b = chain!(p, p.parse_consuming());
        ParseOutcome::consumed_ok(a + b)
    }

    fn parse_with_chain_fail(p: &mut MockParser) -> ParseOutcome<i32> {
        let _a = chain!(p, p.parse_consuming());
        let _b = chain!(p, p.parse_hard_fail()); // Should propagate
        ParseOutcome::consumed_ok(0) // Never reached
    }

    #[test]
    fn test_chain_success() {
        let mut parser = MockParser::new();
        let result = parse_with_chain(&mut parser);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 84); // 42 + 42
    }

    #[test]
    fn test_chain_propagates_error() {
        let mut parser = MockParser::new();
        let result = parse_with_chain_fail(&mut parser);
        assert!(result.failed_with_progress());
    }
}
