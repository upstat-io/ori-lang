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

// === Conversions ===

impl<T> From<ParseOutcome<T>> for Result<T, ParseError> {
    fn from(outcome: ParseOutcome<T>) -> Self {
        outcome.into_result()
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
        let mut last_position: usize = $self.cursor.position();

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
                    consumed_span: ori_ir::Span::point(position as u32),
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

/// Bridge a `Result<T, ParseError>` into a `ParseOutcome`-returning function after commitment.
///
/// Use inside functions returning `ParseOutcome<T>` when you've already committed
/// to a parse path (consumed some tokens). All `Result::Err` values become
/// `ConsumedErr` since backtracking is no longer possible.
///
/// This is the `ParseOutcome` equivalent of the `?` operator for `Result` calls
/// in the committed (post-entry-check) section of a grammar function.
///
/// # Behavior
///
/// - `Ok(value)`: Extracts the value and continues
/// - `Err(error)`: Returns `ConsumedErr` from the enclosing function
///
/// # Usage
///
/// ```ignore
/// fn parse_generics(&mut self) -> ParseOutcome<GenericParamRange> {
///     if !self.check(&TokenKind::Lt) {
///         return ParseOutcome::empty_err_expected(&TokenKind::Lt, self.position());
///     }
///     // Committed: `<` confirmed present, all errors are hard errors
///     committed!(self.expect(&TokenKind::Lt));
///     let params = committed!(self.series(...));
///     committed!(self.expect(&TokenKind::Gt));
///     ParseOutcome::consumed_ok(self.arena.alloc_generic_params(params))
/// }
/// ```
///
/// # Note
///
/// Unlike `chain!` (which takes `ParseOutcome` input), this macro bridges
/// `Result<T, ParseError>` input. Use `chain!` when calling functions that
/// already return `ParseOutcome`, and `committed!` when calling functions
/// that still return `Result` (like `expect()`, `series()`, `expect_ident()`).
#[macro_export]
macro_rules! committed {
    ($expr:expr) => {
        match $expr {
            Ok(value) => value,
            Err(error) => {
                let span = error.span;
                return $crate::ParseOutcome::ConsumedErr {
                    error,
                    consumed_span: span,
                };
            }
        }
    };
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
