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

use ori_ir::{Name, Span};

use super::{ContextKind, ExpectedOrigin, Suggestion, TypeProblem};
use crate::Idx;

/// A type checking error with full context.
///
/// This is the comprehensive error type used throughout type checking.
/// It contains all information needed to render a helpful error message.
#[derive(Clone, Debug)]
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
            vec![Suggestion::new(
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
            vec![Suggestion::new("this type has no fields", 1)]
        } else {
            // Try to find a similar field name
            let mut suggestions = Vec::new();
            for &avail in &available {
                // In real implementation, we'd use edit_distance here
                suggestions.push(Suggestion::new(format!("available field: `{avail:?}`"), 2));
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
            vec![Suggestion::new(
                format!("remove {diff} extra argument{s}"),
                0,
            )]
        } else {
            let diff = expected - found;
            let s = if diff == 1 { "" } else { "s" };
            vec![Suggestion::new(
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
            suggestions: vec![Suggestion::new(
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
                Suggestion::new("this creates a self-referential type", 1),
                Suggestion::new(
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
            suggestions: vec![Suggestion::new(
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

    // ========================================================================
    // Convenience constructors for common errors
    // ========================================================================

    /// Create an undefined identifier error.
    pub fn undefined_identifier(name: Name, span: Span) -> Self {
        Self::unknown_ident(span, name, vec![])
    }

    /// Create a "self outside impl" error.
    pub fn self_outside_impl(span: Span) -> Self {
        Self {
            span,
            kind: TypeErrorKind::UnknownIdent {
                name: Name::from_raw(0), // Special "self" name
                similar: vec![],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::new(
                "`self` can only be used inside impl blocks",
                0,
            )],
        }
    }

    /// Create an undefined config reference error.
    pub fn undefined_config(name: Name, span: Span) -> Self {
        Self {
            span,
            kind: TypeErrorKind::UnknownIdent {
                name,
                similar: vec![],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::new(
                format!("config `${name:?}` is not defined in this scope"),
                0,
            )],
        }
    }

    /// Create a "not callable" error.
    pub fn not_callable(span: Span, actual_type: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: Idx::ERROR, // Placeholder
                found: actual_type,
                problems: vec![TypeProblem::NotCallable { actual_type }],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::new("only functions can be called", 0)],
        }
    }

    /// Create a "negation requires numeric" error.
    pub fn negation_requires_numeric(span: Span) -> Self {
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: Idx::INT,
                found: Idx::ERROR,
                problems: vec![],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::new(
                "negation (-) can only be applied to int or float",
                0,
            )],
        }
    }

    /// Create a "pipe requires unary function" error.
    pub fn pipe_requires_unary_function(span: Span) -> Self {
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: Idx::ERROR,
                found: Idx::ERROR,
                problems: vec![],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::new(
                "right side of pipe (|>) must be a function that takes one argument",
                0,
            )],
        }
    }

    /// Create a "coalesce requires option" error.
    pub fn coalesce_requires_option(span: Span) -> Self {
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: Idx::ERROR,
                found: Idx::ERROR,
                problems: vec![TypeProblem::ExpectedOption],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::new("left side of ?? must be an Option", 0)],
        }
    }

    /// Create a "try requires Option or Result" error.
    pub fn try_requires_option_or_result(span: Span, actual_type: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: Idx::ERROR,
                found: actual_type,
                problems: vec![TypeProblem::NeedsUnwrap {
                    inner_type: Idx::ERROR,
                }],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::new(
                "the ? operator can only be used on Option or Result types",
                0,
            )],
        }
    }
}

/// What kind of type error occurred.
#[derive(Clone, Debug)]
pub enum TypeErrorKind {
    /// Type mismatch (expected vs found).
    Mismatch {
        /// Expected type (from context/annotation).
        expected: Idx,
        /// Actual type found.
        found: Idx,
        /// Specific problems identified.
        problems: Vec<TypeProblem>,
    },

    /// Unknown identifier (not found in scope).
    UnknownIdent {
        /// The identifier that wasn't found.
        name: Name,
        /// Similar names that exist in scope.
        similar: Vec<Name>,
    },

    /// Undefined field access.
    UndefinedField {
        /// Type that was accessed.
        ty: Idx,
        /// Field that doesn't exist.
        field: Name,
        /// Fields that do exist.
        available: Vec<Name>,
    },

    /// Wrong number of arguments/elements.
    ArityMismatch {
        /// Expected count.
        expected: usize,
        /// Found count.
        found: usize,
        /// What kind of arity (function, tuple, etc.).
        kind: ArityMismatchKind,
    },

