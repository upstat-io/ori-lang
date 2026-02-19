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

use ori_diagnostic::ErrorCode;
use ori_ir::{Name, Span};

use ori_diagnostic::Suggestion;

use super::{ContextKind, ExpectedOrigin, TypeProblem};
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

    // ========================================================================
    // Query methods (for test runner, error matching, diagnostics)
    // ========================================================================

    /// Get the error's source span.
    ///
    /// Convenience method matching V1's interface. The span is also
    /// available as the public `span` field.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Format a rich error message using closures for type and name resolution.
    ///
    /// This produces the same output as `TypeErrorRenderer::format_message()` in `oric`,
    /// but is available at the `ori_types` level for consumers like the WASM playground
    /// that can't depend on `oric`.
    ///
    /// # Parameters
    ///
    /// - `format_type`: Resolves a type `Idx` to a human-readable string
    ///   (e.g., `|idx| pool.format_type(idx)`)
    /// - `format_name`: Resolves an interned `Name` to its string value
    ///   (e.g., `|name| interner.lookup(name).to_string()`)
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive TypeErrorKind → rich message dispatch"
    )]
    pub fn format_message_rich(
        &self,
        format_type: &dyn Fn(Idx) -> String,
        format_name: &dyn Fn(Name) -> String,
    ) -> String {
        use std::fmt::Write;
        match &self.kind {
            TypeErrorKind::Mismatch {
                expected,
                found,
                problems,
            } => {
                for problem in problems {
                    if let Some(detail) = problem_message_rich(problem, format_type) {
                        return format!("type mismatch: {detail}");
                    }
                }
                format!(
                    "type mismatch: expected `{}`, found `{}`",
                    format_type(*expected),
                    format_type(*found)
                )
            }
            TypeErrorKind::UnknownIdent { name, similar } => {
                let mut msg = format!("unknown identifier `{}`", format_name(*name));
                if !similar.is_empty() {
                    let suggestions: Vec<String> = similar
                        .iter()
                        .map(|s| format!("`{}`", format_name(*s)))
                        .collect();
                    write!(msg, "; did you mean {}?", suggestions.join(" or ")).ok();
                }
                msg
            }
            TypeErrorKind::UndefinedField { ty, field, .. } => {
                format!(
                    "no such field `{}` on type `{}`",
                    format_name(*field),
                    format_type(*ty)
                )
            }
            TypeErrorKind::ArityMismatch {
                expected,
                found,
                kind,
                func_name,
            } => {
                if let Some(name) = func_name {
                    let s = if *expected == 1 { "" } else { "s" };
                    format!(
                        "function `{name}` expects {expected} argument{s}, but {found} {} provided",
                        if *found == 1 { "was" } else { "were" }
                    )
                } else {
                    let desc = kind.description();
                    format!("expected {expected} {desc}, found {found}")
                }
            }
            TypeErrorKind::MissingCapability { required, .. } => {
                format!("missing required capability `{}`", format_name(*required))
            }
            TypeErrorKind::InfiniteType { var_name } => {
                if let Some(name) = var_name {
                    format!(
                        "infinite type detected: `{}` refers to itself",
                        format_name(*name)
                    )
                } else {
                    "infinite type detected".to_string()
                }
            }
            TypeErrorKind::AmbiguousType { context, .. } => {
                format!("cannot infer type in {context}")
            }
            TypeErrorKind::PatternMismatch { expected, found } => {
                format!(
                    "pattern type mismatch: expected `{}`, found `{}`",
                    format_type(*expected),
                    format_type(*found)
                )
            }
            TypeErrorKind::NonExhaustiveMatch { missing } => {
                format!("non-exhaustive match: missing {}", missing.join(", "))
            }
            TypeErrorKind::RigidMismatch { name, concrete } => {
                format!(
                    "type parameter `{}` cannot be unified with `{}`",
                    format_name(*name),
                    format_type(*concrete)
                )
            }
            TypeErrorKind::ImportError { message, .. } => {
                format!("import error: {message}")
            }
            TypeErrorKind::MissingAssocType {
                assoc_name,
                trait_name,
            } => {
                format!(
                    "missing associated type `{}` in impl for `{}`",
                    format_name(*assoc_name),
                    format_name(*trait_name)
                )
            }
            TypeErrorKind::UnsatisfiedBound { message } => message.clone(),
            TypeErrorKind::NotAStruct { name } => {
                format!("`{}` is not a struct type", format_name(*name))
            }
            TypeErrorKind::MissingFields {
                struct_name,
                fields,
            } => {
                let field_names: Vec<_> = fields
                    .iter()
                    .map(|f| format!("`{}`", format_name(*f)))
                    .collect();
                let count = fields.len();
                let s = if count == 1 { "" } else { "s" };
                format!(
                    "missing {count} required field{s} in `{}`: {}",
                    format_name(*struct_name),
                    field_names.join(", ")
                )
            }
            TypeErrorKind::DuplicateField { struct_name, field } => {
                format!(
                    "duplicate field `{}` in `{}`",
                    format_name(*field),
                    format_name(*struct_name)
                )
            }
            TypeErrorKind::UninhabitedStructField { struct_name, field } => {
                format!(
                    "cannot use `Never` as struct field type: field `{}` in `{}`",
                    format_name(*field),
                    format_name(*struct_name)
                )
            }
            TypeErrorKind::UnsupportedOperator { ty, op, trait_name } => {
                let type_name = format_type(*ty);
                format!(
                    "cannot apply operator `{op}` to type `{type_name}`; implement `{trait_name}` trait"
                )
            }
            TypeErrorKind::DuplicateImpl { trait_name, .. } => {
                format!(
                    "duplicate implementation of `{}` for this type",
                    format_name(*trait_name)
                )
            }
            TypeErrorKind::OverlappingImpls { trait_name, .. } => {
                format!(
                    "overlapping implementations of `{}` with equal specificity",
                    format_name(*trait_name)
                )
            }
            TypeErrorKind::ConflictingDefaults {
                method,
                trait_a,
                trait_b,
            } => {
                format!(
                    "conflicting default for `{}`: provided by both `{}` and `{}`",
                    format_name(*method),
                    format_name(*trait_a),
                    format_name(*trait_b)
                )
            }
            TypeErrorKind::AmbiguousMethod {
                method, candidates, ..
            } => {
                let names: Vec<String> = candidates
                    .iter()
                    .map(|n| format!("`{}`", format_name(*n)))
                    .collect();
                format!(
                    "ambiguous method `{}`: provided by {}",
                    format_name(*method),
                    names.join(" and ")
                )
            }
            TypeErrorKind::NotObjectSafe {
                trait_name,
                violations,
            } => {
                use crate::ObjectSafetyViolation;
                let reasons: Vec<String> = violations
                    .iter()
                    .map(|v| match v {
                        ObjectSafetyViolation::SelfReturn { method, .. } => {
                            format!("method `{}` returns `Self`", format_name(*method))
                        }
                        ObjectSafetyViolation::SelfParam { method, param, .. } => {
                            format!(
                                "method `{}` has `Self` in parameter `{}`",
                                format_name(*method),
                                format_name(*param)
                            )
                        }
                        ObjectSafetyViolation::GenericMethod { method, .. } => {
                            format!(
                                "method `{}` has generic type parameters",
                                format_name(*method)
                            )
                        }
                    })
                    .collect();
                format!(
                    "trait `{}` cannot be made into an object: {}",
                    format_name(*trait_name),
                    reasons.join("; ")
                )
            }
            TypeErrorKind::NotIndexable { ty } => {
                format!(
                    "type `{}` does not support indexing; implement `Index` trait",
                    format_type(*ty)
                )
            }
            TypeErrorKind::IndexKeyMismatch {
                ty,
                expected_key,
                found_key,
            } => {
                format!(
                    "wrong index key type for `{}`: expected `{}`, found `{}`",
                    format_type(*ty),
                    format_type(*expected_key),
                    format_type(*found_key)
                )
            }
            TypeErrorKind::AmbiguousIndex { ty } => {
                format!(
                    "ambiguous index: type `{}` has multiple `Index` implementations",
                    format_type(*ty)
                )
            }
            TypeErrorKind::CannotDeriveDefaultForSumType { type_name } => {
                format!(
                    "cannot derive `Default` for sum type `{}`",
                    format_name(*type_name)
                )
            }
            TypeErrorKind::CannotDeriveHashableWithoutEq { type_name } => {
                format!(
                    "cannot derive `Hashable` without `Eq` for type `{}`",
                    format_name(*type_name)
                )
            }
            TypeErrorKind::HashInvariantViolation { type_name } => {
                format!(
                    "`Hashable` implementation for `{}` may violate hash invariant",
                    format_name(*type_name)
                )
            }
            TypeErrorKind::NonHashableMapKey { key_type } => {
                format!(
                    "`{}` cannot be used as map key (missing `Hashable`)",
                    format_type(*key_type)
                )
            }
            TypeErrorKind::FieldMissingTraitInDerive {
                type_name,
                trait_name,
                field_name,
                field_type,
            } => {
                format!(
                    "cannot derive `{}` for `{}`: field `{}` of type `{}` does not implement `{}`",
                    format_name(*trait_name),
                    format_name(*type_name),
                    format_name(*field_name),
                    format_type(*field_type),
                    format_name(*trait_name),
                )
            }
            TypeErrorKind::TraitNotDerivable { trait_name } => {
                format!("trait `{}` cannot be derived", format_name(*trait_name))
            }
            TypeErrorKind::InvalidFormatSpec { spec, reason } => {
                format!("invalid format specification `{spec}`: {reason}")
            }
            TypeErrorKind::FormatTypeMismatch {
                expr_type,
                format_type: fmt_ty,
                valid_for,
            } => {
                format!(
                    "format type `{fmt_ty}` not supported for `{}`; valid for {valid_for}",
                    format_type(*expr_type)
                )
            }
            TypeErrorKind::IntoNotImplemented { ty, target } => {
                if let Some(t) = target {
                    format!(
                        "type `{}` does not implement `Into<{}>`",
                        format_type(*ty),
                        format_type(*t)
                    )
                } else {
                    format!(
                        "type `{}` does not implement `Into` for any target type",
                        format_type(*ty)
                    )
                }
            }
            TypeErrorKind::AmbiguousInto { ty } => {
                format!(
                    "ambiguous `.into()` call on `{}`: multiple `Into` implementations apply",
                    format_type(*ty)
                )
            }
            TypeErrorKind::MissingPrintable { ty } => {
                format!(
                    "`{}` does not implement `Printable` (cannot be used in string interpolation)",
                    format_type(*ty)
                )
            }
        }
    }

    /// Convenience wrapper for `format_message_rich` using a `Pool` and `StringInterner`.
    ///
    /// This is the easiest way to get rich error messages when you have both
    /// a Pool (for type formatting) and a `StringInterner` (for name resolution).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (result, pool) = check_module_with_imports(&module, &arena, &interner, |_| {});
    /// for error in result.errors() {
    ///     println!("{}", error.format_with(&pool, &interner));
    /// }
    /// ```
    pub fn format_with(&self, pool: &crate::Pool, interner: &ori_ir::StringInterner) -> String {
        self.format_message_rich(&|idx| pool.format_type_resolved(idx, interner), &|name| {
            interner.lookup(name).to_string()
        })
    }

    /// Get a human-readable error message.
    ///
    /// Uses `Idx::display_name()` for type names, which renders primitives
    /// (int, bool, str, etc.) and falls back to `"<type>"` for complex types
    /// that would need a Pool to render fully.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive TypeErrorKind message dispatch"
    )]
    pub fn message(&self) -> String {
        match &self.kind {
            TypeErrorKind::Mismatch {
                expected,
                found,
                problems,
            } => {
                // Check for specific problem-based messages first.
                // Prefix with "type mismatch: " for categorization and
                // backward compatibility with compile_fail expectations.
                for problem in problems {
                    if let Some(detail) = problem_message(problem) {
                        return format!("type mismatch: {detail}");
                    }
                }
                format!(
                    "type mismatch: expected {}, found {}",
                    expected.display_name(),
                    found.display_name()
                )
            }
            TypeErrorKind::UnknownIdent { .. } => {
                // Name is an interned ID — cannot resolve to string without
                // an interner. Callers with interner access should render
                // the full message (e.g., "unknown identifier `foo`").
                "unknown identifier".to_string()
            }
            TypeErrorKind::UndefinedField { ty, .. } => {
                format!("no such field on type {}", ty.display_name())
            }
            TypeErrorKind::ArityMismatch {
                expected,
                found,
                kind,
                func_name,
            } => {
                if let Some(name) = func_name {
                    let s = if *expected == 1 { "" } else { "s" };
                    format!(
                        "function `{name}` expects {expected} argument{s}, but {found} {} provided",
                        if *found == 1 { "was" } else { "were" }
                    )
                } else {
                    let desc = kind.description();
                    format!("expected {expected} {desc}, found {found}")
                }
            }
            TypeErrorKind::MissingCapability { .. } => "missing required capability".to_string(),
            TypeErrorKind::InfiniteType { .. } => "infinite type detected".to_string(),
            TypeErrorKind::AmbiguousType { context, .. } => {
                format!("cannot infer type in {context}")
            }
            TypeErrorKind::PatternMismatch { expected, found } => {
                format!(
                    "pattern type mismatch: expected {}, found {}",
                    expected.display_name(),
                    found.display_name()
                )
            }
            TypeErrorKind::NonExhaustiveMatch { missing } => {
                format!("non-exhaustive match: missing {}", missing.join(", "))
            }
            TypeErrorKind::RigidMismatch { concrete, .. } => {
                format!(
                    "type parameter cannot be unified with {}",
                    concrete.display_name()
                )
            }
            TypeErrorKind::ImportError { message, .. } => {
                format!("import error: {message}")
            }
            TypeErrorKind::MissingAssocType { .. } => {
                "missing associated type in impl block".to_string()
            }
            TypeErrorKind::UnsatisfiedBound { message } => message.clone(),
            TypeErrorKind::NotAStruct { .. } => "not a struct type".to_string(),
            TypeErrorKind::MissingFields { fields, .. } => {
                let count = fields.len();
                let s = if count == 1 { "" } else { "s" };
                format!("missing {count} required field{s} in struct literal")
            }
            TypeErrorKind::DuplicateField { .. } => "duplicate field in struct literal".to_string(),
            TypeErrorKind::UninhabitedStructField { .. } => {
                "cannot use `Never` as struct field type".to_string()
            }
            TypeErrorKind::UnsupportedOperator { ty, op, trait_name } => {
                format!(
                    "cannot apply operator `{op}` to type `{}`; implement `{trait_name}` trait",
                    ty.display_name()
                )
            }
            TypeErrorKind::DuplicateImpl { .. } => {
                "duplicate trait implementation for this type".to_string()
            }
            TypeErrorKind::OverlappingImpls { .. } => {
                "overlapping trait implementations with equal specificity".to_string()
            }
            TypeErrorKind::ConflictingDefaults { .. } => {
                "conflicting default methods from multiple super-traits".to_string()
            }
            TypeErrorKind::AmbiguousMethod { .. } => {
                "ambiguous method call: multiple traits provide this method".to_string()
            }
            TypeErrorKind::NotObjectSafe { .. } => {
                "trait cannot be made into an object".to_string()
            }
            TypeErrorKind::NotIndexable { ty } => {
                format!(
                    "type `{}` does not support indexing; implement `Index` trait",
                    ty.display_name()
                )
            }
            TypeErrorKind::IndexKeyMismatch {
                ty,
                expected_key,
                found_key,
            } => {
                format!(
                    "wrong index key type for `{}`: expected {}, found {}",
                    ty.display_name(),
                    expected_key.display_name(),
                    found_key.display_name()
                )
            }
            TypeErrorKind::AmbiguousIndex { ty } => {
                format!(
                    "ambiguous index: type `{}` has multiple `Index` implementations",
                    ty.display_name()
                )
            }
            TypeErrorKind::CannotDeriveDefaultForSumType { .. } => {
                "cannot derive `Default` for sum type".to_string()
            }
            TypeErrorKind::CannotDeriveHashableWithoutEq { .. } => {
                "cannot derive `Hashable` without `Eq`".to_string()
            }
            TypeErrorKind::HashInvariantViolation { .. } => {
                "`Hashable` implementation may violate hash invariant".to_string()
            }
            TypeErrorKind::NonHashableMapKey { key_type } => {
                format!(
                    "`{}` cannot be used as map key (missing `Hashable`)",
                    key_type.display_name()
                )
            }
            TypeErrorKind::FieldMissingTraitInDerive { .. } => {
                "field type does not implement trait required by derive".to_string()
            }
            TypeErrorKind::TraitNotDerivable { .. } => "trait cannot be derived".to_string(),
            TypeErrorKind::InvalidFormatSpec { spec, reason } => {
                format!("invalid format specification `{spec}`: {reason}")
            }
            TypeErrorKind::FormatTypeMismatch {
                expr_type,
                format_type,
                valid_for,
            } => {
                format!(
                    "format type `{format_type}` not supported for `{}`; valid for {valid_for}",
                    expr_type.display_name()
                )
            }
            TypeErrorKind::IntoNotImplemented { ty, target } => {
                if let Some(t) = target {
                    format!(
                        "type `{}` does not implement `Into<{}>`",
                        ty.display_name(),
                        t.display_name()
                    )
                } else {
                    format!("type `{}` does not implement `Into`", ty.display_name())
                }
            }
            TypeErrorKind::AmbiguousInto { ty } => {
                format!(
                    "ambiguous `.into()` call on `{}`: multiple `Into` implementations apply",
                    ty.display_name()
                )
            }
            TypeErrorKind::MissingPrintable { ty } => {
                format!(
                    "`{}` does not implement `Printable` (cannot be used in string interpolation)",
                    ty.display_name()
                )
            }
        }
    }

    /// Get the error code for this error kind.
    ///
    /// Maps each `TypeErrorKind` to an `ErrorCode`, matching V1's conventions:
    /// - E2001: Type mismatches
    /// - E2003: Unknown identifiers, undefined fields
    /// - E2004: Arity mismatches
    /// - E2005: Ambiguous types
    /// - E2008: Infinite/cyclic types
    /// - E2014: Missing capabilities
    pub fn code(&self) -> ErrorCode {
        match &self.kind {
            // E2001: Type mismatches and constraint violations
            TypeErrorKind::Mismatch { .. }
            | TypeErrorKind::PatternMismatch { .. }
            | TypeErrorKind::NonExhaustiveMatch { .. }
            | TypeErrorKind::UnsatisfiedBound { .. } => ErrorCode::E2001,

            // E2003: Unknown/undefined names and fields
            TypeErrorKind::UnknownIdent { .. }
            | TypeErrorKind::UndefinedField { .. }
            | TypeErrorKind::RigidMismatch { .. }
            | TypeErrorKind::ImportError { .. }
            | TypeErrorKind::NotAStruct { .. }
            | TypeErrorKind::DuplicateField { .. } => ErrorCode::E2003,

            // E2004: Arity and field count mismatches
            TypeErrorKind::ArityMismatch { .. } | TypeErrorKind::MissingFields { .. } => {
                ErrorCode::E2004
            }

            // E2005: Ambiguous types
            TypeErrorKind::AmbiguousType { .. } => ErrorCode::E2005,

            // E2008: Infinite/cyclic types
            TypeErrorKind::InfiniteType { .. } => ErrorCode::E2008,

            // E2010: Missing associated types / duplicate implementation
            TypeErrorKind::MissingAssocType { .. } | TypeErrorKind::DuplicateImpl { .. } => {
                ErrorCode::E2010
            }

            // E2014: Missing capabilities
            TypeErrorKind::MissingCapability { .. } => ErrorCode::E2014,

            // E2019: Never type in struct field
            TypeErrorKind::UninhabitedStructField { .. } => ErrorCode::E2019,

            // E2020: Unsupported operator (missing trait implementation)
            TypeErrorKind::UnsupportedOperator { .. } => ErrorCode::E2020,

            // E2021: Overlapping implementations
            TypeErrorKind::OverlappingImpls { .. } => ErrorCode::E2021,

            // E2022: Conflicting defaults
            TypeErrorKind::ConflictingDefaults { .. } => ErrorCode::E2022,

            // E2023: Ambiguous method
            TypeErrorKind::AmbiguousMethod { .. } => ErrorCode::E2023,

            // E2024: Not object-safe
            TypeErrorKind::NotObjectSafe { .. } => ErrorCode::E2024,

            // E2025: Type not indexable
            TypeErrorKind::NotIndexable { .. } => ErrorCode::E2025,

            // E2026: Wrong index key type
            TypeErrorKind::IndexKeyMismatch { .. } => ErrorCode::E2026,

            // E2027: Ambiguous index key type
            TypeErrorKind::AmbiguousIndex { .. } => ErrorCode::E2027,

            // E2028: Cannot derive Default for sum type
            TypeErrorKind::CannotDeriveDefaultForSumType { .. } => ErrorCode::E2028,

            // E2029: Cannot derive Hashable without Eq
            TypeErrorKind::CannotDeriveHashableWithoutEq { .. } => ErrorCode::E2029,

            // E2030: Hash invariant violation
            TypeErrorKind::HashInvariantViolation { .. } => ErrorCode::E2030,

            // E2031: Non-hashable map key
            TypeErrorKind::NonHashableMapKey { .. } => ErrorCode::E2031,

            // E2032: Field missing trait in derive
            TypeErrorKind::FieldMissingTraitInDerive { .. } => ErrorCode::E2032,

            // E2033: Trait not derivable
            TypeErrorKind::TraitNotDerivable { .. } => ErrorCode::E2033,

            // E2034: Invalid format specification
            TypeErrorKind::InvalidFormatSpec { .. } => ErrorCode::E2034,

            // E2035: Format type not supported for expression type
            TypeErrorKind::FormatTypeMismatch { .. } => ErrorCode::E2035,

            // E2036: Type does not implement Into<T>
            TypeErrorKind::IntoNotImplemented { .. } => ErrorCode::E2036,

            // E2037: Ambiguous Into conversion
            TypeErrorKind::AmbiguousInto { .. } => ErrorCode::E2037,

            // E2038: Missing Printable for string interpolation
            TypeErrorKind::MissingPrintable { .. } => ErrorCode::E2038,
        }
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
            suggestions: vec![Suggestion::text(
                "`self` can only be used inside impl blocks",
                0,
            )],
        }
    }

    /// Create an undefined constant reference error.
    pub fn undefined_const(name: Name, span: Span) -> Self {
        Self {
            span,
            kind: TypeErrorKind::UnknownIdent {
                name,
                similar: vec![],
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                format!("constant `${name:?}` is not defined in this scope"),
                0,
            )],
        }
    }

    /// Create an import resolution error.
    ///
    /// Used when an import path cannot be resolved to a file or when
    /// the imported module has issues.
    pub fn import_error(message: impl Into<String>, span: Span, kind: ImportErrorKind) -> Self {
        let msg = message.into();
        Self {
            span,
            kind: TypeErrorKind::ImportError {
                kind,
                message: msg.clone(),
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(format!("check the import path: {msg}"), 0)],
        }
    }

    /// Create a missing associated type error.
    ///
    /// Used when an impl block doesn't define a required associated type.
    pub fn missing_assoc_type(span: Span, assoc_name: Name, trait_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::MissingAssocType {
                assoc_name,
                trait_name,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add `type <Name> = <Type>` to the impl block",
                0,
            )],
        }
    }

    /// Create an unsatisfied trait bound error.
    ///
    /// Used when a type doesn't satisfy a required trait bound (e.g., from a where clause).
    pub fn unsatisfied_bound(span: Span, message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            span,
            kind: TypeErrorKind::UnsatisfiedBound {
                message: msg.clone(),
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(msg, 1)],
        }
    }

    /// Create an error for attempting to iterate over `Range<float>`.
    ///
    /// Float ranges are not iterable because float arithmetic is imprecise.
    /// The `suggestion` parameter provides a context-specific example
    /// (e.g., method call vs for-loop syntax).
    pub fn range_float_not_iterable(span: Span, suggestion: &str) -> Self {
        Self::unsatisfied_bound(
            span,
            format!(
                "`Range<float>` does not implement `Iterable` — \
                 floating-point ranges cannot be iterated because \
                 float arithmetic is imprecise (use an int range \
                 with conversion, e.g., `{suggestion}`)"
            ),
        )
    }

    /// Create a "not a struct" error for struct literal with non-struct name.
    pub fn not_a_struct(span: Span, name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::NotAStruct { name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "only struct types can be constructed with `Name { field: value }` syntax",
                0,
            )],
        }
    }

    /// Create a "missing fields" error for struct literal.
    pub fn missing_fields(span: Span, struct_name: Name, fields: Vec<Name>) -> Self {
        let count = fields.len();
        let s = if count == 1 { "" } else { "s" };
        Self {
            span,
            kind: TypeErrorKind::MissingFields {
                struct_name,
                fields,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                format!("add the missing field{s} to the struct literal"),
                0,
            )],
        }
    }

    /// Create a "duplicate field" error for struct literal.
    pub fn duplicate_field(span: Span, struct_name: Name, field: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::DuplicateField { struct_name, field },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text("remove the duplicate field", 0)],
        }
    }

    /// Create an "uninhabited struct field" error (E2019).
    ///
    /// Emitted when `Never` is used as a struct field type, which would make the
    /// struct unconstructable. `Never` may appear in sum type variant payloads
    /// (making the variant uninhabited) but not in struct fields.
    pub fn uninhabited_struct_field(span: Span, struct_name: Name, field: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::UninhabitedStructField { struct_name, field },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "use `Never` in sum type variants instead, or use `Option<T>` for optional fields",
                0,
            )],
        }
    }

    /// Create an "unsupported operator" error (E2020).
    ///
    /// Emitted when an operator is used on a type that doesn't implement the
    /// corresponding operator trait.
    pub fn unsupported_operator(
        span: Span,
        ty: Idx,
        op: &'static str,
        trait_name: &'static str,
    ) -> Self {
        Self {
            span,
            kind: TypeErrorKind::UnsupportedOperator { ty, op, trait_name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                format!("implement `{trait_name}` for this type"),
                0,
            )],
        }
    }

    /// Create a "not indexable" error (E2025).
    ///
    /// Emitted when `x[k]` is used on a type that doesn't implement `Index`.
    pub fn not_indexable(span: Span, ty: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::NotIndexable { ty },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "implement `Index<Key, Value>` for this type",
                0,
            )],
        }
    }

    /// Create an "index key mismatch" error (E2026).
    ///
    /// Emitted when `x[k]` uses a key type that doesn't match the `Index` impl.
    pub fn index_key_mismatch(span: Span, ty: Idx, expected_key: Idx, found_key: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::IndexKeyMismatch {
                ty,
                expected_key,
                found_key,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "use a key expression matching the Index implementation's key type",
                0,
            )],
        }
    }

    /// Create an "ambiguous index" error (E2027).
    ///
    /// Emitted when multiple `Index` impls match and the key type is ambiguous.
    pub fn ambiguous_index(span: Span, ty: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::AmbiguousIndex { ty },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add a type annotation to the key to disambiguate",
                0,
            )],
        }
    }

    /// Create a "cannot derive Default for sum type" error (E2028).
    ///
    /// Emitted when `#[derive(Default)]` is applied to a sum type, which is
    /// invalid because there is no unambiguous default variant.
    pub fn cannot_derive_default_for_sum_type(span: Span, type_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::CannotDeriveDefaultForSumType { type_name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "remove `Default` from derive list, or implement `Default` manually choosing a specific variant",
                0,
            )],
        }
    }

    /// Create a "cannot derive Hashable without Eq" error (E2029).
    ///
    /// Emitted when `#[derive(Hashable)]` is applied to a type that does not
    /// also derive or implement `Eq`. The hash invariant requires that equal
    /// values have equal hashes.
    pub fn cannot_derive_hashable_without_eq(span: Span, type_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::CannotDeriveHashableWithoutEq { type_name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add `Eq` to the derive list: `#[derive(Eq, Hashable)]`",
                0,
            )],
        }
    }

    /// Create a "hash invariant violation" warning (E2030).
    ///
    /// Emitted when a type's `Hashable` and `Eq` implementations may be
    /// inconsistent (e.g., one is derived and the other is manual).
    pub fn hash_invariant_violation(span: Span, type_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::HashInvariantViolation { type_name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "ensure equal values produce equal hashes: if a == b then a.hash() == b.hash()",
                0,
            )],
        }
    }

    /// Create a "non-hashable map key" error (E2031).
    ///
    /// Emitted when a type that does not implement `Hashable` is used as a
    /// map key or set element type.
    pub fn non_hashable_map_key(span: Span, key_type: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::NonHashableMapKey { key_type },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "add `#[derive(Eq, Hashable)]` to the type, or implement `Hashable` manually",
                0,
            )],
        }
    }

    /// Create a "field missing trait in derive" error (E2032).
    ///
    /// Emitted when `#[derive(Trait)]` is applied to a type but one of its
    /// fields does not implement the required trait.
    pub fn field_missing_trait_in_derive(
        span: Span,
        type_name: Name,
        trait_name: Name,
        field_name: Name,
        field_type: Idx,
    ) -> Self {
        Self {
            span,
            kind: TypeErrorKind::FieldMissingTraitInDerive {
                type_name,
                trait_name,
                field_name,
                field_type,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "ensure all field types implement the derived trait, or implement the trait manually",
                0,
            )],
        }
    }

    /// Create a "trait not derivable" error (E2033).
    ///
    /// Emitted when `#[derive(Trait)]` is applied with a trait that cannot be
    /// automatically derived.
    pub fn trait_not_derivable(span: Span, trait_name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::TraitNotDerivable { trait_name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "derivable traits are: Eq, Clone, Hashable, Comparable, Printable, Debug, Default",
                0,
            )],
        }
    }

    /// Create a "duplicate impl" error (E2010).
    ///
    /// Emitted when `impl Trait for Type` is defined more than once.
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

