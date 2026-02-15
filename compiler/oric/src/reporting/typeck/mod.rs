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
mod tests;