    /// Missing required capability.
    MissingCapability {
        /// Required capability.
        required: Name,
        /// Available capabilities.
        available: Vec<Name>,
    },

    /// Infinite/recursive type (occurs check failure).
    InfiniteType {
        /// Name of the variable involved, if known.
        var_name: Option<Name>,
    },

    /// Type cannot be determined (ambiguous).
    AmbiguousType {
        /// ID of the unresolved variable.
        var_id: u32,
        /// Context description.
        context: String,
    },

    /// Pattern doesn't match scrutinee type.
    PatternMismatch {
        /// Expected type.
        expected: Idx,
        /// Found type.
        found: Idx,
    },

    /// Non-exhaustive pattern match.
    NonExhaustiveMatch {
        /// Missing patterns.
        missing: Vec<String>,
    },

    /// Cannot unify rigid type variable.
    RigidMismatch {
        /// Name of the rigid variable.
        name: Name,
        /// Type it was asked to unify with.
        concrete: Idx,
    },
}

/// What kind of arity mismatch occurred.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ArityMismatchKind {
    /// Function argument count.
    Function,
    /// Tuple element count.
    Tuple,
    /// Type argument count.
    TypeArgs,
    /// Struct field count.
    StructFields,
    /// Pattern element count.
    Pattern,
}

impl ArityMismatchKind {
    /// Get a description of what has wrong arity.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Function => "arguments",
            Self::Tuple => "tuple elements",
            Self::TypeArgs => "type arguments",
            Self::StructFields => "struct fields",
            Self::Pattern => "pattern elements",
        }
    }
}

/// Context information for a type error.
///
/// Tracks WHERE in code the error occurred and WHY we expected a type.
#[derive(Clone, Debug, Default)]
pub struct ErrorContext {
    /// What kind of context we're checking in.
    pub checking: Option<ContextKind>,
    /// Why we expected a particular type.
    pub expected_because: Option<ExpectedOrigin>,
    /// Additional notes to include in the error.
    pub notes: Vec<String>,
}

impl ErrorContext {
    /// Create a new error context.
    pub fn new(checking: ContextKind) -> Self {
        Self {
            checking: Some(checking),
            expected_because: None,
            notes: Vec::new(),
        }
    }

    /// Set why we expected a type.
    #[must_use]
    pub fn with_expected_origin(mut self, origin: ExpectedOrigin) -> Self {
        self.expected_because = Some(origin);
        self
    }

    /// Add a note to the context.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Get a description of the context for error messages.
    pub fn describe(&self) -> Option<String> {
        self.checking.as_ref().map(ContextKind::describe)
    }

    /// Get a description of why the type was expected.
    pub fn expectation_reason(&self) -> Option<String> {
        self.expected_because.as_ref().map(ExpectedOrigin::describe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_mismatch_error() {
        let error = TypeCheckError::mismatch(
            Span::new(0, 10),
            Idx::INT,
            Idx::STR,
            vec![TypeProblem::StringToNumber],
            ErrorContext::default(),
        );

        assert!(matches!(error.kind, TypeErrorKind::Mismatch { .. }));
        assert!(!error.suggestions.is_empty());
    }

    #[test]
    fn create_unknown_ident_error() {
        let error = TypeCheckError::unknown_ident(
            Span::new(0, 5),
            Name::from_raw(1),
            vec![Name::from_raw(2)],
        );

        assert!(matches!(error.kind, TypeErrorKind::UnknownIdent { .. }));
        assert!(!error.suggestions.is_empty());
    }

    #[test]
    fn create_arity_mismatch_error() {
        let error =
            TypeCheckError::arity_mismatch(Span::new(0, 20), 2, 4, ArityMismatchKind::Function);

        assert!(matches!(error.kind, TypeErrorKind::ArityMismatch { .. }));
        assert!(!error.suggestions.is_empty());
        assert!(error.suggestions[0].message.contains("remove"));
    }

    #[test]
    fn error_context() {
        let context = ErrorContext::new(ContextKind::IfCondition)
            .with_note("conditions must evaluate to bool");

        assert!(context.describe().is_some());
        assert!(!context.notes.is_empty());
    }

    #[test]
    fn arity_kind_descriptions() {
        assert_eq!(ArityMismatchKind::Function.description(), "arguments");
        assert_eq!(ArityMismatchKind::Tuple.description(), "tuple elements");
    }
}
