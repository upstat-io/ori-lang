//! Diagnostic Rendering
//!
//! Converts structured [`super::problem`] types into user-facing Diagnostic
//! messages. This separates the "what went wrong" (Problem) from "how to
//! display it" (Diagnostic).
//!
//! # Design
//!
//! Each problem type has an `into_diagnostic()` method that converts it to
//! a `Diagnostic` with:
//! - Error code for searchability
//! - Clear message explaining what went wrong
//! - Labeled spans showing where
//! - Notes providing context
//! - Suggestions for how to fix
//!
//! Type errors use `TypeErrorRenderer` for Pool-aware type name rendering.
//!
//! Parse errors are rendered directly by `ori_parse::ParseError::to_queued_diagnostic()`
//! and do not flow through this module.

pub mod typeck;

#[cfg(test)]
mod tests {
    use crate::diagnostic::ErrorCode;
    use crate::diagnostic::Severity;
    use crate::ir::Span;
    use crate::problem::semantic::DefinitionKind;
    use crate::problem::{Problem, SemanticProblem};
    use ori_ir::StringInterner;

    fn test_interner() -> StringInterner {
        StringInterner::new()
    }

    #[test]
    fn test_semantic_error_diagnostic() {
        let interner = test_interner();
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(10, 15),
            name: "unknown_var".into(),
            similar: None,
        };

        let diag = problem.into_diagnostic(&interner);

        assert_eq!(diag.code, ErrorCode::E2003);
        assert!(diag.message.contains("unknown identifier"));
        assert!(diag.message.contains("unknown_var"));
    }

    #[test]
    fn test_unknown_identifier_with_suggestion() {
        let interner = test_interner();
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: Some("for".into()),
        };

        let diag = problem.into_diagnostic(&interner);

        assert_eq!(diag.code, ErrorCode::E2003);
        assert!(diag.message.contains("unknown identifier"));
        assert!(diag.suggestions.iter().any(|s| s.contains("for")));
    }

    #[test]
    fn test_duplicate_definition_diagnostic() {
        let interner = test_interner();
        let problem = SemanticProblem::DuplicateDefinition {
            span: Span::new(100, 110),
            name: "bar".into(),
            kind: DefinitionKind::Function,
            first_span: Span::new(10, 20),
        };

        let diag = problem.into_diagnostic(&interner);

        assert_eq!(diag.code, ErrorCode::E2006);
        assert!(diag.message.contains("duplicate"));
        assert!(diag.message.contains("function"));
        assert_eq!(diag.labels.len(), 2); // primary + secondary
    }

    #[test]
    fn test_warning_diagnostic() {
        let interner = test_interner();
        let problem = SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: "x".into(),
        };

        let diag = problem.into_diagnostic(&interner);

        assert!(!diag.is_error());
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn test_problem_into_diagnostic() {
        let interner = test_interner();
        let problems = [
            Problem::Semantic(SemanticProblem::UnknownIdentifier {
                span: Span::new(10, 15),
                name: "foo".into(),
                similar: None,
            }),
            Problem::Semantic(SemanticProblem::MissingTest {
                span: Span::new(20, 30),
                func_name: "my_func".into(),
            }),
        ];

        let diagnostics: Vec<_> = problems
            .iter()
            .map(|p| p.into_diagnostic(&interner))
            .collect();

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, ErrorCode::E2003);
        assert_eq!(diagnostics[1].code, ErrorCode::E3001);
    }
}
