//! Diagnostic Rendering
//!
//! Converts structured Problem types into user-facing Diagnostic messages.
//! This separates the "what went wrong" (Problem) from "how to display it"
//! (Diagnostic).
//!
//! # Design
//!
//! The `Render` trait converts problems to diagnostics with:
//! - Error code for searchability
//! - Clear message explaining what went wrong
//! - Labeled spans showing where
//! - Notes providing context
//! - Suggestions for how to fix
//!
//! Each problem type has its own rendering logic, allowing customized
//! error messages for different situations.

mod parse;
mod type_errors;
mod semantic;

use crate::diagnostic::{Diagnostic, Severity};
use crate::diagnostic::queue::{DiagnosticConfig, DiagnosticQueue};
use crate::problem::Problem;
use crate::typeck::TypeCheckError;

/// Trait for rendering problems into diagnostics.
pub trait Render {
    /// Render this problem into a diagnostic.
    fn render(&self) -> Diagnostic;
}

impl Render for Problem {
    fn render(&self) -> Diagnostic {
        match self {
            Problem::Parse(p) => p.render(),
            Problem::Type(p) => p.render(),
            Problem::Semantic(p) => p.render(),
        }
    }
}

/// Render a collection of problems into diagnostics.
pub fn render_all(problems: &[Problem]) -> Vec<Diagnostic> {
    problems.iter().map(Render::render).collect()
}

/// Process type check errors through the diagnostic queue for filtering and sorting.
///
/// Applies the following error handling strategies:
/// - Error limits to prevent overwhelming output
/// - Deduplication of same-line errors
/// - Soft error suppression after hard errors
/// - Follow-on error filtering
/// - Sorting by source position
///
/// # Arguments
///
/// * `errors` - Type check errors to process
/// * `source` - Source code for computing line numbers
/// * `config` - Optional configuration (uses defaults if None)
///
/// # Returns
///
/// Filtered and sorted diagnostics, ready for display.
pub fn process_type_errors(
    errors: Vec<TypeCheckError>,
    source: &str,
    config: Option<DiagnosticConfig>,
) -> Vec<Diagnostic> {
    let config = config.unwrap_or_default();
    let mut queue = DiagnosticQueue::with_config(config);

    for error in errors {
        let diag = error.to_diagnostic();
        let soft = error.is_soft();
        queue.add_with_source(diag, source, soft);
    }

    queue.flush()
}

/// Process raw diagnostics through the queue for filtering and sorting.
///
/// Similar to `process_type_errors` but works with pre-built Diagnostic objects.
pub fn process_diagnostics(
    diagnostics: Vec<Diagnostic>,
    source: &str,
    config: Option<DiagnosticConfig>,
) -> Vec<Diagnostic> {
    let config = config.unwrap_or_default();
    let mut queue = DiagnosticQueue::with_config(config);

    for diag in diagnostics {
        // All non-TypeCheckError diagnostics are considered hard errors
        queue.add_with_source(diag, source, false);
    }

    queue.flush()
}

/// A report containing multiple diagnostics.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    pub fn new() -> Self {
        Report {
            diagnostics: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Report {
            diagnostics: Vec::with_capacity(capacity),
        }
    }

    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn add_problem(&mut self, problem: &Problem) {
        self.diagnostics.push(problem.render());
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(sigil_diagnostic::Diagnostic::is_error)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| matches!(d.severity, Severity::Warning))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::ErrorCode;
    use crate::ir::Span;
    use crate::problem::{ParseProblem, TypeProblem, SemanticProblem};
    use crate::problem::semantic::DefinitionKind;

    #[test]
    fn test_render_parse_problem() {
        let problem = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        };

        let diag = problem.render();

        assert_eq!(diag.code, ErrorCode::E1001);
        assert!(diag.message.contains("unexpected token"));
        assert!(diag.message.contains("expression"));
        assert!(diag.message.contains("}"));
        assert!(diag.is_error());
    }

    #[test]
    fn test_render_type_mismatch() {
        let problem = TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "int".into(),
            found: "str".into(),
        };

        let diag = problem.render();

        assert_eq!(diag.code, ErrorCode::E2001);
        assert!(diag.message.contains("type mismatch"));
        assert!(diag.message.contains("int"));
        assert!(diag.message.contains("str"));
    }

    #[test]
    fn test_render_unknown_identifier_with_suggestion() {
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: Some("for".into()),
        };

        let diag = problem.render();

        assert_eq!(diag.code, ErrorCode::E2003);
        assert!(diag.message.contains("unknown identifier"));
        assert!(diag.suggestions.iter().any(|s| s.contains("for")));
    }

    #[test]
    fn test_render_duplicate_definition() {
        let problem = SemanticProblem::DuplicateDefinition {
            span: Span::new(100, 110),
            name: "bar".into(),
            kind: DefinitionKind::Function,
            first_span: Span::new(10, 20),
        };

        let diag = problem.render();

        assert_eq!(diag.code, ErrorCode::E2006);
        assert!(diag.message.contains("duplicate"));
        assert!(diag.message.contains("function"));
        assert_eq!(diag.labels.len(), 2); // primary + secondary
    }

    #[test]
    fn test_render_warning() {
        let problem = SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: "x".into(),
        };

        let diag = problem.render();

        assert!(!diag.is_error());
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn test_render_all() {
        let problems = vec![
            Problem::Parse(ParseProblem::UnexpectedToken {
                span: Span::new(0, 5),
                expected: "expression".into(),
                found: "}".into(),
            }),
            Problem::Type(TypeProblem::TypeMismatch {
                span: Span::new(10, 15),
                expected: "int".into(),
                found: "str".into(),
            }),
        ];

        let diagnostics = render_all(&problems);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, ErrorCode::E1001);
        assert_eq!(diagnostics[1].code, ErrorCode::E2001);
    }

    #[test]
    fn test_report() {
        let mut report = Report::new();

        report.add_problem(&Problem::Parse(ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        }));

        report.add_problem(&Problem::Semantic(SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: "x".into(),
        }));

        assert_eq!(report.len(), 2);
        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
    }
}