/// What kind of type error occurred.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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
        /// Function name (for function arity mismatches in error messages).
        func_name: Option<String>,
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

    /// Import resolution error.
    ImportError {
        /// Structured error kind for programmatic matching.
        kind: ImportErrorKind,
        /// Error message from import resolution.
        message: String,
    },

    /// Missing associated type in impl block.
    MissingAssocType {
        /// Name of the missing associated type.
        assoc_name: Name,
        /// Name of the trait requiring it.
        trait_name: Name,
    },

    /// Unsatisfied trait bound on associated type.
    UnsatisfiedBound {
        /// Description of what doesn't satisfy the bound.
        message: String,
    },

    /// Name used in struct literal is not a struct type.
    NotAStruct {
        /// The name that was used.
        name: Name,
    },

    /// Struct literal is missing required fields.
    MissingFields {
        /// The struct name.
        struct_name: Name,
        /// The missing field names.
        fields: Vec<Name>,
    },

    /// Struct literal provides the same field more than once.
    DuplicateField {
        /// The struct name.
        struct_name: Name,
        /// The duplicated field name.
        field: Name,
    },

    /// Never type used as struct field (uninhabited struct).
    UninhabitedStructField {
        /// The struct name.
        struct_name: Name,
        /// The field with Never type.
        field: Name,
    },

    /// Operator not supported for type (no trait implementation).
    UnsupportedOperator {
        /// The type that doesn't support the operator.
        ty: Idx,
        /// The operator symbol (e.g., "+", "-", "~").
        op: &'static str,
        /// The trait name that would need to be implemented.
        trait_name: &'static str,
    },

    /// Duplicate trait implementation for the same type (E2010).
    DuplicateImpl {
        /// Name of the trait being implemented.
        trait_name: Name,
        /// Span of the first (existing) implementation.
        first_span: Span,
    },

    /// Overlapping implementations with equal specificity (E2021).
    OverlappingImpls {
        /// Name of the trait with overlapping impls.
        trait_name: Name,
        /// Span of the first (existing) implementation.
        first_span: Span,
    },

    /// Conflicting default methods from multiple super-traits (E2022).
    ConflictingDefaults {
        /// The method with conflicting defaults.
        method: Name,
        /// First super-trait providing a default.
        trait_a: Name,
        /// Second super-trait providing a different default.
        trait_b: Name,
    },

    /// Ambiguous method call — multiple trait impls provide the same method (E2023).
    AmbiguousMethod {
        /// The method name that's ambiguous.
        method: Name,
        /// Traits that each provide this method.
        candidates: Vec<Name>,
    },

    /// Trait is not object-safe — cannot be used as a trait object (E2024).
    NotObjectSafe {
        /// The trait that is not object-safe.
        trait_name: Name,
        /// The specific object safety violations.
        violations: Vec<crate::ObjectSafetyViolation>,
    },

    /// Type does not implement the `Index` trait — not indexable (E2025).
    NotIndexable {
        /// The type that was used with subscript syntax.
        ty: Idx,
    },

    /// Wrong key type for subscript expression (E2026).
    IndexKeyMismatch {
        /// The receiver type.
        ty: Idx,
        /// The expected key type (from the Index impl).
        expected_key: Idx,
        /// The actual key type found.
        found_key: Idx,
    },

    /// Multiple `Index` impls match the key type (E2027).
    AmbiguousIndex {
        /// The receiver type with ambiguous Index impls.
        ty: Idx,
    },

    /// Cannot derive `Default` for a sum type (E2028).
    CannotDeriveDefaultForSumType {
        /// The sum type name.
        type_name: Name,
    },

    /// Cannot derive `Hashable` without `Eq` (E2029).
    CannotDeriveHashableWithoutEq {
        /// The type name that derives Hashable.
        type_name: Name,
    },

    /// `Hashable` implementation may violate hash invariant (E2030).
    HashInvariantViolation {
        /// The type name with the potentially inconsistent impls.
        type_name: Name,
    },

    /// Type cannot be used as map key — missing `Hashable` (E2031).
    NonHashableMapKey {
        /// The key type that doesn't implement Hashable.
        key_type: Idx,
    },

    /// Field type does not implement trait required by derive (E2032).
    FieldMissingTraitInDerive {
        /// The type name being derived.
        type_name: Name,
        /// The trait being derived.
        trait_name: Name,
        /// The field name whose type lacks the trait.
        field_name: Name,
        /// The field's resolved type.
        field_type: Idx,
    },

    /// Trait cannot be derived — not in the derivable set (E2033).
    TraitNotDerivable {
        /// The trait name that was attempted.
        trait_name: Name,
    },

    /// Invalid format specification in template string (E2034).
    InvalidFormatSpec {
        /// The raw format spec string that failed to parse.
        spec: String,
        /// The parse error message.
        reason: String,
    },

    /// Format type not supported for expression type (E2035).
    FormatTypeMismatch {
        /// The expression's inferred type.
        expr_type: Idx,
        /// The format type that's incompatible (e.g., "x", "b", "e").
        format_type: String,
        /// Which types are valid for this format type.
        valid_for: &'static str,
    },

    /// Type does not implement `Into<T>` — no conversion available (E2036).
    IntoNotImplemented {
        /// The source type that `.into()` was called on.
        ty: Idx,
        /// The expected target type, if known from context.
        target: Option<Idx>,
    },

    /// Multiple `Into` implementations apply — ambiguous conversion (E2037).
    AmbiguousInto {
        /// The source type that `.into()` was called on.
        ty: Idx,
    },

    /// Type does not implement `Printable` — cannot be used in string interpolation (E2038).
    MissingPrintable {
        /// The type that doesn't implement Printable.
        ty: Idx,
    },
}

