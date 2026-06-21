//! Convenience constructors for type/struct/operator/derive errors.
//!
//! These factory methods create [`TypeCheckError`] instances for specific
//! error categories with appropriate suggestions pre-populated.

use ori_diagnostic::Suggestion;
use ori_ir::{DerivedTrait, Name, Span};

use super::kind::{ErrorContext, ImportErrorKind, TypeErrorKind};
use super::TypeCheckError;
use crate::infer::RefutableReason;
use crate::Idx;

impl TypeCheckError {
    /// Create an undefined identifier error.
    pub fn undefined_identifier(name: Name, span: Span) -> Self {
        Self::unknown_ident(span, name, vec![])
    }

    /// Create a refutable-pattern-in-let-binding error (E2001).
    ///
    /// Spec Clauses 15.4 and 16: `let <pat> = expr` SHALL require an
    /// irrefutable pattern. Suggestion points the user at `match`.
    pub fn refutable_pattern(span: Span, reason: RefutableReason) -> Self {
        Self {
            span,
            kind: TypeErrorKind::RefutablePattern { reason },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "Use `match` instead — `match` arms accept refutable patterns",
                0,
            )],
        }
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

    /// Create an error for constructing a `Range<float>`.
    ///
    /// Ranges require integer bounds per the spec. Float ranges are not
    /// supported because float arithmetic is imprecise for stepping.
    pub fn range_float_not_constructible(span: Span) -> Self {
        Self::unsatisfied_bound(
            span,
            "`Range<float>` is not supported — ranges require integer bounds \
             because float arithmetic is imprecise for stepping \
             (use an int range with conversion, e.g., \
             `for i in 0..10 do i.to_float() / 10.0`)"
                .to_string(),
        )
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

    /// Create an "index assignment not supported" error (E2050).
    ///
    /// Emitted when `x[i] = v` targets a type that supports index reads but not
    /// index assignment (e.g. `str`, which is immutable through indexing).
    pub fn index_assign_not_supported(span: Span, ty: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::IndexAssignNotSupported { ty },
            context: ErrorContext::default(),
            suggestions: vec![],
        }
    }

    /// Create an "assign through parameter" error (E2051).
    ///
    /// Emitted when `p[i] = v` / `p.f = v` targets a parameter binding —
    /// parameters are not mutable roots for index/field assignment.
    pub fn assign_through_parameter(span: Span, name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::AssignThroughParameter { name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "bind a mutable local with `let` and assign through it instead",
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
    pub fn cannot_derive_for_sum_type(
        span: Span,
        type_name: Name,
        trait_kind: DerivedTrait,
    ) -> Self {
        let suggestion = format!(
            "remove `{}` from derive list, or implement `{}` manually choosing a specific variant",
            trait_kind.trait_name(),
            trait_kind.trait_name(),
        );
        Self {
            span,
            kind: TypeErrorKind::CannotDeriveForSumType {
                type_name,
                trait_kind,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(suggestion, 0)],
        }
    }

    /// Create a "cannot derive trait without required supertrait" error (E2029).
    ///
    /// Emitted when a derived trait requires a supertrait (e.g., `Hashable`
    /// requires `Eq`, `Comparable` requires `Eq`) but the type does not derive
    /// or implement the required supertrait.
    pub fn cannot_derive_without_supertrait(
        span: Span,
        type_name: Name,
        trait_kind: DerivedTrait,
        required: DerivedTrait,
    ) -> Self {
        let suggestion = format!(
            "add `{}` to the derive list: `#[derive({}, {})]`",
            required.trait_name(),
            required.trait_name(),
            trait_kind.trait_name(),
        );
        Self {
            span,
            kind: TypeErrorKind::CannotDeriveWithoutSupertrait {
                type_name,
                trait_kind,
                required,
            },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(suggestion, 0)],
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

    /// Create a "pre-contract not bool" error (E2044).
    ///
    /// Spec: Clause 10 § Function-Level Contracts. Emitted when the
    /// expression inside `pre(...)` infers to a type other than `bool`.
    pub fn pre_contract_not_bool(span: Span, actual: Idx) -> Self {
        Self {
            span,
            kind: TypeErrorKind::PreContractNotBool { actual },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "pre() conditions must evaluate to bool — use a comparison or boolean expression",
                0,
            )],
        }
    }

    /// Create a "post-contract on void-returning function" error (E2046).
    ///
    /// Spec: Clause 10 § Function-Level Contracts. Emitted when `post()`
    /// is declared on a function whose return type is `void` — there is no
    /// result value for the post-lambda to bind.
    pub fn post_contract_void_return(span: Span) -> Self {
        Self {
            span,
            kind: TypeErrorKind::PostContractVoidReturn,
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "remove the post() contract, or change the function to return a non-void value",
                0,
            )],
        }
    }

    /// Create a "pre-contract unknown identifier" error (E2047).
    ///
    /// Spec: Clause 10 § Function-Level Contracts. Emitted when the
    /// contract expression references a name that is neither a function
    /// parameter nor a module-level binding.
    pub fn pre_contract_unknown_ident(span: Span, name: Name) -> Self {
        Self {
            span,
            kind: TypeErrorKind::PreContractUnknownIdent { name },
            context: ErrorContext::default(),
            suggestions: vec![Suggestion::text(
                "pre() may reference only function parameters and module-level bindings",
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
                format!(
                    "derivable traits are: {}",
                    DerivedTrait::ALL
                        .iter()
                        .map(DerivedTrait::trait_name)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                0,
            )],
        }
    }
}
