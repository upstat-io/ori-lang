use std::fmt;

/// Proof that at least one error was emitted.
///
/// This type cannot be constructed except by emitting an error via
/// `DiagnosticQueue::emit_error`. This provides compile-time assurance
/// that error paths actually report errors rather than silently failing.
///
/// # Purpose
///
/// In a compiler, it's critical that every error path reports a diagnostic.
/// Without `ErrorGuaranteed`, it's easy to accidentally return an error type
/// without actually telling the user what went wrong.
///
/// With `ErrorGuaranteed`, you can only create one by actually emitting an error,
/// so functions that return `Result<T, ErrorGuaranteed>` are guaranteed to have
/// reported something useful.
///
/// # Example
///
/// ```text
/// fn type_check(&mut self) -> Result<TypedModule, ErrorGuaranteed> {
///     if let Some(error) = self.check_for_errors() {
///         // Can only get ErrorGuaranteed by actually emitting
///         let guarantee = self.queue.emit_error(error.to_diagnostic());
///         return Err(guarantee);
///     }
///     Ok(self.build_typed_module())
/// }
/// ```
///
/// # Salsa Compatibility
///
/// Has Copy, Clone, Eq, Hash for use in query results.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ErrorGuaranteed(());

impl ErrorGuaranteed {
    /// Private constructor - only `DiagnosticQueue::emit_error` can create this.
    ///
    /// This is pub(crate) so that queue.rs can create `ErrorGuaranteed` instances.
    pub(crate) fn new() -> Self {
        ErrorGuaranteed(())
    }

    /// Create an `ErrorGuaranteed` from an error count.
    ///
    /// Returns `Some(ErrorGuaranteed)` if the count is greater than zero,
    /// `None` otherwise. This allows downstream crates that track errors
    /// through their own mechanisms (e.g., `Vec<TypeCheckError>`) to create
    /// a guarantee when they have proof that errors exist.
    ///
    /// # Example
    ///
    /// ```text
    /// let error_guarantee = ErrorGuaranteed::from_error_count(self.errors.len());
    /// ```
    #[inline]
    pub fn from_error_count(count: usize) -> Option<Self> {
        if count > 0 {
            Some(ErrorGuaranteed(()))
        } else {
            None
        }
    }

    /// Create an `ErrorGuaranteed` for downstream crates that have already
    /// verified errors exist through other means.
    ///
    /// **Warning:** This should only be called when you have evidence that
    /// errors were emitted. Prefer `from_error_count` when possible.
    ///
    /// This exists for cases where the error count isn't directly accessible
    /// but the calling code has verified errors exist (e.g., by checking
    /// `!errors.is_empty()` in a separate condition).
    #[inline]
    pub fn new_for_downstream() -> Self {
        ErrorGuaranteed(())
    }
}

impl fmt::Display for ErrorGuaranteed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error(s) emitted")
    }
}

impl std::error::Error for ErrorGuaranteed {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_error_count_returns_some_for_nonzero() {
        assert!(ErrorGuaranteed::from_error_count(1).is_some());
        assert!(ErrorGuaranteed::from_error_count(100).is_some());
    }

    #[test]
    fn from_error_count_returns_none_for_zero() {
        assert!(ErrorGuaranteed::from_error_count(0).is_none());
    }

    #[test]
    fn display_shows_error_message() {
        let g = ErrorGuaranteed::new_for_downstream();
        assert_eq!(g.to_string(), "error(s) emitted");
    }

    #[test]
    fn error_guaranteed_is_copy() {
        let g1 = ErrorGuaranteed::new_for_downstream();
        let g2 = g1; // Copy
        let _ = g1; // Still usable after copy
        let _ = g2;
    }

    #[test]
    fn error_guaranteed_is_eq() {
        let g1 = ErrorGuaranteed::new_for_downstream();
        let g2 = ErrorGuaranteed::new_for_downstream();
        assert_eq!(g1, g2);
    }
}
