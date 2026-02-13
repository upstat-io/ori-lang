//! Type error rendering.
//!
//! Converts `TypeCheckError` (from `ori_types`) into rich `Diagnostic`
//! (from `ori_diagnostic`) for terminal output. This is the bridge between
//! the type checker's internal error representation and the user-facing
//! diagnostic system.
//!
//! # Why This Exists
//!
//! `TypeCheckError::message()` can only render primitive type names
//! (e.g., `int`, `str`). Complex types like `[int]` or `(str, bool) -> float`
//! are shown as `<type>` because `message()` lacks access to the `Pool`.
//! Similarly, interned `Name` values need a `StringInterner` to resolve.
//!
//! This renderer has access to both `Pool` and `StringInterner`, producing
//! full type names, context labels, notes, and suggestions.
//!
//! # Design
//!
//! Follows the pattern established by `reporting/parse.rs` and
//! `reporting/semantic.rs`, but works directly with `TypeCheckError`
//! rather than the `Problem` / `Render` trait chain.

use ori_diagnostic::{Diagnostic, Suggestion};
use ori_types::{ErrorContext, Idx, Pool, TypeCheckError, TypeErrorKind, TypeProblem};

use crate::ir::{Name, StringInterner};

/// Renders `TypeCheckError` values as rich `Diagnostic` messages.
///
/// Requires access to both a `Pool` (for formatting complex types via
/// `Pool::format_type()`) and a `StringInterner` (for resolving interned
/// `Name` values to strings).
pub struct TypeErrorRenderer<'a> {
    pool: &'a Pool,
    interner: &'a StringInterner,
}

impl<'a> TypeErrorRenderer<'a> {
    /// Create a new renderer with the given `Pool` and `StringInterner`.
    pub fn new(pool: &'a Pool, interner: &'a StringInterner) -> Self {
        Self { pool, interner }
    }

    /// Render a `TypeCheckError` into a rich `Diagnostic`.
    #[cold]
    pub fn render(&self, error: &TypeCheckError) -> Diagnostic {
        let code = error.code();
        let message = self.format_message(error);
        let primary_label = self.primary_label_text(error);

        let mut diag = Diagnostic::error(code)
            .with_message(message)
            .with_label(error.span, primary_label);

        // Add context information
        Self::add_context(&mut diag, &error.context);

        // Add suggestions
        Self::add_suggestions(&mut diag, &error.suggestions);

        diag
    }

    /// Format a type index into a human-readable string.
    fn format_type(&self, idx: Idx) -> String {
        self.pool.format_type_resolved(idx, self.interner)
    }

    /// Look up an interned Name.
    fn format_name(&self, name: Name) -> String {
        self.interner.lookup(name).to_string()
    }

    /// Build the main error message with full type names.
    ///
    /// Delegates to `TypeCheckError::format_message_rich()` which contains
    /// the canonical rendering logic. This avoids duplicating the message
    /// formatting between `ori_types` and `oric`.
    fn format_message(&self, error: &TypeCheckError) -> String {
        error.format_message_rich(&|idx| self.format_type(idx), &|name| self.format_name(name))
    }

