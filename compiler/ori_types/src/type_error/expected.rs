//! Expected type tracking for rich error messages.
//!
//! This module tracks WHY we expect each type, enabling error messages like:
//! - "expected `int` because of type annotation on line 5"
//! - "expected `str` to match the first element of this list"
//!
//! # Design
//!
//! Inspired by Elm's `Expected` type from `Reporting/Error/Type.hs`:
//! - Every type expectation carries its origin
//! - Origins distinguish annotations from contextual expectations
//! - Sequence contexts track which previous element set the expectation

use ori_ir::{Name, Span};

use super::ContextKind;
use crate::Idx;

/// A type expectation with its origin context.
///
/// Used during type inference to track not just WHAT type is expected,
/// but WHY we expect it. This enables precise error messages.
#[derive(Clone, Debug)]
pub struct Expected {
    /// The expected type.
    pub ty: Idx,
    /// Why we expect this type.
    pub origin: ExpectedOrigin,
}

impl Expected {
    /// Create an expectation with no specific origin (inference determines).
    #[inline]
    pub fn no_expectation(ty: Idx) -> Self {
        Self {
            ty,
            origin: ExpectedOrigin::NoExpectation,
        }
    }

    /// Create an expectation from a type annotation.
    pub fn from_annotation(ty: Idx, name: Name, span: Span) -> Self {
        Self {
            ty,
            origin: ExpectedOrigin::Annotation { name, span },
        }
    }

    /// Create an expectation from surrounding context.
    pub fn from_context(ty: Idx, span: Span, kind: ContextKind) -> Self {
        Self {
            ty,
            origin: ExpectedOrigin::Context { span, kind },
        }
    }

    /// Create an expectation from a previous element in a sequence.
    pub fn from_previous(
        ty: Idx,
        previous_span: Span,
        current_index: usize,
        sequence_kind: SequenceKind,
    ) -> Self {
        Self {
            ty,
            origin: ExpectedOrigin::PreviousInSequence {
                previous_span,
                current_index,
                sequence_kind,
            },
        }
    }

    /// Check if this has a concrete expectation (not `NoExpectation`).
    #[inline]
    pub fn has_expectation(&self) -> bool {
        !matches!(self.origin, ExpectedOrigin::NoExpectation)
    }
}

/// Why we expect a particular type.
///
/// This enum captures the source of type expectations:
/// - `NoExpectation`: Type is inferred bottom-up with no constraint
/// - `Annotation`: Type comes from an explicit annotation in source
/// - `Context`: Type comes from surrounding code structure
/// - `PreviousInSequence`: Type comes from an earlier element in a homogeneous sequence
#[derive(Clone, Debug)]
pub enum ExpectedOrigin {
    /// No specific expectation - inference determines the type.
    ///
    /// Used when we're inferring a type bottom-up without any
    /// contextual constraint (e.g., the initial type of a let binding
    /// without annotation).
    NoExpectation,

    /// Expected because of an explicit type annotation in source.
    ///
    /// Example: `let x: int = ...` expects `int` for the initializer.
    Annotation {
        /// Name of the annotated binding (for error messages).
        name: Name,
        /// Location of the annotation.
        span: Span,
    },

    /// Expected because of surrounding code context.
    ///
    /// Example: In `if cond then ...`, `cond` is expected to be `bool`
    /// because it's an if-condition.
    Context {
        /// Location that established the context.
        span: Span,
        /// What kind of context.
        kind: ContextKind,
    },

    /// Expected because a previous element in a sequence had this type.
    ///
    /// Example: In `[1, 2, "three"]`, "three" is expected to be `int`
    /// because the first element was `int`.
    PreviousInSequence {
        /// Location of the first/representative element.
        previous_span: Span,
        /// Current element's index (for ordinal in error message).
        current_index: usize,
        /// What kind of sequence.
        sequence_kind: SequenceKind,
    },
}

