//! Comprehensive type checking error structure.
//!
//! This module defines `TypeCheckError`, the rich error type used throughout
//! type checking. It combines:
//! - Location information (span)
//! - Error kind (what went wrong)
//! - Context (where it happened)
//! - Suggestions (how to fix it)
//!
//! # Design
//!
//! Based on patterns from Elm and Gleam:
//! - Errors carry full context for rendering Elm-quality messages
//! - Context tracks both WHERE and WHY types were expected
//! - Suggestions are generated based on the specific problem
//!
//! # Module Structure
//!
//! - [`kind`]: Error kind enum, arity kinds, error context
//! - [`format`]: Rich message formatting with type/name resolution
//! - [`message`]: Simple messages and error code mapping
//! - [`constructors`]: Type/struct/operator/derive error factories
//! - [`constructors_expr`]: Impl/trait/expression error factories

mod constructors;
mod constructors_expr;
mod format;
pub(crate) mod kind;
mod message;

pub use kind::{ArityMismatchKind, ErrorContext, ImportErrorKind, TypeErrorKind};

use ori_diagnostic::Suggestion;
use ori_ir::{Name, Span};

use super::TypeProblem;
use crate::Idx;

/// A type checking error with full context.
///
/// This is the comprehensive error type used throughout type checking.
/// It contains all information needed to render a helpful error message.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TypeCheckError {
    /// Location in source code where the error occurred.
    pub span: Span,
    /// What kind of type error this is.
    pub kind: TypeErrorKind,
    /// Context information for the error.
    pub context: ErrorContext,
    /// Generated suggestions for fixing the error.
    pub suggestions: Vec<Suggestion>,
}

impl TypeCheckError {
    /// Create a new type mismatch error.
    pub fn mismatch(
        span: Span,
        expected: Idx,
        found: Idx,
        problems: Vec<TypeProblem>,
        context: ErrorContext,
    ) -> Self {
        let suggestions = problems.iter().flat_map(TypeProblem::suggestions).collect();
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected,
                found,
                problems,
            },
            context,
            suggestions,
        }
    }

    /// Create an unknown identifier error.
    pub fn unknown_ident(span: Span, name: Name, similar: Vec<Name>) -> Self {
        let suggestions = if similar.is_empty() {
            vec![Suggestion::text(
                format!("check spelling or add a definition for `{name:?}`"),
                1,
            )]
        } else {
            similar
                .iter()
                .map(|s| Suggestion::did_you_mean(format!("{s:?}")))
                .collect()
        };

        Self {
            span,
            kind: TypeErrorKind::UnknownIdent { name, similar },
            context: ErrorContext::default(),
            suggestions,
        }
    }

    /// Create an undefined field error.
    pub fn undefined_field(span: Span, ty: Idx, field: Name, available: Vec<Name>) -> Self {
        let suggestions = if available.is_empty() {
            vec![Suggestion::text("this type has no fields", 1)]
        } else {
            // Try to find a similar field name
            let mut suggestions = Vec::new();
            for &avail in &available {
                // In real implementation, we'd use edit_distance here
                suggestions.push(Suggestion::text(format!("available field: `{avail:?}`"), 2));
            }
            if suggestions.len() > 5 {
                suggestions.truncate(5);
            }
            suggestions
        };

        Self {
            span,
            kind: TypeErrorKind::UndefinedField {
                ty,
                field,
                available,
            },
            context: ErrorContext::default(),
            suggestions,
        }
    }

    /// Create an arity mismatch error.
    pub fn arity_mismatch(
        span: Span,
        expected: usize,
        found: usize,
        kind: ArityMismatchKind,
    ) -> Self {
        let suggestions = if found > expected {
            let diff = found - expected;
            let s = if diff == 1 { "" } else { "s" };
            vec![Suggestion::text(
                format!("remove {diff} extra argument{s}"),
                0,
            )]
        } else {
            let diff = expected - found;
            let s = if diff == 1 { "" } else { "s" };
            vec![Suggestion::text(
                format!("add {diff} missing argument{s}"),
                0,
            )]
        };

        Self {
            span,
            kind: TypeErrorKind::ArityMismatch {
                expected,
                found,
                kind,
                func_name: None,
            },
            context: ErrorContext::default(),
            suggestions,
        }
    }

    /// Create an arity mismatch error with a function name.
    pub fn arity_mismatch_named(
        span: Span,
        func_name: String,
        expected: usize,
        found: usize,
    ) -> Self {
        let suggestions = if found > expected {
            let diff = found - expected;
            let s = if diff == 1 { "" } else { "s" };
            vec![Suggestion::text(
                format!("remove {diff} extra argument{s}"),
                0,
            )]
        } else {
            let diff = expected - found;
            let s = if diff == 1 { "" } else { "s" };
            vec![Suggestion::text(
                format!("add {diff} missing argument{s}"),
                0,
            )]
        };

        Self {
            span,
            kind: TypeErrorKind::ArityMismatch {
                expected,
                found,
                kind: ArityMismatchKind::Function,
                func_name: Some(func_name),
            },
            context: ErrorContext::default(),
            suggestions,
        }
    }

    /// Create a missing capability error.
    pub fn missing_capability(span: Span, required: Name, available: &[Name]) -> Self {
        Self {
            span,
            kind: TypeErrorKind::MissingCapability {
                required,
                available: available.to_vec(),
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                format!("add `uses {required:?}` to the function signature"),
                0,
            )],
        }
    }

    /// Create an infinite type error.
    pub fn infinite_type(span: Span, var_name: Option<Name>) -> Self {
        Self {
            span,
            kind: TypeErrorKind::InfiniteType { var_name },
            context: ErrorContext::default(),
            suggestions: vec![
                Suggestion::text("this creates a self-referential type", 1),
                Suggestion::text(
                    "use a newtype wrapper to break the cycle: `type Wrapper = { inner: T }`",
                    2,
                ),
            ],
        }
    }

    /// Create an ambiguous type error.
    pub fn ambiguous_type(span: Span, var_id: u32, context_desc: String) -> Self {
        Self {
            span,
            kind: TypeErrorKind::AmbiguousType {
                var_id,
                context: context_desc,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add a type annotation to clarify the expected type",
                0,
            )],
        }
    }

    /// Set the error context.
    #[must_use]
    pub fn with_context(mut self, context: ErrorContext) -> Self {
        self.context = context;
        self
    }

    /// Add a suggestion to the error.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Get the error's source span.
    ///
    /// Convenience method matching V1's interface. The span is also
    /// available as the public `span` field.
    pub fn span(&self) -> Span {
        self.span
    }
}

#[cfg(test)]
mod tests;
