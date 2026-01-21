// Phase result type for compiler phases
//
// PhaseResult allows phases to return both a value and diagnostics,
// enabling error recovery and continued compilation after non-fatal errors.

use super::{Diagnostic, DiagnosticCollector};

/// Result of a compilation phase, containing both a value and diagnostics.
///
/// This type enables error recovery by allowing phases to:
/// - Return a partial result even when errors occur
/// - Collect multiple errors before failing
/// - Propagate warnings alongside successful results
#[derive(Debug)]
pub struct PhaseResult<T> {
    /// The result value, if the phase succeeded (possibly with warnings).
    pub value: Option<T>,
    /// All diagnostics (errors and warnings) from the phase.
    pub diagnostics: DiagnosticCollector,
}

impl<T> PhaseResult<T> {
    /// Create a successful result with no diagnostics.
    pub fn ok(value: T) -> Self {
        PhaseResult {
            value: Some(value),
            diagnostics: DiagnosticCollector::new(),
        }
    }

    /// Create a successful result with warnings.
    pub fn ok_with_warnings(value: T, diagnostics: DiagnosticCollector) -> Self {
        PhaseResult {
            value: Some(value),
            diagnostics,
        }
    }

    /// Create a failed result with a single error.
    pub fn err(diagnostic: Diagnostic) -> Self {
        let mut collector = DiagnosticCollector::new();
        collector.push(diagnostic);
        PhaseResult {
            value: None,
            diagnostics: collector,
        }
    }

    /// Create a failed result with multiple diagnostics.
    pub fn fail(diagnostics: DiagnosticCollector) -> Self {
        PhaseResult {
            value: None,
            diagnostics,
        }
    }

    /// Create a result from an optional value and diagnostics.
    pub fn from_parts(value: Option<T>, diagnostics: DiagnosticCollector) -> Self {
        PhaseResult { value, diagnostics }
    }

    /// Check if the phase succeeded (has a value, regardless of warnings).
    pub fn is_ok(&self) -> bool {
        self.value.is_some()
    }

    /// Check if the phase failed (no value).
    pub fn is_err(&self) -> bool {
        self.value.is_none()
    }

    /// Check if there are any errors in the diagnostics.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }

    /// Check if there are any warnings in the diagnostics.
    pub fn has_warnings(&self) -> bool {
        self.diagnostics.warning_count() > 0
    }

    /// Get a reference to the value, if present.
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Take the value, consuming self.
    pub fn into_value(self) -> Option<T> {
        self.value
    }

    /// Get the diagnostics.
    pub fn diagnostics(&self) -> &DiagnosticCollector {
        &self.diagnostics
    }

    /// Take ownership of the diagnostics.
    pub fn into_diagnostics(self) -> DiagnosticCollector {
        self.diagnostics
    }

    /// Convert to a standard Result, failing if there are any errors.
    pub fn into_result(self) -> Result<T, DiagnosticCollector> {
        if self.has_errors() || self.value.is_none() {
            Err(self.diagnostics)
        } else {
            // Safe because we checked value.is_some() above
            Ok(self.value.unwrap())
        }
    }

    /// Convert to a standard Result, including diagnostics on success.
    pub fn into_result_with_diagnostics(self) -> Result<(T, DiagnosticCollector), DiagnosticCollector> {
        if self.has_errors() || self.value.is_none() {
            Err(self.diagnostics)
        } else {
            Ok((self.value.unwrap(), self.diagnostics))
        }
    }

    /// Map the success value.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> PhaseResult<U> {
        PhaseResult {
            value: self.value.map(f),
            diagnostics: self.diagnostics,
        }
    }

    /// Chain another phase, carrying forward diagnostics.
    pub fn and_then<U, F: FnOnce(T) -> PhaseResult<U>>(self, f: F) -> PhaseResult<U> {
        match self.value {
            Some(v) if !self.has_errors() => {
                let mut result = f(v);
                // Prepend our diagnostics to the new result's diagnostics
                let mut combined = self.diagnostics;
                combined.merge(result.diagnostics);
                result.diagnostics = combined;
                result
            }
            _ => PhaseResult {
                value: None,
                diagnostics: self.diagnostics,
            },
        }
    }

    /// Add a diagnostic to this result.
    pub fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    /// Add multiple diagnostics to this result.
    pub fn with_diagnostics(mut self, diagnostics: DiagnosticCollector) -> Self {
        self.diagnostics.merge(diagnostics);
        self
    }

    /// Unwrap the value, panicking with error message if none.
    #[track_caller]
    pub fn unwrap(self) -> T {
        match self.value {
            Some(v) => v,
            None => {
                let errors: Vec<_> = self.diagnostics.errors().collect();
                panic!(
                    "called `PhaseResult::unwrap()` on a failed result with {} errors: {:?}",
                    errors.len(),
                    errors.first().map(|e| &e.message)
                );
            }
        }
    }

    /// Unwrap the value, returning a default if none.
    pub fn unwrap_or(self, default: T) -> T {
        self.value.unwrap_or(default)
    }

    /// Unwrap the value, computing a default if none.
    pub fn unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T {
        self.value.unwrap_or_else(f)
    }
}

