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
//! ```ignore
//! use problem::{Problem, ParseProblem};
//!
//! let problem = Problem::Parse(ParseProblem::UnexpectedToken {
//!     span: token.span,
//!     expected: "expression",
//!     found: token.kind.to_string(),
//! });
//! ```

pub mod parse;
pub mod semantic;
pub mod typecheck;

pub use parse::ParseProblem;
pub use semantic::SemanticProblem;
pub use typecheck::TypeProblem;

use crate::ir::Span;

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
/// ```ignore
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
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Problem {
    /// Parse-time problems (syntax errors).
    Parse(ParseProblem),

    /// Type checking problems.
    Type(TypeProblem),

    /// Semantic analysis problems.
    Semantic(SemanticProblem),
}

impl Problem {
    /// Get the primary span of this problem.
    pub fn span(&self) -> Span {
        match self {
            Problem::Parse(p) => p.span(),
            Problem::Type(p) => p.span(),
            Problem::Semantic(p) => p.span(),
        }
    }
}

// Generate type predicates using macro
impl_problem_predicates!(Problem {
    Parse => is_parse,
    Type => is_type,
    Semantic => is_semantic,
});

// Generate From implementations using macro
impl_from_problem!(ParseProblem => Problem::Parse);
impl_from_problem!(TypeProblem => Problem::Type);
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
        assert!(!problem.is_type());
        assert!(!problem.is_semantic());
        assert_eq!(problem.span(), Span::new(0, 5));
    }

    #[test]
    fn test_problem_from_type() {
        let type_problem = TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "int".into(),
            found: "str".into(),
        };

        let problem: Problem = type_problem.clone().into();

        assert!(!problem.is_parse());
        assert!(problem.is_type());
        assert!(!problem.is_semantic());
        assert_eq!(problem.span(), Span::new(10, 15));
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
        assert!(!problem.is_type());
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
        let p3 = Problem::Type(TypeProblem::TypeMismatch {
            span: Span::new(10, 15),
            expected: "int".into(),
            found: "str".into(),
        });

        let mut set = HashSet::new();
        set.insert(p1);
        set.insert(p2); // duplicate
        set.insert(p3);

        assert_eq!(set.len(), 2);
    }
}
