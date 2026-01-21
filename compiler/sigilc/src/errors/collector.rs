// Diagnostic collector for accumulating multiple errors
//
// Allows compilation phases to continue after encountering errors,
// collecting all diagnostics for reporting.

use super::{Diagnostic, Level};

/// Accumulates diagnostics from a compilation phase.
///
/// This allows phases like parsing and type checking to continue after
/// encountering errors, collecting all issues for a better user experience.
#[derive(Debug, Default)]
pub struct DiagnosticCollector {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticCollector {
    /// Create a new empty collector.
    pub fn new() -> Self {
        DiagnosticCollector {
            diagnostics: Vec::new(),
        }
    }

    /// Add a diagnostic to the collection.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Add an error diagnostic.
    pub fn error(&mut self, diagnostic: Diagnostic) {
        self.push(diagnostic);
    }

    /// Add a warning diagnostic.
    pub fn warning(&mut self, diagnostic: Diagnostic) {
        self.push(diagnostic);
    }

    /// Check if any errors (not just warnings) have been recorded.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == Level::Error)
    }

    /// Check if any diagnostics have been recorded.
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Get the number of errors.
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.level == Level::Error)
            .count()
    }

    /// Get the number of warnings.
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.level == Level::Warning)
            .count()
    }

    /// Get all diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Take ownership of all diagnostics.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    /// Get only error diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.level == Level::Error)
    }

    /// Get only warning diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.level == Level::Warning)
    }

    /// Merge another collector's diagnostics into this one.
    pub fn merge(&mut self, other: DiagnosticCollector) {
        self.diagnostics.extend(other.diagnostics);
    }

    /// Merge a vector of diagnostics into this collector.
    pub fn extend(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics.extend(diagnostics);
    }

    /// Clear all diagnostics.
    pub fn clear(&mut self) {
        self.diagnostics.clear();
    }
}

impl From<Diagnostic> for DiagnosticCollector {
    fn from(diagnostic: Diagnostic) -> Self {
        DiagnosticCollector {
            diagnostics: vec![diagnostic],
        }
    }
}

impl From<Vec<Diagnostic>> for DiagnosticCollector {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        DiagnosticCollector { diagnostics }
    }
}

impl IntoIterator for DiagnosticCollector {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.into_iter()
    }
}

impl<'a> IntoIterator for &'a DiagnosticCollector {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::codes::ErrorCode;

    #[test]
    fn test_empty_collector() {
        let collector = DiagnosticCollector::new();
        assert!(!collector.has_errors());
        assert!(!collector.has_diagnostics());
        assert_eq!(collector.error_count(), 0);
        assert_eq!(collector.warning_count(), 0);
    }

    #[test]
    fn test_add_error() {
        let mut collector = DiagnosticCollector::new();
        collector.push(Diagnostic::error(ErrorCode::E3001, "type mismatch"));

        assert!(collector.has_errors());
        assert!(collector.has_diagnostics());
        assert_eq!(collector.error_count(), 1);
        assert_eq!(collector.warning_count(), 0);
    }

    #[test]
    fn test_add_warning() {
        let mut collector = DiagnosticCollector::new();
        collector.push(Diagnostic::warning(ErrorCode::E3005, "unused variable"));

        assert!(!collector.has_errors());
        assert!(collector.has_diagnostics());
        assert_eq!(collector.error_count(), 0);
        assert_eq!(collector.warning_count(), 1);
    }

    #[test]
    fn test_merge_collectors() {
        let mut collector1 = DiagnosticCollector::new();
        collector1.push(Diagnostic::error(ErrorCode::E3001, "error 1"));

        let mut collector2 = DiagnosticCollector::new();
        collector2.push(Diagnostic::error(ErrorCode::E3002, "error 2"));
        collector2.push(Diagnostic::warning(ErrorCode::E3005, "warning"));

        collector1.merge(collector2);

        assert_eq!(collector1.error_count(), 2);
        assert_eq!(collector1.warning_count(), 1);
    }

    #[test]
    fn test_iteration() {
        let mut collector = DiagnosticCollector::new();
        collector.push(Diagnostic::error(ErrorCode::E3001, "error 1"));
        collector.push(Diagnostic::error(ErrorCode::E3002, "error 2"));

        let mut count = 0;
        for _ in &collector {
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_from_diagnostic() {
        let diag = Diagnostic::error(ErrorCode::E3001, "type mismatch");
        let collector: DiagnosticCollector = diag.into();

        assert_eq!(collector.error_count(), 1);
    }

    #[test]
    fn test_errors_iterator() {
        let mut collector = DiagnosticCollector::new();
        collector.push(Diagnostic::error(ErrorCode::E3001, "error 1"));
        collector.push(Diagnostic::warning(ErrorCode::E3005, "warning"));
        collector.push(Diagnostic::error(ErrorCode::E3002, "error 2"));

        let errors: Vec<_> = collector.errors().collect();
        assert_eq!(errors.len(), 2);
    }
}
