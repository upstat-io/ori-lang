//! Progress tracking for parser error recovery.
//!
//! This module implements progress-aware parsing results, inspired by the Roc compiler.
//! The key insight is distinguishing between:
//! - Errors that occurred without consuming any tokens (can try alternatives)
//! - Errors that occurred after consuming tokens (should commit and report)
//!
//! This enables better error recovery decisions in the parser.

use crate::ParseError;

/// Indicates whether parsing made progress (consumed tokens).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Parser consumed one or more tokens before the result.
    Made,
    /// Parser did not consume any tokens.
    None,
}

impl Progress {
    /// Returns true if progress was made.
    pub fn made(self) -> bool {
        matches!(self, Progress::Made)
    }

    /// Returns true if no progress was made.
    pub fn none(self) -> bool {
        matches!(self, Progress::None)
    }

    /// Combines two progress values - Made if either made progress.
    #[must_use]
    pub fn or(self, other: Progress) -> Progress {
        if self.made() || other.made() {
            Progress::Made
        } else {
            Progress::None
        }
    }
}

/// A parse result that tracks whether progress was made.
///
/// This is the core type for progress-aware parsing. It combines:
/// - Whether the parser consumed any tokens (`progress`)
/// - The actual result (`result`)
///
/// The progress information enables better error recovery:
/// - `Progress::None` + error → can try alternative productions
/// - `Progress::Made` + error → commit to this path and report error
#[derive(Debug)]
pub struct ParseResult<T> {
    /// Whether the parser consumed tokens.
    pub progress: Progress,
    /// The actual parse result.
    pub result: Result<T, ParseError>,
}

impl<T> ParseResult<T> {
    /// Creates a successful result with progress made.
    pub fn ok(value: T) -> Self {
        Self {
            progress: Progress::Made,
            result: Ok(value),
        }
    }

    /// Creates a successful result with specified progress.
    pub fn ok_with(value: T, progress: Progress) -> Self {
        Self {
            progress,
            result: Ok(value),
        }
    }

    /// Creates an error result without progress.
    pub fn err_none(error: ParseError) -> Self {
        Self {
            progress: Progress::None,
            result: Err(error),
        }
    }

    /// Creates an error result with progress made.
    pub fn err_made(error: ParseError) -> Self {
        Self {
            progress: Progress::Made,
            result: Err(error),
        }
    }

    /// Creates an error result with specified progress.
    pub fn err_with(error: ParseError, progress: Progress) -> Self {
        Self {
            progress,
            result: Err(error),
        }
    }

    /// Returns true if parsing succeeded.
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Returns true if parsing failed.
    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }

    /// Returns true if progress was made (regardless of success).
    pub fn made_progress(&self) -> bool {
        self.progress.made()
    }

    /// Returns true if no progress was made (regardless of success).
    pub fn no_progress(&self) -> bool {
        self.progress.none()
    }

    /// Returns true if failed without making progress.
    /// This is the key condition for trying alternatives.
    pub fn failed_without_progress(&self) -> bool {
        self.is_err() && self.no_progress()
    }

    /// Returns true if failed after making progress.
    /// This indicates we should commit to this parse path.
    pub fn failed_with_progress(&self) -> bool {
        self.is_err() && self.made_progress()
    }

    /// Maps the success value, preserving progress.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ParseResult<U> {
        ParseResult {
            progress: self.progress,
            result: self.result.map(f),
        }
    }

    /// Maps the error, preserving progress.
    #[must_use]
    pub fn map_err<F: FnOnce(ParseError) -> ParseError>(self, f: F) -> Self {
        ParseResult {
            progress: self.progress,
            result: self.result.map_err(f),
        }
    }

    /// Chains parsing operations, combining progress.
    ///
    /// If this result is Ok, runs the function and combines progress.
    /// If this result is Err, returns the error with current progress.
    pub fn and_then<U, F: FnOnce(T) -> ParseResult<U>>(self, f: F) -> ParseResult<U> {
        match self.result {
            Ok(value) => {
                let next = f(value);
                ParseResult {
                    progress: self.progress.or(next.progress),
                    result: next.result,
                }
            }
            Err(e) => ParseResult {
                progress: self.progress,
                result: Err(e),
            },
        }
    }

    /// Tries an alternative if this failed without progress.
    ///
    /// This is the key combinator for backtracking:
    /// - If succeeded: return this result
    /// - If failed with progress: return this error (committed)
    /// - If failed without progress: try the alternative
    #[must_use]
    pub fn or_else<F: FnOnce() -> ParseResult<T>>(self, f: F) -> ParseResult<T> {
        if self.is_ok() || self.made_progress() {
            self
        } else {
            f()
        }
    }

    /// Unwraps the result, panicking if it's an error.
    #[track_caller]
    #[allow(clippy::unwrap_used)] // Intentional panic for test assertions
    pub fn unwrap(self) -> T {
        self.result.unwrap()
    }

    /// Unwraps the result or returns a default.
    pub fn unwrap_or(self, default: T) -> T {
        self.result.unwrap_or(default)
    }

    /// Unwraps the result or computes a default.
    pub fn unwrap_or_else<F: FnOnce(ParseError) -> T>(self, f: F) -> T {
        self.result.unwrap_or_else(f)
    }

    /// Converts to a standard Result, discarding progress info.
    pub fn into_result(self) -> Result<T, ParseError> {
        self.result
    }

    /// Returns the progress value.
    pub fn progress(&self) -> Progress {
        self.progress
    }
}

