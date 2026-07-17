//! Backtracking macros for `ParseOutcome` composition.
//!
//! These macros implement Elm/Roc-style automatic backtracking using the
//! four-way distinction in `ParseOutcome`. The key insight:
//!
//! - `ConsumedErr`: Hard error - don't backtrack, report immediately
//! - `EmptyErr`: Soft error - try the next alternative
//!
//! This enables clean alternative parsing without explicit lookahead.

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
/// ```
/// use ori_parse::{one_of, ParseOutcome, TokenSet};
///
/// # #[derive(Clone)]
/// # struct Snapshot;
/// # struct Cursor { position: usize }
/// # impl Cursor { fn position(&self) -> usize { self.position } }
/// # struct DemoParser { cursor: Cursor }
/// # impl DemoParser {
/// #     fn snapshot(&self) -> Snapshot { Snapshot }
/// #     fn restore(&mut self, _: Snapshot) {}
/// fn parse_atom(&mut self) -> ParseOutcome<&'static str> {
///     let no_literal = ParseOutcome::empty_err(TokenSet::new(), self.cursor.position());
///     one_of!(self,
///         no_literal,
///         ParseOutcome::consumed_ok("identifier"),
///     )
/// }
/// # }
/// # let mut parser = DemoParser { cursor: Cursor { position: 0 } };
/// # assert!(matches!(parser.parse_atom(), ParseOutcome::ConsumedOk { value: "identifier" }));
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
        let mut accumulated_expected = $crate::TokenSet::new();
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
/// ```
/// use ori_parse::{try_outcome, ParseOutcome, TokenSet};
///
/// # #[derive(Default)]
/// # struct DemoParser { restores: usize }
/// # impl DemoParser {
/// #     fn snapshot(&self) {}
/// #     fn restore(&mut self, (): ()) { self.restores += 1; }
/// fn parse_optional_type_annotation(&mut self) -> ParseOutcome<Option<u8>> {
///     let absent = ParseOutcome::empty_err(TokenSet::new(), 0);
///     let ty = try_outcome!(self, absent);
///     ParseOutcome::consumed_ok(ty)
/// }
/// # }
/// # let mut parser = DemoParser::default();
/// # assert!(matches!(parser.parse_optional_type_annotation(),
/// #     ParseOutcome::ConsumedOk { value: None }));
/// # assert_eq!(parser.restores, 1);
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
/// ```
/// use ori_parse::{require, ParseOutcome, TokenSet};
///
/// fn parse_required(candidate: ParseOutcome<u8>) -> ParseOutcome<u8> {
///     let value = require!((), candidate, "condition in if expression");
///     ParseOutcome::consumed_ok(value)
/// }
///
/// let missing = ParseOutcome::empty_err(TokenSet::new(), 4);
/// assert!(matches!(parse_required(missing), ParseOutcome::ConsumedErr { .. }));
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
                let consumed_span = error.span();
                return $crate::ParseOutcome::ConsumedErr {
                    error,
                    consumed_span,
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
/// ```
/// use ori_parse::{chain, ParseOutcome};
///
/// fn parse_sum(lhs: ParseOutcome<i32>, rhs: ParseOutcome<i32>) -> ParseOutcome<i32> {
///     let lhs = chain!((), lhs);
///     let rhs = chain!((), rhs);
///     ParseOutcome::consumed_ok(lhs + rhs)
/// }
///
/// let sum = parse_sum(ParseOutcome::consumed_ok(20), ParseOutcome::empty_ok(22));
/// assert!(matches!(sum, ParseOutcome::ConsumedOk { value: 42 }));
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
/// ```
/// use ori_parse::{committed, ParseError, ParseOutcome};
///
/// fn after_commit(result: Result<u8, ParseError>) -> ParseOutcome<u8> {
///     let value = committed!(result);
///     ParseOutcome::consumed_ok(value)
/// }
///
/// let parsed = after_commit(Ok(42));
/// assert!(matches!(parsed, ParseOutcome::ConsumedOk { value: 42 }));
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
                let span = error.span();
                return $crate::ParseOutcome::ConsumedErr {
                    error,
                    consumed_span: span,
                };
            }
        }
    };
}
