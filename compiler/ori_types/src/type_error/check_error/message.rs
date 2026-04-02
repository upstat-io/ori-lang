//! Simple error messages and error code classification.
//!
//! Contains [`TypeCheckError::message`] (lightweight message using `Idx::display_name()`)
//! and [`TypeCheckError::code`] (error code mapping).

use ori_diagnostic::ErrorCode;

use super::format::problem_message_with;
use super::kind::TypeErrorKind;
use super::TypeCheckError;

impl TypeCheckError {
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
                    if let Some(detail) =
                        problem_message_with(problem, &|idx| idx.display_name().to_string())
                    {
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
            TypeErrorKind::CannotDeriveForSumType { trait_kind, .. } => {
                format!("cannot derive `{}` for sum type", trait_kind.trait_name())
            }
            TypeErrorKind::CannotDeriveWithoutSupertrait {
                trait_kind,
                required,
                ..
            } => {
                format!(
                    "cannot derive `{}` without `{}`",
                    trait_kind.trait_name(),
                    required.trait_name()
                )
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
            TypeErrorKind::AssignToImmutable { .. } => {
                "cannot assign to immutable binding".to_string()
            }
            TypeErrorKind::UnsupportedFeature { feature } => {
                format!("`{feature}` is not yet supported")
            }
            TypeErrorKind::InvalidReprAttribute { reason, .. } => {
                format!("invalid `#repr` attribute: {reason}")
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
            | TypeErrorKind::NotAStruct { .. } => ErrorCode::E2003,

            // E2004: Arity mismatches
            TypeErrorKind::ArityMismatch { .. } | TypeErrorKind::MissingFields { .. } => {
                ErrorCode::E2004
            }

            // E2005: Ambiguous types
            TypeErrorKind::AmbiguousType { .. } => ErrorCode::E2005,

            // E2008: Infinite/cyclic types
            TypeErrorKind::InfiniteType { .. } => ErrorCode::E2008,

            // E2010: Duplicate implementations
            TypeErrorKind::DuplicateImpl { .. }
            | TypeErrorKind::DuplicateField { .. }
            | TypeErrorKind::MissingAssocType { .. } => ErrorCode::E2010,

            // E2014: Missing capabilities
            TypeErrorKind::MissingCapability { .. } => ErrorCode::E2014,

            // E2019: Uninhabited struct field
            TypeErrorKind::UninhabitedStructField { .. } => ErrorCode::E2019,

            // E2020: Unsupported operator
            TypeErrorKind::UnsupportedOperator { .. } => ErrorCode::E2020,

            // E2021: Overlapping implementations
            TypeErrorKind::OverlappingImpls { .. } => ErrorCode::E2021,

            // E2022: Conflicting default methods
            TypeErrorKind::ConflictingDefaults { .. } => ErrorCode::E2022,

            // E2023: Ambiguous method
            TypeErrorKind::AmbiguousMethod { .. } => ErrorCode::E2023,

            // E2024: Not object-safe
            TypeErrorKind::NotObjectSafe { .. } => ErrorCode::E2024,

            // E2025: Not indexable
            TypeErrorKind::NotIndexable { .. } => ErrorCode::E2025,

            // E2026: Index key mismatch
            TypeErrorKind::IndexKeyMismatch { .. } => ErrorCode::E2026,

            // E2027: Ambiguous index
            TypeErrorKind::AmbiguousIndex { .. } => ErrorCode::E2027,

            // E2028: Cannot derive for sum type
            TypeErrorKind::CannotDeriveForSumType { .. } => ErrorCode::E2028,

            // E2029: Cannot derive without supertrait
            TypeErrorKind::CannotDeriveWithoutSupertrait { .. } => ErrorCode::E2029,

            // E2030: Hash invariant violation
            TypeErrorKind::HashInvariantViolation { .. } => ErrorCode::E2030,

            // E2031: Non-hashable map key
            TypeErrorKind::NonHashableMapKey { .. } => ErrorCode::E2031,

            // E2032: Field missing trait in derive
            TypeErrorKind::FieldMissingTraitInDerive { .. } => ErrorCode::E2032,

            // E2033: Trait not derivable
            TypeErrorKind::TraitNotDerivable { .. } => ErrorCode::E2033,

            // E2034: Invalid format spec
            TypeErrorKind::InvalidFormatSpec { .. } => ErrorCode::E2034,

            // E2035: Format type mismatch
            TypeErrorKind::FormatTypeMismatch { .. } => ErrorCode::E2035,

            // E2036: Into not implemented
            TypeErrorKind::IntoNotImplemented { .. } => ErrorCode::E2036,

            // E2037: Ambiguous into
            TypeErrorKind::AmbiguousInto { .. } => ErrorCode::E2037,

            // E2038: Missing Printable for string interpolation
            TypeErrorKind::MissingPrintable { .. } => ErrorCode::E2038,

            // E2039: Cannot assign to immutable binding
            TypeErrorKind::AssignToImmutable { .. } => ErrorCode::E2039,

            // E2040: Feature not yet supported
            TypeErrorKind::UnsupportedFeature { .. } => ErrorCode::E2040,

            // E2041: Invalid #repr attribute
            TypeErrorKind::InvalidReprAttribute { .. } => ErrorCode::E2041,
        }
    }
}
