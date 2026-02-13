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
    use crate::ir::{Span, StringInterner};
    use crate::problem::semantic::DefinitionKind;
    use crate::problem::SemanticProblem;

    fn test_interner() -> StringInterner {
        StringInterner::new()
    }

    #[test]
    fn test_semantic_error_diagnostic() {
        let interner = test_interner();
        let name = interner.intern("unknown_var");
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(10, 15),
            name,
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
        let foo = interner.intern("foo");
        let for_kw = interner.intern("for");
        let problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: foo,
            similar: Some(for_kw),
        };

        let diag = problem.into_diagnostic(&interner);

        assert_eq!(diag.code, ErrorCode::E2003);
        assert!(diag.message.contains("unknown identifier"));
        assert!(diag.suggestions.iter().any(|s| s.contains("for")));
    }

    #[test]
    fn test_duplicate_definition_diagnostic() {
        let interner = test_interner();
        let bar = interner.intern("bar");
        let problem = SemanticProblem::DuplicateDefinition {
            span: Span::new(100, 110),
            name: bar,
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
        let x = interner.intern("x");
        let problem = SemanticProblem::UnusedVariable {
            span: Span::new(5, 10),
            name: x,
        };

        let diag = problem.into_diagnostic(&interner);

        assert!(!diag.is_error());
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn test_semantic_problems_into_diagnostics() {
        let interner = test_interner();
        let foo = interner.intern("foo");
        let my_func = interner.intern("my_func");

        let diag1 = SemanticProblem::UnknownIdentifier {
            span: Span::new(10, 15),
            name: foo,
            similar: None,
        }
        .into_diagnostic(&interner);

        let diag2 = SemanticProblem::MissingTest {
            span: Span::new(20, 30),
            func_name: my_func,
        }
        .into_diagnostic(&interner);

        assert_eq!(diag1.code, ErrorCode::E2003);
        assert_eq!(diag2.code, ErrorCode::E3001);
    }
}