    /// Build the primary label text for the error location.
    fn primary_label_text(&self, error: &TypeCheckError) -> String {
        match &error.kind {
            TypeErrorKind::Mismatch {
                expected,
                found,
                problems,
            } => {
                // Check for problem-specific label text first
                for problem in problems {
                    if let Some(label) = self.problem_label(problem) {
                        return label;
                    }
                }
                format!(
                    "expected `{}`, found `{}`",
                    self.format_type(*expected),
                    self.format_type(*found)
                )
            }
            TypeErrorKind::UnknownIdent { .. } => "not found in this scope".to_string(),
            TypeErrorKind::UndefinedField { field, .. } => {
                format!("unknown field `{}`", self.format_name(*field))
            }
            TypeErrorKind::ArityMismatch {
                expected, found, ..
            } => {
                format!("expected {expected}, found {found}")
            }
            TypeErrorKind::MissingCapability { required, .. } => {
                format!("requires `{}`", self.format_name(*required))
            }
            TypeErrorKind::InfiniteType { .. } => "creates infinite type".to_string(),
            TypeErrorKind::AmbiguousType { .. } => "type cannot be determined".to_string(),
            TypeErrorKind::PatternMismatch { expected, .. } => {
                format!("expected `{}`", self.format_type(*expected))
            }
            TypeErrorKind::NonExhaustiveMatch { .. } => "non-exhaustive patterns".to_string(),
            TypeErrorKind::RigidMismatch { name, .. } => {
                format!("cannot unify `{}`", self.format_name(*name))
            }
            TypeErrorKind::ImportError { .. } => "import failed".to_string(),
            TypeErrorKind::MissingAssocType { assoc_name, .. } => {
                format!("missing `{}`", self.format_name(*assoc_name))
            }
            TypeErrorKind::UnsatisfiedBound { .. } => "bound not satisfied".to_string(),
            TypeErrorKind::NotAStruct { name } => {
                format!("`{}` is not a struct", self.format_name(*name))
            }
            TypeErrorKind::MissingFields { fields, .. } => {
                let names: Vec<_> = fields
                    .iter()
                    .map(|f| format!("`{}`", self.format_name(*f)))
                    .collect();
                format!("missing {}", names.join(", "))
            }
            TypeErrorKind::DuplicateField { field, .. } => {
                format!("duplicate `{}`", self.format_name(*field))
            }
            TypeErrorKind::UninhabitedStructField { field, .. } => {
                format!("`{}`: uninhabited type", self.format_name(*field))
            }
        }
    }

    /// Generate a specific primary label for a `TypeProblem`.
    ///
    /// When a problem provides a specific label, it replaces the generic
    /// `"expected X, found Y"` text — especially important when types are
    /// `<error>` (inference failures where raw type names are meaningless).
    fn problem_label(&self, problem: &TypeProblem) -> Option<String> {
        match problem {
            TypeProblem::ClosureSelfCapture => Some("self-referential closure".to_string()),
            TypeProblem::NotCallable { actual_type } => Some(format!(
                "`{}` is not callable",
                self.format_type(*actual_type)
            )),
            TypeProblem::BadOperandType {
                op,
                op_category,
                found_type,
                required_type,
            } => {
                if *op_category == "unary" {
                    Some(format!("cannot apply `{op}` to `{found_type}`"))
                } else {
                    Some(format!(
                        "`{found_type}` is not a valid `{required_type}` operand"
                    ))
                }
            }
            TypeProblem::ReturnMismatch { expected, found }
            | TypeProblem::ArgumentMismatch {
                expected, found, ..
            } => Some(format!(
                "expected `{}`, found `{}`",
                self.format_type(*expected),
                self.format_type(*found)
            )),
            _ => None,
        }
    }

    /// Add context notes to the diagnostic.
    fn add_context(diag: &mut Diagnostic, context: &ErrorContext) {
        // Add context description as a note
        if let Some(desc) = context.describe() {
            diag.notes.push(desc);
        }

        // Add expectation reason as a note
        if let Some(reason) = context.expectation_reason() {
            diag.notes.push(reason);
        }

        // Add explicit notes from context
        diag.notes.extend(context.notes.iter().cloned());
    }

    /// Add suggestions to the diagnostic.
    ///
    /// Text-only suggestions (no substitutions) go to `diag.suggestions` as
    /// plain strings. Span-bearing suggestions go to `diag.structured_suggestions`.
    fn add_suggestions(diag: &mut Diagnostic, suggestions: &[Suggestion]) {
        for suggestion in suggestions {
            if suggestion.is_text_only() {
                diag.suggestions.push(suggestion.message.clone());
            } else {
                diag.structured_suggestions.push(suggestion.clone());
            }
        }
    }
}

