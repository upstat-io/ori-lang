//! Type error kind definitions.
//!
//! Contains [`TypeErrorKind`] (the enum of all type check error variants),
//! [`ArityMismatchKind`], [`ErrorContext`], and the [`ImportErrorKind`] re-export.

use ori_ir::{DerivedTrait, Name, Span};

use crate::type_error::{ContextKind, ExpectedOrigin, TypeProblem};
use crate::{Idx, ObjectSafetyViolation};

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
        /// What kind of thing has wrong arity.
        kind: ArityMismatchKind,
        /// Optional function name for better messages.
        func_name: Option<String>,
    },

    /// Missing required capability.
    MissingCapability {
        /// The capability that's needed.
        required: Name,
        /// Capabilities the function currently has.
        available: Vec<Name>,
    },

    /// Infinite (self-referential) type detected.
    InfiniteType {
        /// Optional variable name involved.
        var_name: Option<Name>,
    },

    /// Cannot infer type — needs annotation.
    AmbiguousType {
        /// Variable ID that couldn't be resolved.
        var_id: u32,
        /// Description of where the ambiguity occurred.
        context: String,
    },

    /// Pattern type doesn't match scrutinee type.
    PatternMismatch {
        /// Expected type from scrutinee.
        expected: Idx,
        /// Type from pattern.
        found: Idx,
    },

    /// Match expression doesn't cover all variants.
    NonExhaustiveMatch {
        /// Names of missing patterns.
        missing: Vec<String>,
    },

    /// Rigid type variable cannot unify with concrete type.
    RigidMismatch {
        /// Name of the type variable.
        name: Name,
        /// The concrete type it can't unify with.
        concrete: Idx,
    },

    /// Import resolution error.
    ImportError {
        /// What kind of import error (uses `ori_ir::ImportErrorKind`).
        kind: ori_ir::ImportErrorKind,
        /// Human-readable error message.
        message: String,
    },

    /// Missing associated type in impl block.
    MissingAssocType {
        /// Name of the missing associated type.
        assoc_name: Name,
        /// Trait requiring the associated type.
        trait_name: Name,
    },

    /// Unsatisfied trait bound.
    UnsatisfiedBound {
        /// Description of the unsatisfied bound.
        message: String,
    },

    /// Struct literal used with non-struct type.
    NotAStruct {
        /// Name that was used with struct syntax.
        name: Name,
    },

    /// Missing required fields in struct literal.
    MissingFields {
        /// Struct type name.
        struct_name: Name,
        /// Fields that were not provided.
        fields: Vec<Name>,
    },

    /// Duplicate field in struct literal.
    DuplicateField {
        /// Struct type name.
        struct_name: Name,
        /// Field that appeared twice.
        field: Name,
    },

    /// `Never` used as struct field type (E2019).
    UninhabitedStructField {
        /// Struct type name.
        struct_name: Name,
        /// Field with Never type.
        field: Name,
    },

    /// Operator not supported for type (E2020).
    UnsupportedOperator {
        /// Type the operator was used on.
        ty: Idx,
        /// The operator (e.g., "+", "-").
        op: &'static str,
        /// The trait that would need to be implemented.
        trait_name: &'static str,
    },

    /// Duplicate trait impl for same type (E2010).
    DuplicateImpl {
        /// Trait being implemented.
        trait_name: Name,
        /// Span of the first implementation.
        first_span: Span,
    },

    /// Overlapping impls with equal specificity (E2021).
    OverlappingImpls {
        /// Trait being implemented.
        trait_name: Name,
        /// Span of the first implementation.
        first_span: Span,
    },

    /// Conflicting default methods from multiple super-traits (E2022).
    ConflictingDefaults {
        /// The method with conflicting defaults.
        method: Name,
        /// First trait providing a default.
        trait_a: Name,
        /// Second trait providing a different default.
        trait_b: Name,
    },

    /// Ambiguous method — multiple traits provide it (E2023).
    AmbiguousMethod {
        /// The ambiguous method name.
        method: Name,
        /// Traits that provide the method.
        candidates: Vec<Name>,
    },

    /// Trait cannot be made into an object (E2024).
    NotObjectSafe {
        /// Trait that isn't object-safe.
        trait_name: Name,
        /// Why it isn't object-safe.
        violations: Vec<ObjectSafetyViolation>,
    },

    /// Type doesn't support indexing (E2025).
    NotIndexable {
        /// Type that was indexed.
        ty: Idx,
    },

    /// Wrong key type for indexing (E2026).
    IndexKeyMismatch {
        /// Type being indexed.
        ty: Idx,
        /// Expected key type.
        expected_key: Idx,
        /// Actual key type.
        found_key: Idx,
    },

    /// Ambiguous index — multiple Index impls (E2027).
    AmbiguousIndex {
        /// Type with multiple Index impls.
        ty: Idx,
    },

    /// Cannot derive trait for sum type (E2028).
    CannotDeriveForSumType {
        /// Type name.
        type_name: Name,
        /// Which trait was being derived.
        trait_kind: DerivedTrait,
    },

    /// Cannot derive trait without required supertrait (E2029).
    CannotDeriveWithoutSupertrait {
        /// Type name.
        type_name: Name,
        /// Trait being derived.
        trait_kind: DerivedTrait,
        /// Required supertrait that's missing.
        required: DerivedTrait,
    },

    /// Hash invariant may be violated (E2030).
    HashInvariantViolation {
        /// Type whose hash invariant may be broken.
        type_name: Name,
    },

    /// Non-hashable type used as map key (E2031).
    NonHashableMapKey {
        /// Type used as key that isn't Hashable.
        key_type: Idx,
    },

    /// Field type doesn't implement trait required by derive (E2032).
    FieldMissingTraitInDerive {
        /// Struct type name.
        type_name: Name,
        /// Trait being derived.
        trait_name: Name,
        /// Field that's missing the trait.
        field_name: Name,
        /// Field's type.
        field_type: Idx,
    },

    /// Trait cannot be automatically derived (E2033).
    TraitNotDerivable {
        /// Trait that was attempted to derive.
        trait_name: Name,
    },

    /// Invalid format specification in template string (E2034).
    InvalidFormatSpec {
        /// The format spec string.
        spec: String,
        /// Why it's invalid.
        reason: String,
    },

    /// Format type mismatch (E2035).
    FormatTypeMismatch {
        /// Expression type.
        expr_type: Idx,
        /// Format type string (e.g., "x", "b").
        format_type: String,
        /// What types this format is valid for.
        valid_for: &'static str,
    },

    /// `.into()` not implemented for target type (E2036).
    IntoNotImplemented {
        /// Source type.
        ty: Idx,
        /// Expected target type (if known).
        target: Option<Idx>,
    },

    /// Ambiguous `.into()` — multiple implementations (E2037).
    AmbiguousInto {
        /// Source type.
        ty: Idx,
    },

    /// Missing `Printable` for string interpolation (E2038).
    MissingPrintable {
        /// Type that needs Printable.
        ty: Idx,
    },

    /// Cannot assign to immutable binding (E2039).
    AssignToImmutable {
        /// Name of the immutable binding.
        name: Name,
    },

    /// Feature not yet supported (E2040).
    UnsupportedFeature {
        /// Description of the unsupported feature.
        feature: &'static str,
    },

    /// Invalid `#repr` attribute (E2041).
    InvalidReprAttribute {
        /// The type name this attribute was applied to.
        type_name: Name,
        /// Human-readable reason (e.g., "alignment must be a power of two").
        reason: String,
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
