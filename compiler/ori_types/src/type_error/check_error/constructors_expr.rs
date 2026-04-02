//! Convenience constructors for impl/trait and expression/semantic errors.
//!
//! These factory methods create [`TypeCheckError`] instances for trait implementation
//! conflicts, expression-level errors, and conversion/format errors.

use ori_diagnostic::Suggestion;
use ori_ir::{Name, Span};

use super::kind::{ErrorContext, TypeErrorKind};
use super::TypeCheckError;
use crate::type_error::TypeProblem;
use crate::Idx;

impl TypeCheckError {
    /// Create a "duplicate impl" error (E2010).
    ///
    /// Emitted when `impl Type: Trait` is defined more than once.
    pub fn duplicate_impl(span: Span, first_span: Span, trait_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::DuplicateImpl {
                trait_name,
                first_span,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text("remove the duplicate implementation", 0)],
        }
    }

    /// Create an "overlapping impls" error (E2021).
    ///
    /// Emitted when two impls with equal specificity could both apply.
    pub fn overlapping_impls(span: Span, first_span: Span, trait_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::OverlappingImpls {
                trait_name,
                first_span,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add a where clause or use a more specific type to disambiguate",
                0,
            )],
        }
    }

    /// Create a "conflicting defaults" error (E2022).
    ///
    /// Emitted when multiple super-traits provide different default
    /// implementations for the same method and the impl doesn't override it.
    pub fn conflicting_defaults(span: Span, method: Name, trait_a: Name, trait_b: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::ConflictingDefaults {
                method,
                trait_a,
                trait_b,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "provide an explicit implementation to resolve the conflict",
                0,
            )],
        }
    }

    /// Create an "ambiguous method" error (E2023).
    ///
    /// Emitted when multiple trait impls provide the same method for a type.
    pub fn ambiguous_method(span: Span, method: Name, candidates: Vec<Name>) -> Self {
        Self {
            span,
            kind: TypeErrorKind::AmbiguousMethod { method, candidates },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "use fully-qualified syntax to disambiguate: `TraitName.method(x)`",
                0,
            )],
        }
    }

    /// Create a "not object-safe" error (E2024).
    ///
    /// Emitted when a non-object-safe trait is used as a trait object type.
    pub fn not_object_safe(
        span: Span,
        trait_name: Name,
        violations: Vec<crate::ObjectSafetyViolation>,
    ) -> Self {
        use crate::ObjectSafetyViolation;

        let suggestions: Vec<_> = violations
            .iter()
            .map(|v| match v {
                ObjectSafetyViolation::SelfReturn { .. } => Suggestion::text(
                    "consider using a generic parameter `<T: Trait>` instead of a trait object",
                    1,
                ),
                ObjectSafetyViolation::SelfParam { .. } => Suggestion::text(
                    "consider using a generic parameter to preserve type information",
                    1,
                ),
                ObjectSafetyViolation::GenericMethod { .. } => {
                    Suggestion::text("consider removing the generic parameter from the method", 1)
                }
            })
            .collect();

        Self {
            span,
            kind: TypeErrorKind::NotObjectSafe {
                trait_name,
                violations,
            },
            context: ErrorContext::default(),
            suggestions,
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
            suggestions: vec![Suggestion::text("only functions can be called", 0)],
        }
    }

    /// Create a "bad operand type for unary operator" error.
    ///
    /// Produces messages like "cannot apply `-` to `str`".
    pub fn bad_unary_operand(span: Span, op: &'static str, found_type: Idx) -> Self {
        let found_name = found_type.display_name();
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: Idx::ERROR,
                found: found_type,
                problems: vec![TypeProblem::BadOperandType {
                    op,
                    op_category: "unary",
                    found_type: found_name,
                    required_type: if op == "-" { "int or float" } else { "bool" },
                }],
            },
            context: ErrorContext::default(),
            suggestions: vec![],
        }
    }

    /// Create a "bad operand type for binary operator" error.
    ///
    /// Produces messages like "left operand of bitwise operator must be `int`".
    pub fn bad_binary_operand(
        span: Span,
        op_category: &'static str,
        required_type: &'static str,
        found_type: Idx,
    ) -> Self {
        let found_name = found_type.display_name();
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: Idx::ERROR,
                found: found_type,
                problems: vec![TypeProblem::BadOperandType {
                    op: "",
                    op_category,
                    found_type: found_name,
                    required_type,
                }],
            },
            context: ErrorContext::default(),
            suggestions: vec![],
        }
    }

    /// Create a "closure cannot capture itself" error.
    pub fn closure_self_capture(span: Span) -> Self {
        Self {
            span,
            kind: TypeErrorKind::Mismatch {
                expected: Idx::ERROR,
                found: Idx::ERROR,
                problems: vec![TypeProblem::ClosureSelfCapture],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "use recursion through named functions instead",
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
            suggestions: vec![Suggestion::text(
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
            suggestions: vec![Suggestion::text("left side of ?? must be an Option", 0)],
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
            suggestions: vec![Suggestion::text(
                "the ? operator can only be used on Option or Result types",
                0,
            )],
        }
    }

    /// Create an "invalid format spec" error (E2034).
    ///
    /// Emitted when a format spec in a template string doesn't parse.
    pub fn invalid_format_spec(span: Span, spec: String, reason: String) -> Self {
        Self {
            span,
            kind: TypeErrorKind::InvalidFormatSpec { spec, reason },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "format specs follow: [[fill]align][sign][#][0][width][.precision][type]",
                0,
            )],
        }
    }

    /// Create an "into not implemented" error (E2036).
    ///
    /// Emitted when `.into()` is called on a type that has no `Into`
    /// implementation for the expected target type.
    pub fn into_not_implemented(span: Span, ty: Idx, target: Option<Idx>) -> Self {
        Self {
            span,
            kind: TypeErrorKind::IntoNotImplemented { ty, target },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "implement `Into<T>` for this type, or use a different conversion method",
                0,
            )],
        }
    }

    /// Create an "ambiguous into" error (E2037).
    ///
    /// Emitted when `.into()` is called on a type with multiple `Into`
    /// implementations and the target type cannot be inferred.
    pub fn ambiguous_into(span: Span, ty: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::AmbiguousInto { ty },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add a type annotation to disambiguate: `let x: TargetType = value.into()`",
                0,
            )],
        }
    }

    /// Create a "missing printable" error (E2038).
    ///
    /// Emitted when a value used in string interpolation doesn't implement
    /// the `Printable` trait (required for `to_str()` conversion).
    pub fn missing_printable(span: Span, ty: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::MissingPrintable { ty },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add `#derive(Printable)` to the type, or implement `Printable` manually",
                0,
            )],
        }
    }

    /// Create a "cannot assign to immutable binding" error (E2039).
    ///
    /// Emitted when assigning to a binding declared with `$` prefix (immutable).
    pub fn assign_to_immutable(span: Span, name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::AssignToImmutable { name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "remove the `$` prefix to make this binding mutable, or use a new `let` binding",
                0,
            )],
        }
    }

    /// Create an "unsupported feature" error (E2040).
    ///
    /// Emitted for language features that exist in the grammar but are not yet
    /// implemented (e.g., concurrency primitives like `parallel`, `spawn`).
    pub fn unsupported_feature(span: Span, feature: &'static str) -> Self {
        Self {
            span,
            kind: TypeErrorKind::UnsupportedFeature { feature },
            context: ErrorContext::default(),
            suggestions: vec![],
        }
    }

    /// Create an "invalid #repr attribute" error (E2041).
    ///
    /// Emitted when a `#repr(...)` attribute is malformed, applied to a
    /// non-struct type, or has invalid parameters (e.g., non-power-of-two alignment).
    pub fn invalid_repr_attribute(span: Span, type_name: Name, reason: impl Into<String>) -> Self {
        Self {
            span,
            kind: TypeErrorKind::InvalidReprAttribute {
                type_name,
                reason: reason.into(),
            },
            context: ErrorContext::default(),
            suggestions: vec![],
        }
    }

    /// Create a "format type mismatch" error (E2035).
    ///
    /// Emitted when a format type (e.g., `x`, `b`) is used with an
    /// incompatible expression type.
    pub fn format_type_mismatch(
        span: Span,
        expr_type: Idx,
        format_type: String,
        valid_for: &'static str,
    ) -> Self {
        Self {
            span,
            kind: TypeErrorKind::FormatTypeMismatch {
                expr_type,
                format_type,
                valid_for,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                format!("this format type is only valid for {valid_for} types"),
                0,
            )],
        }
    }
}
