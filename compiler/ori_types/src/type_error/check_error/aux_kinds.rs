//! Satellite kind enums + error context for [`super::kind::TypeErrorKind`].
//!
//! Holds the small classifier enums ([`AmbiguousTypeSite`],
//! [`OrBindingMismatchReason`], [`VoidLoopKind`], [`NonCollectingLoopKind`],
//! [`ArityMismatchKind`]), the [`ImportErrorKind`] re-export, and
//! [`ErrorContext`] — the satellites of the core `TypeErrorKind` enum.

use crate::type_error::{ContextKind, ExpectedOrigin};
use crate::Idx;

/// Classifies the expression position where an unresolved type variable was
/// observed — drives specialized E2005 diagnostic wording.
///
/// The producer (`validate_body_types` in `check/validators/mod.rs`) inspects
/// the `ExprKind` at the error site and selects the variant. The consumer
/// (`TypeCheckError::message()` in `check_error/message.rs`) dispatches on
/// the variant to produce actionable, site-specific message text. The
/// structured form (vs. a free-form `String`) is SSOT — message wording lives
/// in exactly one match arm.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum AmbiguousTypeSite {
    /// Generic fallback — used for signature-position vars (no `ExprKind`
    /// in scope) and for any body-expression `ExprKind` not specifically
    /// classified below.
    Expression,
    /// Empty list literal `[]` (or `ListWithSpread([])` — spec-canonical
    /// forms for an empty-literal list site). For map/set
    /// literals stay on the generic fallback; this variant is list-only.
    EmptyList,
    /// Closure parameter position within a `Lambda` expression whose
    /// parameter type could not be inferred from the receiver/context.
    /// Signals the user should write the full `typed_lambda` form
    /// (`(x: T) -> R = body`) per the grammar.
    LambdaParam,
}

/// Why an or-pattern's alternatives have inconsistent bindings.
///
/// Selects the error code: `NameDivergence` → E2052, `TypeDivergence` → E2053.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum OrBindingMismatchReason {
    /// A name is bound on only some alternatives (rustc E0408 analog) → E2052.
    NameDivergence,
    /// A name is bound on all alternatives but at divergent types → E2053.
    TypeDivergence {
        /// The type the name carries in the first (canonical) alternative.
        found: Idx,
        /// The divergent type in the conflicting alternative.
        other: Idx,
    },
}

impl OrBindingMismatchReason {
    /// Actionable diagnostic message for this divergence (Spec: Clause 15).
    pub fn message(self) -> &'static str {
        match self {
            Self::NameDivergence => "or-pattern alternatives bind different variables: every `|` alternative must bind the same set of names so the arm body is sound on every match",
            Self::TypeDivergence { .. } => "or-pattern alternatives bind a variable at different types: a name shared across `|` alternatives must have the same type in each",
        }
    }
}

/// Void-typed loop forms that reject `break value` (E0860).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum VoidLoopKind {
    /// `while cond do body`.
    While,
    /// `for x in iter do body`.
    ForDo,
}

impl VoidLoopKind {
    /// The surface keyword form for diagnostic messages.
    pub fn description(self) -> &'static str {
        match self {
            Self::While => "`while...do`",
            Self::ForDo => "`for...do`",
        }
    }
}

/// Non-collecting loop forms that reject `continue value` (E0861).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum NonCollectingLoopKind {
    /// `loop { body }`.
    Loop,
    /// `while cond do body`.
    While,
    /// `for x in iter do body`.
    ForDo,
}

impl NonCollectingLoopKind {
    /// The surface keyword form for diagnostic messages.
    pub fn description(self) -> &'static str {
        match self {
            Self::Loop => "`loop`",
            Self::While => "`while`",
            Self::ForDo => "`for...do`",
        }
    }
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
