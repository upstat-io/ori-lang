//! Diagnostic Rendering
//!
//! Converts structured [`super::problem`] types into user-facing Diagnostic
//! messages. This separates the "what went wrong" (Problem) from "how to
//! display it" (Diagnostic).
//!
//! The 1:1 coupling with `problem` is intentional: each problem variant has a
//! corresponding `Render` implementation here. Adding a new problem type
//! requires adding its renderer in this module.
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

mod lex;
mod parse;
mod semantic;
pub mod typeck;

use crate::diagnostic::queue::{DiagnosticConfig, DiagnosticQueue, DiagnosticSeverity};
use crate::diagnostic::{Diagnostic, Severity};
use crate::problem::Problem;
use ori_ir::StringInterner;

/// Trait for rendering problems into diagnostics.
pub trait Render {
    /// Render this problem into a diagnostic.
    ///
    /// The interner is required to look up interned `Name` values.
    fn render(&self, interner: &StringInterner) -> Diagnostic;
}

impl Render for Problem {
    fn render(&self, interner: &StringInterner) -> Diagnostic {
        match self {
            Problem::Lex(p) => p.render(interner),
            Problem::Parse(p) => p.render(interner),
            Problem::Semantic(p) => p.render(interner),
        }
    }
}

/// Render a collection of problems into diagnostics.
pub fn render_all(problems: &[Problem], interner: &StringInterner) -> Vec<Diagnostic> {
    problems.iter().map(|p| p.render(interner)).collect()
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
        queue.add_with_source_and_severity(diag, source, DiagnosticSeverity::Hard);
    }

    queue.flush()
}

/// A report containing multiple diagnostics.
///
/// Collects diagnostics from compilation phases (parsing, type checking)
/// and provides methods to query error counts and severity levels.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// The diagnostics in this report.
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Creates a new empty report.
    pub fn new() -> Self {
        Report {
            diagnostics: Vec::new(),
        }
    }

    /// Creates a new report with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Report {
            diagnostics: Vec::with_capacity(capacity),
        }
    }

    /// Adds a diagnostic to the report.
    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Adds a problem to the report, rendering it as a diagnostic.
    pub fn add_problem(&mut self, problem: &Problem, interner: &StringInterner) {
        self.diagnostics.push(problem.render(interner));
    }

    /// Returns true if the report contains no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns the total number of diagnostics.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Returns true if any diagnostic is an error.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(ori_diagnostic::Diagnostic::is_error)
    }

    /// Returns the number of error-level diagnostics.
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    /// Returns the number of warning-level diagnostics.
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
    use crate::problem::semantic::DefinitionKind;
    use crate::problem::{ParseProblem, SemanticProblem};

    fn test_interner() -> StringInterner {
        StringInterner::new()
    }

    #[test]
    fn test_render_parse_problem() {
        let interner = test_interner();
        let problem = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        };

        let diag = problem.render(&interner);

        assert_eq!(diag.code, ErrorCode::E1001);
        assert!(diag.message.contains("unexpected token"));
        assert!(diag.message.contains("expression"));
        assert!(diag.message.contains('}'));
        assert!(diag.is_error());
    }

    #[test]
    fn test_render_semantic_error() {
        let interner = test_interner();
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(10, 15),
            name: "unknown_var".into(),
            similar: None,
        };

        let diag = problem.render(&interner);

        assert_eq!(diag.code, ErrorCode::E2003);
        assert!(diag.message.contains("unknown identifier"));
        assert!(diag.message.contains("unknown_var"));
    }

    #[test]
    fn test_render_unknown_identifier_with_suggestion() {
        let interner = test_interner();
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: Some("for".into()),
        };

        let diag = problem.render(&interner);

        assert_eq!(diag.code, ErrorCode::E2003);
        assert!(diag.message.contains("unknown identifier"));
        assert!(diag.suggestions.iter().any(|s| s.contains("for")));
    }

    #[test]
    fn test_render_duplicate_definition() {
        let interner = test_interner();
        let problem = SemanticProblem::DuplicateDefinition {
            span: Span::new(100, 110),
            name: "bar".into(),
            kind: DefinitionKind::Function,
            first_span: Span::new(10, 20),
        };

        let diag = problem.render(&interner);

        assert_eq!(diag.code, ErrorCode::E2006);
        assert!(diag.message.contains("duplicate"));
        assert!(diag.message.contains("function"));
        assert_eq!(diag.labels.len(), 2); // primary + secondary
    }

    #[test]
    fn test_render_warning() {
        let interner = test_interner();
        let problem = SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: "x".into(),
        };

        let diag = problem.render(&interner);

        assert!(!diag.is_error());
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn test_render_all() {
        let interner = test_interner();
        let problems = vec![
            Problem::Parse(ParseProblem::UnexpectedToken {
                span: Span::new(0, 5),
                expected: "expression".into(),
                found: "}".into(),
            }),
            Problem::Semantic(SemanticProblem::UnknownIdentifier {
                span: Span::new(10, 15),
                name: "foo".into(),
                similar: None,
            }),
        ];

        let diagnostics = render_all(&problems, &interner);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, ErrorCode::E1001);
        assert_eq!(diagnostics[1].code, ErrorCode::E2003);
    }

    #[test]
    fn test_report() {
        let interner = test_interner();
        let mut report = Report::new();

        report.add_problem(
            &Problem::Parse(ParseProblem::UnexpectedToken {
                span: Span::new(0, 5),
                expected: "expression".into(),
                found: "}".into(),
            }),
            &interner,
        );

        report.add_problem(
            &Problem::Semantic(SemanticProblem::UnusedVariable {
                span: Span::new(5, 10),
                name: "x".into(),
            }),
            &interner,
        );

        assert_eq!(report.len(), 2);
        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
    }
}