/// Render type errors from a `TypeCheckResult` using the given Pool and interner.
///
/// Convenience function that creates a `TypeErrorRenderer` and maps all errors
/// to diagnostics.
#[cold]
pub fn render_type_errors(
    errors: &[TypeCheckError],
    pool: &Pool,
    interner: &StringInterner,
) -> Vec<Diagnostic> {
    let renderer = TypeErrorRenderer::new(pool, interner);
    errors.iter().map(|e| renderer.render(e)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Name, Span};
    use ori_diagnostic::ErrorCode;
    use ori_types::{ArityMismatchKind, ContextKind, TypeProblem};

    /// Create a test `Pool` and `StringInterner`.
    fn test_env() -> (Pool, StringInterner) {
        (Pool::new(), StringInterner::new())
    }

    #[test]
    fn mismatch_with_primitives() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let error = TypeCheckError::mismatch(
            Span::new(0, 10),
            Idx::INT,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        );

        let diag = renderer.render(&error);
        assert_eq!(diag.code, ErrorCode::E2001);
        assert!(
            diag.message.contains("int"),
            "message should contain 'int': {}",
            diag.message
        );
        assert!(
            diag.message.contains("str"),
            "message should contain 'str': {}",
            diag.message
        );
    }

    #[test]
    fn mismatch_with_complex_types() {
        let (mut pool, interner) = test_env();

        // Create a [int] list type in the Pool
        let list_int = pool.list(Idx::INT);

        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let error = TypeCheckError::mismatch(
            Span::new(5, 15),
            list_int,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        );

        let diag = renderer.render(&error);
        // Should show "[int]" not "<type>"
        assert!(
            diag.message.contains("[int]"),
            "message should contain '[int]' not '<type>': {}",
            diag.message
        );
        assert!(diag.message.contains("str"));
    }

    #[test]
    fn unknown_ident_shows_name() {
        let (pool, interner) = test_env();
        let name = interner.intern("my_variable");
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let error = TypeCheckError::unknown_ident(Span::new(0, 11), name, vec![]);

        let diag = renderer.render(&error);
        assert_eq!(diag.code, ErrorCode::E2003);
        assert!(
            diag.message.contains("my_variable"),
            "message should contain identifier name: {}",
            diag.message
        );
    }

    #[test]
    fn undefined_field_shows_field_and_type() {
        let (pool, interner) = test_env();
        let field_name = interner.intern("length");
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let error = TypeCheckError::undefined_field(Span::new(0, 10), Idx::INT, field_name, vec![]);

        let diag = renderer.render(&error);
        assert!(
            diag.message.contains("length"),
            "message should contain field name: {}",
            diag.message
        );
        assert!(
            diag.message.contains("int"),
            "message should contain type name: {}",
            diag.message
        );
    }

    #[test]
    fn arity_mismatch_correct_counts() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let error =
            TypeCheckError::arity_mismatch(Span::new(0, 20), 2, 4, ArityMismatchKind::Function);

        let diag = renderer.render(&error);
        assert_eq!(diag.code, ErrorCode::E2004);
        assert!(
            diag.message.contains('2') && diag.message.contains('4'),
            "message should contain expected and found counts: {}",
            diag.message
        );
    }

    #[test]
    fn arity_mismatch_with_func_name() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let error = TypeCheckError::arity_mismatch_named(Span::new(0, 20), "add".to_string(), 2, 3);

        let diag = renderer.render(&error);
        assert!(
            diag.message.contains("add"),
            "message should contain function name: {}",
            diag.message
        );
    }

    #[test]
    fn error_context_produces_notes() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let context =
            ErrorContext::new(ContextKind::IfCondition).with_note("conditions must be bool");

        let error =
            TypeCheckError::mismatch(Span::new(0, 10), Idx::BOOL, Idx::INT, vec![], context);

        let diag = renderer.render(&error);
        assert!(
            !diag.notes.is_empty(),
            "diagnostic should have notes from context"
        );
        // Should contain the context description
        assert!(
            diag.notes.iter().any(|n| n.contains("if expression")),
            "notes should contain context description: {:?}",
            diag.notes
        );
        // Should contain the explicit note
        assert!(
            diag.notes
                .iter()
                .any(|n| n.contains("conditions must be bool")),
            "notes should contain explicit note: {:?}",
            diag.notes
        );
    }

    #[test]
    fn text_suggestions_go_to_suggestions() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let error = TypeCheckError::mismatch(
            Span::new(0, 10),
            Idx::INT,
            Idx::FLOAT,
            vec![ori_types::TypeProblem::IntFloat],
            ErrorContext::default(),
        );

        let diag = renderer.render(&error);
        // IntFloat suggestions are text-only, should appear in suggestions
        assert!(
            !diag.suggestions.is_empty(),
            "text-only suggestions should be in diag.suggestions"
        );
        assert!(
            diag.suggestions.iter().any(|s| s.contains("to_float")),
            "should suggest to_float: {:?}",
            diag.suggestions
        );
    }

    #[test]
    fn span_suggestions_go_to_structured() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        // Create an error with a span-bearing suggestion
        let structured_suggestion = Suggestion::text_with_replacement(
            "replace with correct type",
            0,
            Span::new(5, 10),
            "int",
        );

        let error = TypeCheckError::mismatch(
            Span::new(0, 10),
            Idx::INT,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        )
        .with_suggestion(structured_suggestion);

        let diag = renderer.render(&error);
        assert!(
            !diag.structured_suggestions.is_empty(),
            "span-bearing suggestions should be in diag.structured_suggestions"
        );
    }

    #[test]
    fn error_codes_map_correctly() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        // Mismatch -> E2001
        let mismatch = TypeCheckError::mismatch(
            Span::new(0, 5),
            Idx::INT,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        );
        assert_eq!(renderer.render(&mismatch).code, ErrorCode::E2001);

        // UnknownIdent -> E2003
        let ident = TypeCheckError::unknown_ident(Span::new(0, 5), Name::from_raw(1), vec![]);
        assert_eq!(renderer.render(&ident).code, ErrorCode::E2003);

        // ArityMismatch -> E2004
        let arity =
            TypeCheckError::arity_mismatch(Span::new(0, 5), 2, 3, ArityMismatchKind::Function);
        assert_eq!(renderer.render(&arity).code, ErrorCode::E2004);

        // InfiniteType -> E2008
        let infinite = TypeCheckError::infinite_type(Span::new(0, 5), None);
        assert_eq!(renderer.render(&infinite).code, ErrorCode::E2008);

        // AmbiguousType -> E2005
        let ambiguous = TypeCheckError::ambiguous_type(Span::new(0, 5), 1, "expression".into());
        assert_eq!(renderer.render(&ambiguous).code, ErrorCode::E2005);
    }

    #[test]
    fn render_type_errors_helper() {
        let (pool, interner) = test_env();

        let errors = vec![
            TypeCheckError::mismatch(
                Span::new(0, 5),
                Idx::INT,
                Idx::STR,
                vec![],
                ErrorContext::default(),
            ),
            TypeCheckError::unknown_ident(Span::new(10, 15), interner.intern("foo"), vec![]),
        ];

        let diagnostics = render_type_errors(&errors, &pool, &interner);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, ErrorCode::E2001);
        assert_eq!(diagnostics[1].code, ErrorCode::E2003);
        assert!(diagnostics[1].message.contains("foo"));
    }

    #[test]
    fn closure_self_capture_label_not_error_types() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        // Closure self-capture uses Idx::ERROR for both expected and found
        let error = TypeCheckError::closure_self_capture(Span::new(5, 6));

        let diag = renderer.render(&error);
        assert_eq!(diag.code, ErrorCode::E2001);
        assert!(
            diag.message.contains("closure cannot capture itself"),
            "message: {}",
            diag.message
        );
        // Label should NOT contain "<error>" - it should be problem-specific
        let label_text = &diag.labels[0].message;
        assert!(
            !label_text.contains("<error>"),
            "label should not show raw error types, got: {label_text}"
        );
        assert!(
            label_text.contains("self-referential"),
            "label should describe the problem, got: {label_text}"
        );
    }

    #[test]
    fn bad_operand_label_is_specific() {
        let (pool, interner) = test_env();
        let renderer = TypeErrorRenderer::new(&pool, &interner);

        let error = TypeCheckError::mismatch(
            Span::new(0, 5),
            Idx::INT,
            Idx::FLOAT,
            vec![TypeProblem::BadOperandType {
                op: "-",
                op_category: "unary",
                found_type: "float",
                required_type: "int",
            }],
            ErrorContext::default(),
        );

        let diag = renderer.render(&error);
        let label_text = &diag.labels[0].message;
        assert!(
            label_text.contains("cannot apply"),
            "label should describe the operator problem, got: {label_text}"
        );
    }
}