impl<T: Default> PhaseResult<T> {
    /// Unwrap the value, returning T::default() if none.
    pub fn unwrap_or_default(self) -> T {
        self.value.unwrap_or_default()
    }
}

impl<T> Default for PhaseResult<T> {
    fn default() -> Self {
        PhaseResult {
            value: None,
            diagnostics: DiagnosticCollector::new(),
        }
    }
}

impl<T> From<Result<T, Diagnostic>> for PhaseResult<T> {
    fn from(result: Result<T, Diagnostic>) -> Self {
        match result {
            Ok(v) => PhaseResult::ok(v),
            Err(e) => PhaseResult::err(e),
        }
    }
}

impl<T> From<Result<T, DiagnosticCollector>> for PhaseResult<T> {
    fn from(result: Result<T, DiagnosticCollector>) -> Self {
        match result {
            Ok(v) => PhaseResult::ok(v),
            Err(diagnostics) => PhaseResult::fail(diagnostics),
        }
    }
}

/// Convert a Result<T, String> to PhaseResult<T> using a fallback error code.
pub fn from_string_result<T>(
    result: Result<T, String>,
    filename: &str,
) -> PhaseResult<T> {
    match result {
        Ok(v) => PhaseResult::ok(v),
        Err(msg) => PhaseResult::err(super::from_string_error(msg, filename)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::codes::ErrorCode;

    #[test]
    fn test_ok_result() {
        let result = PhaseResult::ok(42);
        assert!(result.is_ok());
        assert!(!result.is_err());
        assert!(!result.has_errors());
        assert_eq!(result.value(), Some(&42));
    }

    #[test]
    fn test_err_result() {
        let result: PhaseResult<i32> = PhaseResult::err(
            Diagnostic::error(ErrorCode::E3001, "type mismatch")
        );
        assert!(result.is_err());
        assert!(!result.is_ok());
        assert!(result.has_errors());
        assert_eq!(result.value(), None);
    }

    #[test]
    fn test_ok_with_warnings() {
        let mut warnings = DiagnosticCollector::new();
        warnings.push(Diagnostic::warning(ErrorCode::E3005, "unused variable"));

        let result = PhaseResult::ok_with_warnings(42, warnings);
        assert!(result.is_ok());
        assert!(result.has_warnings());
        assert!(!result.has_errors());
    }

    #[test]
    fn test_into_result() {
        let ok_result = PhaseResult::ok(42);
        assert_eq!(ok_result.into_result().unwrap(), 42);

        let err_result: PhaseResult<i32> = PhaseResult::err(
            Diagnostic::error(ErrorCode::E3001, "error")
        );
        assert!(err_result.into_result().is_err());
    }

    #[test]
    fn test_map() {
        let result = PhaseResult::ok(21);
        let mapped = result.map(|x| x * 2);
        assert_eq!(mapped.value(), Some(&42));
    }

    #[test]
    fn test_and_then() {
        let result = PhaseResult::ok(21);
        let chained = result.and_then(|x| PhaseResult::ok(x * 2));
        assert_eq!(chained.value(), Some(&42));
    }

    #[test]
    fn test_and_then_with_error() {
        let result: PhaseResult<i32> = PhaseResult::err(
            Diagnostic::error(ErrorCode::E3001, "error")
        );
        let chained = result.and_then(|x| PhaseResult::ok(x * 2));
        assert!(chained.is_err());
    }

    #[test]
    fn test_with_diagnostic() {
        let result = PhaseResult::ok(42)
            .with_diagnostic(Diagnostic::warning(ErrorCode::E3005, "warning"));

        assert!(result.is_ok());
        assert!(result.has_warnings());
    }

    #[test]
    fn test_from_result_ok() {
        let std_result: Result<i32, Diagnostic> = Ok(42);
        let phase_result: PhaseResult<i32> = std_result.into();
        assert!(phase_result.is_ok());
        assert_eq!(phase_result.value(), Some(&42));
    }

    #[test]
    fn test_from_result_err() {
        let std_result: Result<i32, Diagnostic> = Err(
            Diagnostic::error(ErrorCode::E3001, "error")
        );
        let phase_result: PhaseResult<i32> = std_result.into();
        assert!(phase_result.is_err());
    }

    #[test]
    fn test_unwrap_or() {
        let ok_result = PhaseResult::ok(42);
        assert_eq!(ok_result.unwrap_or(0), 42);

        let err_result: PhaseResult<i32> = PhaseResult::err(
            Diagnostic::error(ErrorCode::E3001, "error")
        );
        assert_eq!(err_result.unwrap_or(0), 0);
    }
}
