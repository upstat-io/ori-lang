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
            TypeErrorKind::ImportError { message } => {
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
        self.format_message_rich(&|idx| pool.format_type(idx), &|name| {
            interner.lookup(name).to_string()
        })
    }

    /// Get a human-readable error message.
    ///
    /// Uses `Idx::display_name()` for type names, which renders primitives
    /// (int, bool, str, etc.) and falls back to `"<type>"` for complex types
    /// that would need a Pool to render fully.
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
            TypeErrorKind::ImportError { message } => {
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

            // E2010: Missing associated types
            TypeErrorKind::MissingAssocType { .. } => ErrorCode::E2010,

            // E2014: Missing capabilities
            TypeErrorKind::MissingCapability { .. } => ErrorCode::E2014,
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
    pub fn import_error(message: impl Into<String>, span: Span) -> Self {
        let msg = message.into();
        Self {
            span,
            kind: TypeErrorKind::ImportError {
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
fn problem_message(problem: &TypeProblem) -> Option<String> {
    match problem {
        TypeProblem::NotCallable { actual_type } => Some(format!(
            "expected a function, found {}",
            actual_type.display_name()
        )),
        TypeProblem::WrongArity { expected, found } => {
            let s = if *expected == 1 { "" } else { "s" };
            Some(format!("expected {expected} argument{s}, found {found}"))
        }
        TypeProblem::IntFloat => {
            Some("int and float are different types; use explicit conversion".to_string())
        }
        TypeProblem::NumberToString => {
            Some("cannot use number as string; use `str()` to convert".to_string())
        }
        TypeProblem::StringToNumber => {
            Some("cannot use string as number; use `int()` or `float()` to convert".to_string())
        }
        TypeProblem::ExpectedOption => Some("expected an Option type".to_string()),
        TypeProblem::NeedsUnwrap { inner_type } => Some(format!(
            "value needs to be unwrapped; inner type is {}",
            inner_type.display_name()
        )),
        TypeProblem::ReturnMismatch { expected, found } => Some(format!(
            "return type mismatch: expected {}, found {}",
            expected.display_name(),
            found.display_name()
        )),
        TypeProblem::ArgumentMismatch {
            arg_index,
            expected,
            found,
        } => Some(format!(
            "argument {} has type {}, expected {}",
            arg_index + 1,
            found.display_name(),
            expected.display_name()
        )),
        TypeProblem::BadOperandType {
            op,
            op_category,
            found_type,
            required_type,
        } => {
            if *op_category == "unary" {
                // "cannot apply `-` to `str`", "cannot apply `!` to `int`"
                Some(format!("cannot apply `{op}` to `{found_type}`"))
            } else {
                // "left operand of bitwise operator must be `int`"
                Some(format!(
                    "left operand of {op_category} operator must be `{required_type}`"
                ))
            }
        }
        TypeProblem::ClosureSelfCapture => Some("closure cannot capture itself".to_string()),
        _ => None,
    }
}

/// Generate a rich problem message using a type formatter.
///
/// Same as `problem_message` but uses the provided formatter for full type names
/// instead of `Idx::display_name()`.
fn problem_message_rich(
    problem: &TypeProblem,
    format_type: &dyn Fn(Idx) -> String,
) -> Option<String> {
    match problem {
        TypeProblem::NotCallable { actual_type } => Some(format!(
            "expected a function, found `{}`",
            format_type(*actual_type)
        )),
        TypeProblem::WrongArity { expected, found } => {
            let s = if *expected == 1 { "" } else { "s" };
            Some(format!("expected {expected} argument{s}, found {found}"))
        }
        TypeProblem::IntFloat => {
            Some("int and float are different types; use explicit conversion".to_string())
        }
        TypeProblem::NumberToString => {
            Some("cannot use number as string; use `str()` to convert".to_string())
        }
        TypeProblem::StringToNumber => {
            Some("cannot use string as number; use `int()` or `float()` to convert".to_string())
        }
        TypeProblem::ExpectedOption => Some("expected an Option type".to_string()),
        TypeProblem::NeedsUnwrap { inner_type } => Some(format!(
            "value needs to be unwrapped; inner type is `{}`",
            format_type(*inner_type)
        )),
        TypeProblem::ReturnMismatch { expected, found } => Some(format!(
            "return type mismatch: expected `{}`, found `{}`",
            format_type(*expected),
            format_type(*found)
        )),
        TypeProblem::ArgumentMismatch {
            arg_index,
            expected,
            found,
        } => Some(format!(
            "argument {} has type `{}`, expected `{}`",
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
mod tests {
    use super::*;

    #[test]
    fn create_mismatch_error() {
        let error = TypeCheckError::mismatch(
            Span::new(0, 10),
            Idx::INT,
            Idx::STR,
            vec![TypeProblem::StringToNumber],
            ErrorContext::default(),
        );

        assert!(matches!(error.kind, TypeErrorKind::Mismatch { .. }));
        assert!(!error.suggestions.is_empty());
    }

    #[test]
    fn create_unknown_ident_error() {
        let error = TypeCheckError::unknown_ident(
            Span::new(0, 5),
            Name::from_raw(1),
            vec![Name::from_raw(2)],
        );

        assert!(matches!(error.kind, TypeErrorKind::UnknownIdent { .. }));
        assert!(!error.suggestions.is_empty());
    }

    #[test]
    fn create_arity_mismatch_error() {
        let error =
            TypeCheckError::arity_mismatch(Span::new(0, 20), 2, 4, ArityMismatchKind::Function);

        assert!(matches!(error.kind, TypeErrorKind::ArityMismatch { .. }));
        assert!(!error.suggestions.is_empty());
        assert!(error.suggestions[0].message.contains("remove"));
    }

    #[test]
    fn error_context() {
        let context = ErrorContext::new(ContextKind::IfCondition)
            .with_note("conditions must evaluate to bool");

        assert!(context.describe().is_some());
        assert!(!context.notes.is_empty());
    }

    #[test]
    fn arity_kind_descriptions() {
        assert_eq!(ArityMismatchKind::Function.description(), "arguments");
        assert_eq!(ArityMismatchKind::Tuple.description(), "tuple elements");
    }

    #[test]
    fn message_for_mismatch() {
        let error = TypeCheckError::mismatch(
            Span::new(0, 10),
            Idx::INT,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        );
        assert_eq!(error.message(), "type mismatch: expected int, found str");
    }

    #[test]
    fn message_for_unknown_ident() {
        let error = TypeCheckError::unknown_ident(Span::new(0, 5), Name::from_raw(1), vec![]);
        assert!(error.message().contains("unknown identifier"));
    }

    #[test]
    fn code_for_mismatch() {
        let error = TypeCheckError::mismatch(
            Span::new(0, 10),
            Idx::INT,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        );
        assert_eq!(error.code(), ori_diagnostic::ErrorCode::E2001);
    }

    #[test]
    fn code_for_unknown_ident() {
        let error = TypeCheckError::unknown_ident(Span::new(0, 5), Name::from_raw(1), vec![]);
        assert_eq!(error.code(), ori_diagnostic::ErrorCode::E2003);
    }

    #[test]
    fn span_method_matches_field() {
        let error = TypeCheckError::mismatch(
            Span::new(10, 20),
            Idx::INT,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        );
        assert_eq!(error.span(), error.span);
    }

    // ====================================================================
    // format_message_rich tests
    // ====================================================================

    fn identity_type(idx: Idx) -> String {
        idx.display_name().to_string()
    }

    fn test_name_resolver(name: Name) -> String {
        match name.raw() {
            1 => "foo".to_string(),
            2 => "bar".to_string(),
            3 => "baz".to_string(),
            10 => "MyStruct".to_string(),
            11 => "length".to_string(),
            12 => "width".to_string(),
            20 => "Http".to_string(),
            30 => "Iter".to_string(),
            31 => "Container".to_string(),
            _ => format!("<name:{}>", name.raw()),
        }
    }

    #[test]
    fn rich_message_unknown_ident_with_name() {
        let error = TypeCheckError::unknown_ident(Span::new(0, 3), Name::from_raw(1), vec![]);
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(msg, "unknown identifier `foo`");
    }

    #[test]
    fn rich_message_unknown_ident_with_suggestions() {
        let error = TypeCheckError::unknown_ident(
            Span::new(0, 3),
            Name::from_raw(1),
            vec![Name::from_raw(2), Name::from_raw(3)],
        );
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(
            msg,
            "unknown identifier `foo`; did you mean `bar` or `baz`?"
        );
    }

    #[test]
    fn rich_message_mismatch_primitives() {
        let error = TypeCheckError::mismatch(
            Span::new(0, 10),
            Idx::INT,
            Idx::STR,
            vec![],
            ErrorContext::default(),
        );
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(msg, "type mismatch: expected `int`, found `str`");
    }

    #[test]
    fn rich_message_undefined_field() {
        let error = TypeCheckError::undefined_field(
            Span::new(0, 5),
            Idx::INT,
            Name::from_raw(11),
            vec![Name::from_raw(12)],
        );
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(msg, "no such field `length` on type `int`");
    }

    #[test]
    fn rich_message_missing_capability() {
        let error = TypeCheckError::missing_capability(Span::new(0, 5), Name::from_raw(20), &[]);
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(msg, "missing required capability `Http`");
    }

    #[test]
    fn rich_message_missing_fields() {
        let error = TypeCheckError::missing_fields(
            Span::new(0, 10),
            Name::from_raw(10),
            vec![Name::from_raw(11), Name::from_raw(12)],
        );
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(
            msg,
            "missing 2 required fields in `MyStruct`: `length`, `width`"
        );
    }

    #[test]
    fn rich_message_duplicate_field() {
        let error = TypeCheckError::duplicate_field(
            Span::new(0, 5),
            Name::from_raw(10),
            Name::from_raw(11),
        );
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(msg, "duplicate field `length` in `MyStruct`");
    }

    #[test]
    fn rich_message_not_a_struct() {
        let error = TypeCheckError::not_a_struct(Span::new(0, 5), Name::from_raw(1));
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(msg, "`foo` is not a struct type");
    }

    #[test]
    fn rich_message_missing_assoc_type() {
        let error = TypeCheckError::missing_assoc_type(
            Span::new(0, 5),
            Name::from_raw(30),
            Name::from_raw(31),
        );
        let msg = error.format_message_rich(&identity_type, &test_name_resolver);
        assert_eq!(
            msg,
            "missing associated type `Iter` in impl for `Container`"
        );
    }

    #[test]
    fn format_with_uses_pool_and_interner() {
        let pool = crate::Pool::new();
        let interner = ori_ir::StringInterner::new();
        let name = interner.intern("my_var");

        let error = TypeCheckError::unknown_ident(Span::new(0, 6), name, vec![]);
        let msg = error.format_with(&pool, &interner);
        assert_eq!(msg, "unknown identifier `my_var`");
    }
}