impl ExpectedOrigin {
    /// Get a human-readable description of why this type was expected.
    pub fn describe(&self) -> String {
        match self {
            Self::NoExpectation => "inferred".to_string(),

            Self::Annotation { .. } => {
                // Note: Name requires StringInterner to display properly.
                // Full formatting is done at diagnostic rendering time.
                "because of type annotation".to_string()
            }

            Self::Context { kind, .. } => {
                format!("because {}", kind.expectation_reason())
            }

            Self::PreviousInSequence {
                current_index,
                sequence_kind,
                ..
            } => {
                let ordinal = ordinal(*current_index + 1);
                match sequence_kind {
                    SequenceKind::ListLiteral => {
                        format!("to match the first element of this list (this is the {ordinal} element)")
                    }
                    SequenceKind::MatchArms => {
                        format!("to match the first arm of this match (this is the {ordinal} arm)")
                    }
                    SequenceKind::IfBranches => {
                        format!("to match the `then` branch (this is the {ordinal} branch)")
                    }
                    SequenceKind::TupleElements => {
                        format!("for the {ordinal} element of this tuple")
                    }
                }
            }
        }
    }

    /// Get the span where the expectation originated, if any.
    pub fn span(&self) -> Option<Span> {
        match self {
            Self::NoExpectation => None,
            Self::Annotation { span, .. } | Self::Context { span, .. } => Some(*span),
            Self::PreviousInSequence { previous_span, .. } => Some(*previous_span),
        }
    }
}

/// What kind of homogeneous sequence set the type expectation.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SequenceKind {
    /// Elements of a list literal: `[a, b, c]`
    ListLiteral,
    /// Arms of a match expression (all must return same type).
    MatchArms,
    /// Branches of an if expression (then/else must match).
    IfBranches,
    /// Elements of a tuple (for display purposes, tuples are heterogeneous).
    TupleElements,
}

impl SequenceKind {
    /// Get a description of this sequence kind.
    pub fn description(&self) -> &'static str {
        match self {
            Self::ListLiteral => "list literal",
            Self::MatchArms => "match arms",
            Self::IfBranches => "if branches",
            Self::TupleElements => "tuple elements",
        }
    }
}

/// Convert a 1-based index to an ordinal string ("1st", "2nd", "3rd", etc.).
fn ordinal(n: usize) -> String {
    let suffix = match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinal_formatting() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(2), "2nd");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(4), "4th");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(13), "13th");
        assert_eq!(ordinal(21), "21st");
        assert_eq!(ordinal(22), "22nd");
        assert_eq!(ordinal(23), "23rd");
        assert_eq!(ordinal(100), "100th");
        assert_eq!(ordinal(101), "101st");
        assert_eq!(ordinal(111), "111th");
    }

    #[test]
    fn expected_no_expectation() {
        let exp = Expected::no_expectation(Idx::INT);
        assert_eq!(exp.ty, Idx::INT);
        assert!(!exp.has_expectation());
    }

    #[test]
    fn expected_from_annotation() {
        let name = Name::from_raw(1);
        let span = Span::new(0, 10);
        let exp = Expected::from_annotation(Idx::STR, name, span);
        assert_eq!(exp.ty, Idx::STR);
        assert!(exp.has_expectation());
        assert!(matches!(exp.origin, ExpectedOrigin::Annotation { .. }));
    }

    #[test]
    fn sequence_kind_descriptions() {
        assert_eq!(SequenceKind::ListLiteral.description(), "list literal");
        assert_eq!(SequenceKind::MatchArms.description(), "match arms");
        assert_eq!(SequenceKind::IfBranches.description(), "if branches");
    }

    #[test]
    fn previous_in_sequence_description() {
        let origin = ExpectedOrigin::PreviousInSequence {
            previous_span: Span::new(0, 5),
            current_index: 2, // 3rd element (0-indexed)
            sequence_kind: SequenceKind::ListLiteral,
        };
        let desc = origin.describe();
        assert!(desc.contains("3rd"));
        assert!(desc.contains("list"));
    }
}