impl<T> From<Result<T, ParseError>> for ParseResult<T> {
    /// Converts a `Result` into a `ParseResult` with `Progress::Made`.
    /// Use this when converting from existing code that returns `Result`.
    fn from(result: Result<T, ParseError>) -> Self {
        Self {
            progress: Progress::Made,
            result,
        }
    }
}

/// Extension trait to add progress tracking to `Result`s.
pub trait WithProgress<T> {
    /// Wraps in `ParseResult` with `Progress::Made`.
    fn with_progress_made(self) -> ParseResult<T>;

    /// Wraps in `ParseResult` with `Progress::None`.
    fn with_progress_none(self) -> ParseResult<T>;

    /// Wraps in `ParseResult` with specified progress.
    fn with_progress(self, progress: Progress) -> ParseResult<T>;
}

impl<T> WithProgress<T> for Result<T, ParseError> {
    fn with_progress_made(self) -> ParseResult<T> {
        ParseResult {
            progress: Progress::Made,
            result: self,
        }
    }

    fn with_progress_none(self) -> ParseResult<T> {
        ParseResult {
            progress: Progress::None,
            result: self,
        }
    }

    fn with_progress(self, progress: Progress) -> ParseResult<T> {
        ParseResult {
            progress,
            result: self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::Span;

    fn make_error() -> ParseError {
        ParseError::new(
            ori_diagnostic::ErrorCode::E1001,
            "test error",
            Span::new(0, 1),
        )
    }

    #[test]
    fn test_progress_or() {
        assert_eq!(Progress::None.or(Progress::None), Progress::None);
        assert_eq!(Progress::None.or(Progress::Made), Progress::Made);
        assert_eq!(Progress::Made.or(Progress::None), Progress::Made);
        assert_eq!(Progress::Made.or(Progress::Made), Progress::Made);
    }

    #[test]
    fn test_ok_result() {
        let result: ParseResult<i32> = ParseResult::ok(42);
        assert!(result.is_ok());
        assert!(result.made_progress());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_err_none_result() {
        let result: ParseResult<i32> = ParseResult::err_none(make_error());
        assert!(result.is_err());
        assert!(result.no_progress());
        assert!(result.failed_without_progress());
    }

    #[test]
    fn test_err_made_result() {
        let result: ParseResult<i32> = ParseResult::err_made(make_error());
        assert!(result.is_err());
        assert!(result.made_progress());
        assert!(result.failed_with_progress());
    }

    #[test]
    fn test_map() {
        let result = ParseResult::ok(42).map(|x| x * 2);
        assert_eq!(result.unwrap(), 84);
    }

    #[test]
    fn test_and_then_success() {
        let result = ParseResult::ok(42).and_then(|x| ParseResult::ok(x * 2));
        assert!(result.made_progress());
        assert_eq!(result.unwrap(), 84);
    }

    #[test]
    fn test_and_then_combines_progress() {
        let result = ParseResult::ok_with(42, Progress::None).and_then(|x| ParseResult::ok(x * 2));
        // Progress::None.or(Progress::Made) = Progress::Made
        assert!(result.made_progress());
    }

    #[test]
    fn test_or_else_on_success() {
        let result = ParseResult::ok(42).or_else(|| ParseResult::ok(0));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_or_else_on_err_made() {
        // Error with progress - stays committed
        let result = ParseResult::<i32>::err_made(make_error()).or_else(|| ParseResult::ok(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_or_else_on_err_none() {
        // Error without progress - tries alternative
        let result = ParseResult::<i32>::err_none(make_error()).or_else(|| ParseResult::ok(0));
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_with_progress_extension() {
        let ok: Result<i32, ParseError> = Ok(42);
        let result = ok.with_progress_made();
        assert!(result.made_progress());
        assert_eq!(result.unwrap(), 42);

        let err: Result<i32, ParseError> = Err(make_error());
        let result = err.with_progress_none();
        assert!(result.no_progress());
        assert!(result.is_err());
    }
}
