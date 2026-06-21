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

pub use kind::{
    AmbiguousTypeSite, ArityMismatchKind, ErrorContext, ImportErrorKind, NonCollectingLoopKind,
    TypeErrorKind, VoidLoopKind,
};

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
    ///
    /// `site` classifies the expression position so the consumer
    /// (`TypeCheckError::message()`) can dispatch on the `AmbiguousTypeSite`
    /// variant and produce site-specific wording. Callers in
    /// the validator compute the site from the `ExprKind` at the error
    /// position; signature positions (no `ExprKind` in scope) pass
    /// `AmbiguousTypeSite::Expression`.
    pub fn ambiguous_type(span: Span, var_id: u32, site: AmbiguousTypeSite) -> Self {
        Self {
            span,
            kind: TypeErrorKind::AmbiguousType { var_id, site },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add a type annotation to clarify the expected type",
                0,
            )],
        }
    }

    /// Create a conditional partial-move error (E2043).
    ///
    /// Emitted by `validate_partial_move` when a non-Drop owned aggregate's
    /// field is projected on one branch of an `if`/`match` but not on a
    /// sibling branch (or differently across arms). The Phase 5 ARC lowering
    /// invariant (`moved_out_fields[v]` statically computable per-CFG-path)
    /// forbids these patterns; rejecting at typeck keeps Phase 5 emission
    /// trivial.
    pub fn conditional_partial_move(span: Span, aggregate: Name, field: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::ConditionalPartialMove { aggregate, field },
            context: ErrorContext::default(),
            suggestions: vec![
                Suggestion::text(
                    "move the field projection out of the conditional so it runs unconditionally",
                    0,
                ),
                Suggestion::text(
                    "OR mirror the same projection on every branch so the move set is symmetric",
                    1,
                ),
            ],
        }
    }

    /// Create a partial-move-on-Drop-type error (E2048).
    ///
    /// Emitted by `validate_drop_partial_move` when `let $f = v.field` projects
    /// a field of a type implementing `Drop`. Per
    /// `drop-trait-proposal.md §Execution Timing`, the compiler walks owned
    /// fields in reverse declaration order AFTER invoking the user `@drop`
    /// body; a partial move would leave `@drop` observing absent fields.
    ///
    /// The axis is disjoint from E2043 (`conditional_partial_move`): every
    /// `Drop` partial move is forbidden regardless of CFG path.
    pub fn drop_partial_move(span: Span, aggregate: Name, field: Name, type_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::DropPartialMove {
                aggregate,
                field,
                type_name,
            },
            context: ErrorContext::default(),
            suggestions: vec![
                Suggestion::text(
                    "consume the value as a whole via `let owned = v` then access fields on `owned`",
                    0,
                ),
                Suggestion::text(
                    "OR access the field as a read-only operand (e.g., `v.field.length()`) instead of binding it",
                    1,
                ),
                Suggestion::text(
                    "OR match-destructure the value: `match v { T { f1, f2, .. } -> ... }`",
                    2,
                ),
            ],
        }
    }

    /// Create a `Value` + `Drop` mutual-exclusion error (E2049).
    ///
    /// Emitted at TWO non-derived registration surfaces (`register_user_types`
    /// for type declarations carrying the `Value` marker AND a registered
    /// `impl T: Drop`; `register_impl` for `impl T: Drop` blocks when T's
    /// trait set already carries `Value`). The span always points at the
    /// second registration so users see the rejected declaration.
    ///
    /// Per Spec: Annex E §AIMS: `Value` declares inline storage with
    /// bitwise copy and no ARC. The refcount-zero
    /// cleanup path that `@drop` hooks into never fires for `Value`
    /// types; the two markers are mutually exclusive.
    pub fn value_drop_conflict(span: Span, type_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::ValueDropConflict { type_name },
            context: ErrorContext::default(),
            suggestions: vec![
                Suggestion::text(
                    "remove the `Value` marker from the type declaration if `Drop` is required",
                    0,
                ),
                Suggestion::text(
                    "OR remove the `impl T: Drop` block if `Value` semantics are intended (inline, bitwise copy, no cleanup)",
                    1,
                ),
                Suggestion::text(
                    "OR split into two types — one carrying `Value` for inline data, another carrying `Drop` for the resource",
                    2,
                ),
            ],
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
