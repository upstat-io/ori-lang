//! Structured Problem Types
//!
//! This module separates problem definitions from rendering. Problems describe
//! what went wrong in a structured way, while rendering converts them to
//! user-facing diagnostics.
//!
//! The 1:1 coupling with [`super::reporting`] is intentional: each problem type
//! has a corresponding renderer. `problem` owns the *data*, `reporting` owns
//! the *presentation*. This separation keeps error descriptions independent of
//! output format while guaranteeing every problem has a rendering.
//!
//! # Design
//!
//! Problems are categorized by compilation phase:
//! - `LexProblem`: Tokenization errors, confusables, cross-language habits
//! - `SemanticProblem`: Semantic analysis errors (name resolution, patterns, etc.)
//!
//! Parse errors are rendered directly by `ori_parse::ParseError::to_queued_diagnostic()`
//! and do not flow through this module.
//!
//! Each problem type carries all the data needed to render a helpful error
//! message, including spans, types, and context.

pub mod eval;
pub mod lex;
pub mod parse;
pub mod semantic;

#[cfg(feature = "llvm")]
pub mod codegen;
#[cfg(feature = "llvm")]
pub use codegen::{emit_codegen_error, report_codegen_error, CodegenProblem};

pub use eval::eval_error_to_diagnostic;
pub use lex::LexProblem;
pub use semantic::SemanticProblem;

use crate::ir::Span;

// HasSpan trait and macros for DRY problem implementations

/// Trait for problem types that have a primary source location span.
pub trait HasSpan {
    fn span(&self) -> Span;
}

/// Generate `HasSpan` implementation for an enum with span fields.
///
/// Groups variants by their span field name to handle exceptions.
/// Most variants use `span`, but some may use a different field.
///
/// # Example
///
/// ```text
/// impl_has_span! {
///     SemanticProblem {
///         span: [UnknownIdentifier, DuplicateDefinition, ...],
///     }
/// }
/// ```
macro_rules! impl_has_span {
    ($enum_name:ident { $( $field:ident : [ $($variant:ident),* $(,)? ] ),* $(,)? }) => {
        impl $crate::problem::HasSpan for $enum_name {
            fn span(&self) -> $crate::ir::Span {
                match self {
                    $( $( $enum_name::$variant { $field, .. } => *$field, )* )*
                }
            }
        }
    };
}

/// Generate `From<T> for Problem` implementation.
macro_rules! impl_from_problem {
    ($source:ty => $variant:path) => {
        impl From<$source> for Problem {
            fn from(p: $source) -> Self {
                $variant(p)
            }
        }
    };
}

/// Generate type predicates for Problem enum.
macro_rules! impl_problem_predicates {
    ($enum_name:ident { $( $variant:ident => $method:ident ),* $(,)? }) => {
        impl $enum_name {
            $(
                #[doc = concat!("Check if this is a ", stringify!($variant), " problem.")]
                pub fn $method(&self) -> bool {
                    matches!(self, $enum_name::$variant(_))
                }
            )*
        }
    };
}

pub(crate) use impl_has_span;

/// Unified problem enum for all compilation phases.
///
/// # Salsa Compatibility
/// Has Clone, Eq, `PartialEq`, Hash, Debug for use in query results.
///
/// # Note on Parse Errors
/// Parse errors use `ori_parse::ParseError::to_queued_diagnostic()` directly
/// and do not flow through this enum. This avoids a dual rendering path.
///
/// # Note on Type Errors
/// Type checking errors use `TypeCheckError` from `ori_types` directly,
/// rather than being wrapped in this enum. This allows the type checker
/// to use structured error variants while other phases use this unified type.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Problem {
    /// Lex-time problems (tokenization errors, confusables, cross-language habits).
    Lex(LexProblem),

    /// Semantic analysis problems.
    Semantic(SemanticProblem),
}

impl Problem {
    /// Get the primary span of this problem.
    pub fn span(&self) -> Span {
        match self {
            Problem::Lex(p) => p.span(),
            Problem::Semantic(p) => p.span(),
        }
    }

    /// Convert this problem into a diagnostic.
    ///
    /// Delegates to the sub-type's `into_diagnostic()` method.
    pub fn into_diagnostic(&self, interner: &ori_ir::StringInterner) -> ori_diagnostic::Diagnostic {
        match self {
            Problem::Lex(p) => p.into_diagnostic(interner),
            Problem::Semantic(p) => p.into_diagnostic(interner),
        }
    }
}

// Generate type predicates using macro
impl_problem_predicates!(Problem {
    Lex => is_lex,
    Semantic => is_semantic,
});

// Generate From implementations using macro
impl_from_problem!(LexProblem => Problem::Lex);
impl_from_problem!(SemanticProblem => Problem::Semantic);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_problem_from_lex() {
        use ori_lexer::lex_error::{LexError, LexErrorContext, LexErrorKind};

        let lex_problem = LexProblem::Error(LexError {
            kind: LexErrorKind::Semicolon,
            span: Span::new(0, 1),
            context: LexErrorContext::default(),
            suggestions: Vec::new(),
        });

        let problem: Problem = lex_problem.clone().into();

        assert!(problem.is_lex());
        assert!(!problem.is_semantic());
        assert_eq!(problem.span(), Span::new(0, 1));
    }

    #[test]
    fn test_problem_from_semantic() {
        let semantic_problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: None,
        };

        let problem: Problem = semantic_problem.clone().into();

        assert!(!problem.is_lex());
        assert!(problem.is_semantic());
        assert_eq!(problem.span(), Span::new(20, 25));
    }

    #[test]
    fn test_problem_equality() {
        let p1 = Problem::Semantic(SemanticProblem::UnknownIdentifier {
            span: Span::new(0, 5),
            name: "foo".into(),
            similar: None,
        });

        let p2 = Problem::Semantic(SemanticProblem::UnknownIdentifier {
            span: Span::new(0, 5),
            name: "foo".into(),
            similar: None,
        });

        let p3 = Problem::Semantic(SemanticProblem::UnknownIdentifier {
            span: Span::new(0, 5),
            name: "bar".into(),
            similar: None,
        });

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_problem_hash() {
        use std::collections::HashSet;

        let p1 = Problem::Semantic(SemanticProblem::UnknownIdentifier {
            span: Span::new(0, 5),
            name: "foo".into(),
            similar: None,
        });

        let p2 = p1.clone();
        let p3 = Problem::Semantic(SemanticProblem::UnusedVariable {
            span: Span::new(10, 15),
            name: "x".into(),
        });

        let mut set = HashSet::new();
        set.insert(p1);
        set.insert(p2); // duplicate
        set.insert(p3);

        assert_eq!(set.len(), 2);
    }
}