/// What kind of arity mismatch occurred.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
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

/// Re-export the canonical `ImportErrorKind` from `ori_ir`.
///
/// Single source of truth shared by both the import resolver (`oric::imports`)
/// and the type checker. All 6 variants are available without lossy mapping.
pub use ori_ir::ImportErrorKind;

/// Context information for a type error.
///
/// Tracks WHERE in code the error occurred and WHY we expected a type.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
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

/// Generate a specific message for a `TypeProblem`, if the problem
/// provides more context than the generic mismatch message.
///
/// Uses `Idx::display_name()` for type names (renders primitives, falls back
/// to `"<type>"` for complex types).
fn problem_message(problem: &TypeProblem) -> Option<String> {
    problem_message_with(problem, &|idx| idx.display_name().to_string())
}

/// Generate a rich problem message using a type formatter.
///
/// Uses the provided formatter for full type names with backtick wrapping,
/// instead of `Idx::display_name()`.
fn problem_message_rich(
    problem: &TypeProblem,
    format_type: &dyn Fn(Idx) -> String,
) -> Option<String> {
    problem_message_with(problem, &|idx| format!("`{}`", format_type(idx)))
}

/// Shared implementation for problem message generation.
///
/// The `format_type` closure controls how `Idx` values are rendered:
/// - Simple path: `|idx| idx.display_name().to_string()` (no backticks)
/// - Rich path: `|idx| format!("`{}`", full_format(idx))` (with backticks)
fn problem_message_with(
    problem: &TypeProblem,
    format_type: &dyn Fn(Idx) -> String,
) -> Option<String> {
    match problem {
        TypeProblem::NotCallable { actual_type } => Some(format!(
            "expected a function, found {}",
            format_type(*actual_type)
        )),
        TypeProblem::WrongArity { expected, found } => {
            let s = if *expected == 1 { "" } else { "s" };
            Some(format!("expected {expected} argument{s}, found {found}"))
        }
        TypeProblem::IntFloat { expected, found }
        | TypeProblem::NumericTypeMismatch { expected, found } => Some(format!(
            "expected `{expected}`, found `{found}`; use `{expected}(x)` to convert"
        )),
        TypeProblem::NumberToString => {
            Some("cannot use number as string; use `str(x)` to convert".to_string())
        }
        TypeProblem::StringToNumber => {
            Some("cannot use string as number; use `int(x)` or `float(x)` to convert".to_string())
        }
        TypeProblem::ExpectedList { .. } => {
            Some("expected a list; wrap the value in a list: `[x]`".to_string())
        }
        TypeProblem::ExpectedOption => Some("expected an Option type".to_string()),
        TypeProblem::NeedsUnwrap { inner_type } => Some(format!(
            "value needs to be unwrapped; inner type is {}",
            format_type(*inner_type)
        )),
        TypeProblem::ReturnMismatch { expected, found } => Some(format!(
            "return type mismatch: expected {}, found {}",
            format_type(*expected),
            format_type(*found)
        )),
        TypeProblem::ArgumentMismatch {
            arg_index,
            expected,
            found,
        } => Some(format!(
            "argument {} has type {}, expected {}",
            arg_index + 1,
            format_type(*found),
            format_type(*expected)
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
                    "left operand of {op_category} operator must be `{required_type}`"
                ))
            }
        }
        TypeProblem::ClosureSelfCapture => Some("closure cannot capture itself".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
