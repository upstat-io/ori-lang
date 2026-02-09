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
//! - `ParseProblem`: Syntax errors during parsing
//! - `TypeProblem`: Type checking errors
//! - `SemanticProblem`: Semantic analysis errors (name resolution, etc.)
//!
//! Each problem type carries all the data needed to render a helpful error
//! message, including spans, types, and context.
//!
//! # Usage
//!
//! ```text
//! use problem::{Problem, ParseProblem};
//!
//! let problem = Problem::Parse(ParseProblem::UnexpectedToken {
//!     span: token.span,
//!     expected: "expression",
//!     found: token.kind.to_string(),
//! });
//! ```

pub mod lex;
pub mod parse;
pub mod semantic;

#[cfg(feature = "llvm")]
pub mod codegen;
#[cfg(feature = "llvm")]
pub use codegen::{report_codegen_error, CodegenProblem};

pub use lex::LexProblem;
pub use parse::ParseProblem;
pub use semantic::SemanticProblem;

use crate::ir::Span;
use ori_ir::StringInterner;

// HasSpan trait and macros for DRY problem implementations

/// Trait for problem types that have a primary source location span.
pub trait HasSpan {
    fn span(&self) -> Span;
}

/// Generate `HasSpan` implementation for an enum with span fields.
///
/// Groups variants by their span field name to handle exceptions.
/// Most variants use `span`, but some (like `UnclosedDelimiter`) use a different field.
///
/// # Example
///
/// ```text
/// impl_has_span! {
///     ParseProblem {
///         found_span: [UnclosedDelimiter],  // Exception case
///         span: [UnexpectedToken, ExpectedExpression, ...],
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
/// # Note on Type Errors
/// Type checking errors use `TypeCheckError` from `ori_types` directly,
/// rather than being wrapped in this enum. This allows the type checker
/// to use structured error variants while other phases use this unified type.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Problem {
    /// Lex-time problems (tokenization errors, confusables, cross-language habits).
    Lex(LexProblem),

    /// Parse-time problems (syntax errors).
    Parse(ParseProblem),

    /// Semantic analysis problems.
    Semantic(SemanticProblem),
}

impl Problem {
    /// Get the primary span of this problem.
    pub fn span(&self) -> Span {
        match self {
            Problem::Lex(p) => p.span(),
            Problem::Parse(p) => p.span(),
            Problem::Semantic(p) => p.span(),
        }
    }

    /// Convert this problem into a diagnostic.
    ///
    /// The interner is required to look up interned `Name` values.
    pub fn into_diagnostic(&self, interner: &StringInterner) -> crate::diagnostic::Diagnostic {
        match self {
            Problem::Lex(p) => p.into_diagnostic(interner),
            Problem::Parse(p) => p.into_diagnostic(interner),
            Problem::Semantic(p) => p.into_diagnostic(interner),
        }
    }
}

// Generate type predicates using macro
impl_problem_predicates!(Problem {
    Lex => is_lex,
    Parse => is_parse,
    Semantic => is_semantic,
});

// Generate From implementations using macro
impl_from_problem!(LexProblem => Problem::Lex);
impl_from_problem!(ParseProblem => Problem::Parse);
impl_from_problem!(SemanticProblem => Problem::Semantic);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_problem_from_parse() {
        let parse_problem = ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        };

        let problem: Problem = parse_problem.clone().into();

        assert!(problem.is_parse());
        assert!(!problem.is_semantic());
        assert_eq!(problem.span(), Span::new(0, 5));
    }

    #[test]
    fn test_problem_from_semantic() {
        let semantic_problem = SemanticProblem::UnknownIdentifier {
            span: Span::new(20, 25),
            name: "foo".into(),
            similar: None,
        };

        let problem: Problem = semantic_problem.clone().into();

        assert!(!problem.is_parse());
        assert!(problem.is_semantic());
        assert_eq!(problem.span(), Span::new(20, 25));
    }

    #[test]
    fn test_problem_equality() {
        let p1 = Problem::Parse(ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        });

        let p2 = Problem::Parse(ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        });

        let p3 = Problem::Parse(ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "statement".into(),
            found: "}".into(),
        });

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_problem_hash() {
        use std::collections::HashSet;

        let p1 = Problem::Parse(ParseProblem::UnexpectedToken {
            span: Span::new(0, 5),
            expected: "expression".into(),
            found: "}".into(),
        });

        let p2 = p1.clone();
        let p3 = Problem::Semantic(SemanticProblem::UnknownIdentifier {
            span: Span::new(10, 15),
            name: "foo".into(),
            similar: None,
        });

        let mut set = HashSet::new();
        set.insert(p1);
        set.insert(p2); // duplicate
        set.insert(p3);

        assert_eq!(set.len(), 2);
    }
}
