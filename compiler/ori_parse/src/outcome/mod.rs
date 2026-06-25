//! Four-way parse outcome for Elm-style progress tracking.
//!
//! This module provides `ParseOutcome`, a four-way result type
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
//! ## Integration
//!
//! `ParseOutcome` is the primary parse result type. Convert to `Result<T, ParseError>`
//! via the `From` impl when needed.

mod macros;

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
        /// Byte offset in the source where the mismatch occurred.
        position: usize,
    },
}

impl<T> ParseOutcome<T> {
    // Constructors

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
    #[cold]
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
    #[cold]
    pub fn empty_err_expected(kind: &ori_ir::TokenKind, position: usize) -> Self {
        Self::EmptyErr {
            expected: TokenSet::single(kind.clone()),
            position,
        }
    }

    // Predicates

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

    // Transformations

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
                        // Why: after consumed-ok, progress is committed, so an
                        // empty error must surface as a ConsumedErr (no backtrack).
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "position fits in u32 for source files"
                        )]
                        ParseOutcome::ConsumedErr {
                            error: ParseError::from_expected_tokens(&expected, position),
                            consumed_span: Span::point(position as u32),
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

// Conversions

impl<T> From<ParseOutcome<T>> for Result<T, ParseError> {
    fn from(outcome: ParseOutcome<T>) -> Self {
        outcome.into_result()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
