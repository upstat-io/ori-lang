//! Rich error message formatting.
//!
//! Contains [`TypeCheckError::format_message_rich`] and [`TypeCheckError::format_with`]
//! for producing human-readable error messages with full type and name resolution.

use ori_ir::Name;

use super::kind::TypeErrorKind;
use super::TypeCheckError;
use crate::type_error::TypeProblem;
use crate::Idx;

impl TypeCheckError {
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
            TypeErrorKind::CannotDeriveForSumType {
                type_name,
                trait_kind,
            } => {
                format!(
                    "cannot derive `{}` for sum type `{}`",
                    trait_kind.trait_name(),
                    format_name(*type_name)
                )
            }
            TypeErrorKind::CannotDeriveWithoutSupertrait {
                type_name,
                trait_kind,
                required,
            } => {
                format!(
                    "cannot derive `{}` without `{}` for type `{}`",
                    trait_kind.trait_name(),
                    required.trait_name(),
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
            TypeErrorKind::AssignToImmutable { name } => {
                format!(
                    "cannot assign to immutable binding `{}`",
                    format_name(*name)
                )
            }
            TypeErrorKind::UnsupportedFeature { feature } => {
                format!("`{feature}` is not yet supported")
            }
            TypeErrorKind::InvalidReprAttribute { reason, .. } => {
                format!("invalid `#repr` attribute: {reason}")
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
pub(super) fn problem_message_with(
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
