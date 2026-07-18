//! Closure-driven rich formatting for type-check errors.

use ori_ir::Name;

use super::super::kind::{AmbiguousTypeSite, TypeErrorKind};
use super::super::TypeCheckError;
use super::problems::problem_message_rich;
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
    pub fn format_message_rich(
        &self,
        format_type: &dyn Fn(Idx) -> String,
        format_name: &dyn Fn(Name) -> String,
    ) -> String {
        if let Some(message) = self.rich_resolution_message(format_type, format_name) {
            return message;
        }
        if let Some(message) = self.rich_inference_message(format_type, format_name) {
            return message;
        }
        if let Some(message) = self.rich_trait_message(format_type, format_name) {
            return message;
        }
        if let Some(message) = self.rich_derive_message(format_type, format_name) {
            return message;
        }
        if let Some(message) = self.rich_conversion_message(format_type, format_name) {
            return message;
        }
        if let Some(message) = self.rich_ownership_message(format_type, format_name) {
            return message;
        }

        match &self.kind {
            TypeErrorKind::Mismatch { .. }
            | TypeErrorKind::UnknownIdent { .. }
            | TypeErrorKind::UnresolvedTrait { .. }
            | TypeErrorKind::UndefinedField { .. }
            | TypeErrorKind::UnknownMethod { .. }
            | TypeErrorKind::ArityMismatch { .. }
            | TypeErrorKind::MissingCapability { .. }
            | TypeErrorKind::InfiniteType { .. }
            | TypeErrorKind::AmbiguousType { .. }
            | TypeErrorKind::PatternMismatch { .. }
            | TypeErrorKind::NonExhaustiveMatch { .. }
            | TypeErrorKind::RigidMismatch { .. }
            | TypeErrorKind::ImportError { .. }
            | TypeErrorKind::MissingAssocType { .. }
            | TypeErrorKind::UnsatisfiedBound { .. }
            | TypeErrorKind::NotAStruct { .. }
            | TypeErrorKind::MissingFields { .. }
            | TypeErrorKind::DuplicateField { .. }
            | TypeErrorKind::UninhabitedStructField { .. }
            | TypeErrorKind::UnsupportedOperator { .. }
            | TypeErrorKind::DuplicateImpl { .. }
            | TypeErrorKind::OverlappingImpls { .. }
            | TypeErrorKind::ConflictingDefaults { .. }
            | TypeErrorKind::AmbiguousMethod { .. }
            | TypeErrorKind::NotObjectSafe { .. }
            | TypeErrorKind::NotIndexable { .. }
            | TypeErrorKind::IndexKeyMismatch { .. }
            | TypeErrorKind::AmbiguousIndex { .. }
            | TypeErrorKind::CannotDeriveForSumType { .. }
            | TypeErrorKind::CannotDeriveWithoutSupertrait { .. }
            | TypeErrorKind::HashInvariantViolation { .. }
            | TypeErrorKind::NonHashableMapKey { .. }
            | TypeErrorKind::FieldMissingTraitInDerive { .. }
            | TypeErrorKind::TraitNotDerivable { .. }
            | TypeErrorKind::InvalidFormatSpec { .. }
            | TypeErrorKind::FormatTypeMismatch { .. }
            | TypeErrorKind::IntoNotImplemented { .. }
            | TypeErrorKind::AmbiguousInto { .. }
            | TypeErrorKind::MissingPrintable { .. }
            | TypeErrorKind::AssignToImmutable { .. }
            | TypeErrorKind::IndexAssignNotSupported { .. }
            | TypeErrorKind::AssignThroughParameter { .. }
            | TypeErrorKind::UnsupportedFeature { .. }
            | TypeErrorKind::InvalidReprAttribute { .. }
            | TypeErrorKind::ConditionalPartialMove { .. }
            | TypeErrorKind::UseAfterDropEarly { .. }
            | TypeErrorKind::DropPartialMove { .. }
            | TypeErrorKind::ValueDropConflict { .. }
            | TypeErrorKind::PreContractNotBool { .. }
            | TypeErrorKind::PostContractVoidReturn
            | TypeErrorKind::PreContractUnknownIdent { .. }
            | TypeErrorKind::RefutablePattern { .. }
            | TypeErrorKind::BreakValueInVoidLoop { .. }
            | TypeErrorKind::ContinueValueInNonCollectingLoop { .. }
            | TypeErrorKind::OrPatternBindingMismatch { .. } => {
                unreachable!("type error kind was not handled by its message family")
            }
        }
    }

    fn rich_resolution_message(
        &self,
        format_type: &dyn Fn(Idx) -> String,
        format_name: &dyn Fn(Name) -> String,
    ) -> Option<String> {
        use std::fmt::Write;
        let message = match &self.kind {
            TypeErrorKind::Mismatch {
                expected,
                found,
                problems,
            } => {
                for problem in problems {
                    if let Some(detail) = problem_message_rich(problem, format_type) {
                        return Some(format!("type mismatch: {detail}"));
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
            TypeErrorKind::UnresolvedTrait { trait_name } => {
                format!(
                    "unresolved trait `{}` — the trait is not registered (is the prelude available, or is the name a typo?)",
                    format_name(*trait_name)
                )
            }
            TypeErrorKind::UndefinedField { ty, field, .. } => {
                format!(
                    "no such field `{}` on type `{}`",
                    format_name(*field),
                    format_type(*ty)
                )
            }
            TypeErrorKind::UnknownMethod { ty, method } => {
                format!(
                    "no method `{}` on type `{}`",
                    format_name(*method),
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
            _ => return None,
        };
        Some(message)
    }

    fn rich_inference_message(
        &self,
        format_type: &dyn Fn(Idx) -> String,
        format_name: &dyn Fn(Name) -> String,
    ) -> Option<String> {
        let message = match &self.kind {
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
            TypeErrorKind::AmbiguousType { site, .. } => match site {
                AmbiguousTypeSite::Expression => "cannot infer type in expression".to_string(),
                AmbiguousTypeSite::EmptyList => {
                    "cannot infer the type of this empty list; add a type annotation like `let x: [int] = []`"
                        .to_string()
                }
                AmbiguousTypeSite::LambdaParam => {
                    "cannot infer the type of this closure parameter; add a full typed-lambda annotation like `(x: int) -> ReturnT = body`"
                        .to_string()
                }
            },
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
            _ => return None,
        };
        Some(message)
    }

    fn rich_trait_message(
        &self,
        format_type: &dyn Fn(Idx) -> String,
        format_name: &dyn Fn(Name) -> String,
    ) -> Option<String> {
        let message = match &self.kind {
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
            _ => return None,
        };
        Some(message)
    }

    fn rich_derive_message(
        &self,
        format_type: &dyn Fn(Idx) -> String,
        format_name: &dyn Fn(Name) -> String,
    ) -> Option<String> {
        let message = match &self.kind {
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
            _ => return None,
        };
        Some(message)
    }

    fn rich_conversion_message(
        &self,
        format_type: &dyn Fn(Idx) -> String,
        format_name: &dyn Fn(Name) -> String,
    ) -> Option<String> {
        let message = match &self.kind {
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
            TypeErrorKind::IndexAssignNotSupported { ty } => {
                format!(
                    "type `{}` does not support index assignment",
                    format_type(*ty)
                )
            }
            TypeErrorKind::AssignThroughParameter { name } => {
                format!("cannot assign through parameter `{}`", format_name(*name))
            }
            TypeErrorKind::UnsupportedFeature { feature } => {
                format!("`{feature}` is not yet supported")
            }
            TypeErrorKind::InvalidReprAttribute { reason, .. } => {
                format!("invalid `#repr` attribute: {reason}")
            }
            _ => return None,
        };
        Some(message)
    }

    fn rich_ownership_message(
        &self,
        format_type: &dyn Fn(Idx) -> String,
        format_name: &dyn Fn(Name) -> String,
    ) -> Option<String> {
        let message = match &self.kind {
            TypeErrorKind::ConditionalPartialMove { aggregate, field } => {
                format!(
                    "conditional partial move of `{}.{}` not statically computable; \
                     make the projection unconditional, or mirror it symmetrically on every branch",
                    format_name(*aggregate),
                    format_name(*field)
                )
            }
            TypeErrorKind::UseAfterDropEarly { binding } => {
                format!(
                    "use of `{}` after `drop_early` consumed it; use the value before the \
                     `drop_early` call, or re-bind the name afterward",
                    format_name(*binding)
                )
            }
            TypeErrorKind::DropPartialMove {
                aggregate,
                field,
                type_name,
            } => {
                format!(
                    "cannot partially move field `{}.{}` of type `{}` (implements `Drop`); \
                     use full move, field borrow, or match-destructuring instead",
                    format_name(*aggregate),
                    format_name(*field),
                    format_name(*type_name)
                )
            }
            TypeErrorKind::ValueDropConflict { type_name } => {
                format!(
                    "type `{}` carries both `Value` and `Drop`; `Value` declares inline storage \
                     with bitwise copy and no ARC, so the refcount-zero cleanup path `@drop` \
                     hooks into never fires — the two markers are mutually exclusive",
                    format_name(*type_name)
                )
            }
            TypeErrorKind::PreContractNotBool { actual } => {
                format!(
                    "pre() condition must have type `bool`, found `{}`",
                    format_type(*actual)
                )
            }
            TypeErrorKind::PostContractVoidReturn => {
                "post() cannot apply to a function returning `void`".to_string()
            }
            TypeErrorKind::PreContractUnknownIdent { name } => {
                format!(
                    "pre() references unknown identifier `{}` — only function parameters \
                     and module-level bindings are visible",
                    format_name(*name)
                )
            }
            // RefutablePattern carries no pool-dependent rich formatting; delegate
            // to the SSOT renderer (`message()`) so the rich path cannot drift from
            // the spec-conformant text (`message()` is the single SSOT renderer).
            TypeErrorKind::RefutablePattern { .. } => self.message(),
            TypeErrorKind::BreakValueInVoidLoop { loop_kind } => {
                format!(
                    "`break` with a value is not allowed in {}: this loop form has type `void`",
                    loop_kind.description()
                )
            }
            TypeErrorKind::ContinueValueInNonCollectingLoop { loop_kind } => {
                format!(
                    "`continue` with a value is not allowed in {}: this loop does not accumulate values",
                    loop_kind.description()
                )
            }
            TypeErrorKind::OrPatternBindingMismatch { name, reason } => {
                use super::super::kind::OrBindingMismatchReason;
                let var = format_name(*name);
                match reason {
                    OrBindingMismatchReason::NameDivergence => format!(
                        "or-pattern binds `{var}` on only some alternatives: every `|` alternative must bind the same names, or the arm body reads `{var}` unbound when a non-binding alternative matches"
                    ),
                    OrBindingMismatchReason::TypeDivergence { found, other } => format!(
                        "or-pattern binds `{var}` at `{}` in one alternative but `{}` in another: a name shared across `|` alternatives must have the same type",
                        format_type(*found),
                        format_type(*other)
                    ),
                }
            }
            _ => return None,
        };
        Some(message)
    }
}
