//! Type Checker Output Types
//!
//! Contains `TypedModule`, `GenericBound`, `FunctionType`, and `TypeCheckError`.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.
//!
//! # `TypeId` Migration
//! This module uses `TypeId` for efficient O(1) type comparisons.
//! Convert to `Type` when needed using `TypeInterner::to_type()`.

use ori_diagnostic::{Diagnostic, ErrorCode, ErrorGuaranteed};
use ori_ir::{Name, Span, StringInterner, TypeId};

/// Type-checked module.
///
/// Uses `TypeId` internally for O(1) type equality comparisons.
/// Convert to `Type` using a `TypeInterner` when needed.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypedModule {
    /// Type of each expression (indexed by `ExprId`), stored as `TypeId` for efficiency.
    pub expr_types: Vec<TypeId>,
    /// Type of each function.
    pub function_types: Vec<FunctionType>,
    /// Type checking errors.
    pub errors: Vec<TypeCheckError>,
    /// Type-level proof that errors were emitted.
    ///
    /// `Some(guarantee)` if at least one error was emitted during type checking,
    /// `None` if type checking succeeded without errors.
    ///
    /// This provides a compile-time guarantee that error reporting was not forgotten.
    pub error_guarantee: Option<ErrorGuaranteed>,
}

impl TypedModule {
    /// Check if this module has type errors.
    ///
    /// Returns `true` if any errors were emitted during type checking.
    /// Prefer using `error_guarantee` for pattern matching when you need
    /// to prove that errors exist at the type level.
    pub fn has_errors(&self) -> bool {
        self.error_guarantee.is_some()
    }
}

/// A generic parameter with its trait bounds and associated type variable.
#[derive(Clone, Debug)]
pub struct GenericBound {
    /// The generic parameter name (e.g., `T` in `<T: Eq>`)
    pub param: Name,
    /// Trait bounds as paths (e.g., `["Eq"]`, `["Comparable"]`)
    pub bounds: Vec<Vec<Name>>,
    /// The type variable used for this generic in the function signature (as `TypeId`).
    /// Used to resolve the actual type at call sites for constraint checking.
    pub type_var: TypeId,
}

// Manual Eq/PartialEq/Hash implementations that exclude `type_var`.
//
// **Why this is necessary for Salsa compliance:**
//
// The `type_var` field contains a fresh type variable ID created during
// type checking. These IDs are unique per type-check invocation, meaning
// two equivalent generic bounds would have different `type_var` values.
//
// If we derived Eq/Hash including type_var, Salsa would see "different"
// function types across invocations, causing unnecessary cache misses
// and recomputation. By excluding type_var from equality, we ensure
// that logically equivalent generic bounds are treated as equal.
//
// The type_var is only used for resolving constraints at call sites
// and doesn't affect the semantic identity of the bound.
impl PartialEq for GenericBound {
    fn eq(&self, other: &Self) -> bool {
        self.param == other.param && self.bounds == other.bounds
    }
}

impl Eq for GenericBound {}

impl std::hash::Hash for GenericBound {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.param.hash(state);
        self.bounds.hash(state);
    }
}

/// A where clause constraint, potentially with an associated type projection.
///
/// Examples:
/// - `where T: Clone` → param=T, projection=None, bounds=[Clone]
/// - `where C.Item: Eq` → param=C, projection=Some(Item), bounds=[Eq]
#[derive(Clone, Debug)]
pub struct WhereConstraint {
    /// The type parameter being constrained (e.g., `T` or `C`).
    pub param: Name,
    /// Optional associated type projection (e.g., `Item` in `C.Item: Eq`).
    pub projection: Option<Name>,
    /// Trait bounds as paths (e.g., `["Eq"]`, `["Comparable"]`).
    pub bounds: Vec<Vec<Name>>,
    /// The type variable for the base parameter (as `TypeId`, for resolving at call sites).
    pub type_var: TypeId,
}

// Manual Eq/PartialEq/Hash that ignores type_var.
// See GenericBound above for rationale (Salsa cache coherence).
impl PartialEq for WhereConstraint {
    fn eq(&self, other: &Self) -> bool {
        self.param == other.param
            && self.projection == other.projection
            && self.bounds == other.bounds
    }
}

impl Eq for WhereConstraint {}

impl std::hash::Hash for WhereConstraint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.param.hash(state);
        self.projection.hash(state);
        self.bounds.hash(state);
    }
}

/// Function type information.
///
/// Uses `TypeId` for params and `return_type` for O(1) type comparisons.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionType {
    pub name: Name,
    /// Generic parameters with their trait bounds
    pub generics: Vec<GenericBound>,
    /// Where clause constraints (may include associated type projections).
    pub where_constraints: Vec<WhereConstraint>,
    /// Parameter types (as `TypeId` for efficiency)
    pub params: Vec<TypeId>,
    /// Return type (as `TypeId` for efficiency)
    pub return_type: TypeId,
    /// Capabilities required by this function (from `uses` clause)
    pub capabilities: Vec<Name>,
}

/// Type checking error with structured information.
///
/// Each variant captures the specific data needed for that error type,
/// enabling precise diagnostics and future improvements like suggestions.
///
/// # Salsa Compatibility
/// Has Clone, Eq, `PartialEq`, Hash, Debug for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeCheckError {
    // ===== Type Mismatches =====
    /// Type mismatch between expected and found types.
    TypeMismatch {
        span: Span,
        expected: String,
        found: String,
    },

    /// Return type mismatch in function.
    ReturnTypeMismatch {
        span: Span,
        expected: String,
        found: String,
        func_name: String,
    },

    /// Match arms have incompatible types.
    MatchArmTypeMismatch {
        span: Span,
        first_type: String,
        this_type: String,
        first_span: Span,
    },

    /// Pattern type doesn't match scrutinee.
    PatternTypeMismatch {
        span: Span,
        expected: String,
        found: String,
    },

    // ===== Function Call Errors =====
    /// Argument count mismatch in function call.
    ArgCountMismatch {
        span: Span,
        expected: usize,
        found: usize,
        func_name: Option<String>,
    },

    /// Named arguments required for function call.
    NamedArgsRequired {
        span: Span,
        func_name: Option<String>,
    },

    /// Type is not callable.
    NotCallable { span: Span, found_type: String },

    /// Missing capability for function call.
    MissingCapability {
        span: Span,
        func_name: String,
        capability: String,
    },

    // ===== Identifier/Resolution Errors =====
    /// Unknown identifier.
    UnknownIdentifier {
        span: Span,
        name: String,
        suggestion: Option<String>,
    },

    /// Unknown type name.
    UnknownType {
        span: Span,
        name: String,
        suggestion: Option<String>,
    },

    /// Unknown function.
    UnknownFunction {
        span: Span,
        name: String,
        suggestion: Option<String>,
    },

    /// Undefined config variable.
    UndefinedConfig { span: Span, name: String },

    /// Self used outside impl block.
    SelfOutsideImpl { span: Span },

    // ===== Field/Method Access Errors =====
    /// Type doesn't have the accessed field.
    NoSuchField {
        span: Span,
        type_name: String,
        field_name: String,
        suggestion: Option<String>,
    },

    /// Type doesn't have the called method.
    NoSuchMethod {
        span: Span,
        type_name: String,
        method_name: String,
        suggestion: Option<String>,
    },

    /// Type doesn't support field access.
    FieldAccessNotSupported {
        span: Span,
        type_name: String,
        hint: Option<String>,
    },

    /// Type is not indexable.
    NotIndexable {
        span: Span,
        found_type: String,
        hint: Option<String>,
    },

    /// Module has no exported item.
    NoSuchExport { span: Span, item_name: String },

    // ===== Operator Errors =====
    /// Binary operation not supported for types.
    InvalidBinaryOp {
        span: Span,
        op: String,
        left_type: String,
        right_type: String,
    },

    /// Unary operation not supported for type.
    InvalidUnaryOp {
        span: Span,
        op: String,
        operand_type: String,
    },

    /// Mismatched types for operator trait.
    OperatorTypeMismatch {
        span: Span,
        trait_name: String,
        expected: String,
        found: String,
    },

    // ===== Control Flow Errors =====
    /// Type is not iterable.
    NotIterable { span: Span, found_type: String },

    /// Try operator used on non-Result/Option type.
    InvalidTryOperand { span: Span, found_type: String },

    /// Await is not supported.
    AwaitNotSupported { span: Span },

    /// Condition must be bool.
    ConditionNotBool { span: Span, found_type: String },

    // ===== Struct Errors =====
    /// Unknown struct type.
    UnknownStruct {
        span: Span,
        name: String,
        suggestion: Option<String>,
    },

    /// Type is not a struct.
    NotAStruct { span: Span, name: String },

    /// Duplicate field in struct literal.
    DuplicateField { span: Span, field_name: String },

    /// Missing field in struct literal.
    MissingField {
        span: Span,
        struct_name: String,
        field_name: String,
    },

    // ===== Pattern Errors =====
    /// Tuple pattern length mismatch.
    TupleLengthMismatch {
        span: Span,
        expected: usize,
        found: usize,
    },

    /// List pattern cannot match type.
    ListPatternMismatch { span: Span, found_type: String },

    /// Tuple pattern cannot match type.
    TuplePatternMismatch { span: Span, found_type: String },

    /// Invalid pattern for variant.
    InvalidVariantPattern {
        span: Span,
        variant_name: String,
        scrutinee_type: String,
    },

    // ===== Inference Errors =====
    /// Cannot infer type.
    CannotInfer { span: Span, context: String },

    /// Infinite type detected (occurs check failure).
    InfiniteType { span: Span },

    // ===== Trait/Impl Errors =====
    /// Type does not satisfy trait bound.
    BoundNotSatisfied {
        span: Span,
        type_name: String,
        bound_name: String,
        generic_name: Option<String>,
    },

    /// Provider does not implement capability.
    CapabilityNotImplemented {
        span: Span,
        provider_type: String,
        capability: String,
    },

    /// Unknown capability.
    UnknownCapability { span: Span, name: String },

    /// Coherence violation (duplicate impl).
    CoherenceViolation {
        span: Span,
        message: String,
        existing_span: Span,
    },

    /// Missing associated type in impl.
    MissingAssocType {
        span: Span,
        trait_name: String,
        type_name: String,
        assoc_name: String,
    },

    /// Missing type argument for trait.
    MissingTypeArg {
        span: Span,
        trait_name: String,
        param_name: String,
    },

    /// Too many type arguments.
    TooManyTypeArgs {
        span: Span,
        trait_name: String,
        expected: usize,
        found: usize,
    },

    /// Trait not found.
    TraitNotFound { span: Span, name: String },

    /// Type parameter ordering error.
    TypeParamOrdering {
        span: Span,
        non_default_param: String,
        default_param: String,
    },

    // ===== Closure/Cycle Errors =====
    /// Closure cannot capture itself.
    ClosureSelfCapture { span: Span, name: String },

    /// Cyclic type definition.
    CyclicType { span: Span, type_name: String },

    // ===== Generic/Fallback =====
    /// Generic type error with custom message.
    ///
    /// This variant is used for errors that don't fit other categories
    /// or when transitioning from the old string-based errors.
    /// Prefer using specific variants when possible.
    Generic {
        span: Span,
        message: String,
        code: ErrorCode,
        suggestion: Option<String>,
    },
}

impl TypeCheckError {
    /// Get the primary span of this error.
    pub fn span(&self) -> Span {
        match self {
            // Type mismatches
            TypeCheckError::TypeMismatch { span, .. }
            | TypeCheckError::ReturnTypeMismatch { span, .. }
            | TypeCheckError::MatchArmTypeMismatch { span, .. }
            | TypeCheckError::PatternTypeMismatch { span, .. }
            // Function calls
            | TypeCheckError::ArgCountMismatch { span, .. }
            | TypeCheckError::NamedArgsRequired { span, .. }
            | TypeCheckError::NotCallable { span, .. }
            | TypeCheckError::MissingCapability { span, .. }
            // Identifiers
            | TypeCheckError::UnknownIdentifier { span, .. }
            | TypeCheckError::UnknownType { span, .. }
            | TypeCheckError::UnknownFunction { span, .. }
            | TypeCheckError::UndefinedConfig { span, .. }
            | TypeCheckError::SelfOutsideImpl { span }
            // Field/method access
            | TypeCheckError::NoSuchField { span, .. }
            | TypeCheckError::NoSuchMethod { span, .. }
            | TypeCheckError::FieldAccessNotSupported { span, .. }
            | TypeCheckError::NotIndexable { span, .. }
            | TypeCheckError::NoSuchExport { span, .. }
            // Operators
            | TypeCheckError::InvalidBinaryOp { span, .. }
            | TypeCheckError::InvalidUnaryOp { span, .. }
            | TypeCheckError::OperatorTypeMismatch { span, .. }
            // Control flow
            | TypeCheckError::NotIterable { span, .. }
            | TypeCheckError::InvalidTryOperand { span, .. }
            | TypeCheckError::AwaitNotSupported { span }
            | TypeCheckError::ConditionNotBool { span, .. }
            // Structs
            | TypeCheckError::UnknownStruct { span, .. }
            | TypeCheckError::NotAStruct { span, .. }
            | TypeCheckError::DuplicateField { span, .. }
            | TypeCheckError::MissingField { span, .. }
            // Patterns
            | TypeCheckError::TupleLengthMismatch { span, .. }
            | TypeCheckError::ListPatternMismatch { span, .. }
            | TypeCheckError::TuplePatternMismatch { span, .. }
            | TypeCheckError::InvalidVariantPattern { span, .. }
            // Inference
            | TypeCheckError::CannotInfer { span, .. }
            | TypeCheckError::InfiniteType { span }
            // Traits/impls
            | TypeCheckError::BoundNotSatisfied { span, .. }
            | TypeCheckError::CapabilityNotImplemented { span, .. }
            | TypeCheckError::UnknownCapability { span, .. }
            | TypeCheckError::CoherenceViolation { span, .. }
            | TypeCheckError::MissingAssocType { span, .. }
            | TypeCheckError::MissingTypeArg { span, .. }
            | TypeCheckError::TooManyTypeArgs { span, .. }
            | TypeCheckError::TraitNotFound { span, .. }
            | TypeCheckError::TypeParamOrdering { span, .. }
            // Closure/cycle
            | TypeCheckError::ClosureSelfCapture { span, .. }
            | TypeCheckError::CyclicType { span, .. }
            // Generic
            | TypeCheckError::Generic { span, .. } => *span,
        }
    }

    /// Get the error code for this error.
    pub fn code(&self) -> ErrorCode {
        match self {
            // E2001: Type mismatch / general type error
            TypeCheckError::TypeMismatch { .. }
            | TypeCheckError::ReturnTypeMismatch { .. }
            | TypeCheckError::MatchArmTypeMismatch { .. }
            | TypeCheckError::PatternTypeMismatch { .. }
            | TypeCheckError::NotCallable { .. }
            | TypeCheckError::FieldAccessNotSupported { .. }
            | TypeCheckError::NotIndexable { .. }
            | TypeCheckError::InvalidBinaryOp { .. }
            | TypeCheckError::InvalidUnaryOp { .. }
            | TypeCheckError::OperatorTypeMismatch { .. }
            | TypeCheckError::NotIterable { .. }
            | TypeCheckError::InvalidTryOperand { .. }
            | TypeCheckError::AwaitNotSupported { .. }
            | TypeCheckError::ConditionNotBool { .. }
            | TypeCheckError::NotAStruct { .. }
            | TypeCheckError::DuplicateField { .. }
            | TypeCheckError::MissingField { .. }
            | TypeCheckError::TupleLengthMismatch { .. }
            | TypeCheckError::ListPatternMismatch { .. }
            | TypeCheckError::TuplePatternMismatch { .. }
            | TypeCheckError::InvalidVariantPattern { .. } => ErrorCode::E2001,

            // E2002: Method not found
            TypeCheckError::NoSuchMethod { .. } => ErrorCode::E2002,

            // E2003: Unknown identifier/type/field
            TypeCheckError::UnknownIdentifier { .. }
            | TypeCheckError::UnknownType { .. }
            | TypeCheckError::UnknownFunction { .. }
            | TypeCheckError::NoSuchField { .. }
            | TypeCheckError::NoSuchExport { .. }
            | TypeCheckError::UnknownStruct { .. }
            | TypeCheckError::SelfOutsideImpl { .. }
            | TypeCheckError::TraitNotFound { .. } => ErrorCode::E2003,

            // E2004: Argument errors
            TypeCheckError::ArgCountMismatch { .. } | TypeCheckError::UndefinedConfig { .. } => {
                ErrorCode::E2004
            }

            // E2005: Cannot infer
            TypeCheckError::CannotInfer { .. } => ErrorCode::E2005,

            // E2006: Duplicate definition (used for coherence too)
            TypeCheckError::CoherenceViolation { .. } => ErrorCode::E2006,

            // E2007: Self-reference error
            TypeCheckError::ClosureSelfCapture { .. } => ErrorCode::E2007,

            // E2008: Infinite/cyclic type
            TypeCheckError::InfiniteType { .. } | TypeCheckError::CyclicType { .. } => {
                ErrorCode::E2008
            }

            // E2009: Bound not satisfied
            TypeCheckError::BoundNotSatisfied { .. } => ErrorCode::E2009,

            // E2010: Coherence violation (duplicate impl)
            // Already handled above with E2006

            // E2011: Named arguments required
            TypeCheckError::NamedArgsRequired { .. } => ErrorCode::E2011,

            // E2012: Unknown capability
            TypeCheckError::UnknownCapability { .. } => ErrorCode::E2012,

            // E2013: Capability not implemented
            TypeCheckError::CapabilityNotImplemented { .. } => ErrorCode::E2013,

            // E2014: Missing capability
            TypeCheckError::MissingCapability { .. } => ErrorCode::E2014,

            // E2015: Type parameter ordering
            TypeCheckError::TypeParamOrdering { .. } => ErrorCode::E2015,

            // E2016: Missing type argument
            TypeCheckError::MissingTypeArg { .. } => ErrorCode::E2016,

            // E2017: Too many type arguments
            TypeCheckError::TooManyTypeArgs { .. } => ErrorCode::E2017,

            // E2018: Missing associated type
            TypeCheckError::MissingAssocType { .. } => ErrorCode::E2018,

            // Generic uses its stored code
            TypeCheckError::Generic { code, .. } => *code,
        }
    }

    /// Check if this is a soft error that can be suppressed after hard errors.
    ///
    /// Soft errors are typically inference failures that result from
    /// earlier errors propagating through the type system.
    pub fn is_soft(&self) -> bool {
        match self {
            // Cannot infer errors are often caused by earlier errors
            TypeCheckError::CannotInfer { .. } => true,
            // Check for <error> type in various fields
            TypeCheckError::TypeMismatch {
                expected, found, ..
            } => expected.contains("<error>") || found.contains("<error>"),
            TypeCheckError::InvalidBinaryOp {
                left_type,
                right_type,
                ..
            } => left_type.contains("<error>") || right_type.contains("<error>"),
            TypeCheckError::InvalidUnaryOp { operand_type, .. } => operand_type.contains("<error>"),
            TypeCheckError::NoSuchMethod { type_name, .. }
            | TypeCheckError::NoSuchField { type_name, .. }
            | TypeCheckError::NotIndexable {
                found_type: type_name,
                ..
            }
            | TypeCheckError::NotIterable {
                found_type: type_name,
                ..
            }
            | TypeCheckError::NotCallable {
                found_type: type_name,
                ..
            } => type_name.contains("<error>"),
            TypeCheckError::Generic { message, .. } => message.contains("<error>"),
            _ => false,
        }
    }

    /// Check if this is a follow-on error resulting from previous errors.
    ///
    /// Follow-on errors contain types like `<error>` or phrases indicating
    /// they're a consequence of earlier type errors.
    pub fn is_follow_on(&self) -> bool {
        // Check message content for error indicators
        let contains_error_type = |s: &str| {
            s.contains("<error>") || s.contains("invalid operand") || s.contains("invalid type")
        };

        match self {
            TypeCheckError::TypeMismatch {
                expected, found, ..
            } => contains_error_type(expected) || contains_error_type(found),
            TypeCheckError::InvalidBinaryOp {
                left_type,
                right_type,
                ..
            } => contains_error_type(left_type) || contains_error_type(right_type),
            TypeCheckError::InvalidUnaryOp { operand_type, .. } => {
                contains_error_type(operand_type)
            }
            TypeCheckError::Generic { message, .. } => contains_error_type(message),
            _ => false,
        }
    }

    /// Convert this error to a diagnostic without an interner.
    ///
    /// This is a convenience method for code that doesn't have access
    /// to a `StringInterner`. Since we currently store Strings (not Names),
    /// an interner isn't actually needed yet.
    pub fn to_diagnostic(&self) -> Diagnostic {
        // Use the interner-taking version with a temporary interner
        // This will be simplified once we remove the interner parameter
        let interner = StringInterner::new();
        self.to_diagnostic_with_interner(&interner)
    }

    /// Convert this error to a diagnostic with an interner.
    ///
    /// The interner parameter is for future Name field lookups
    /// (currently unused as we store Strings).
    #[expect(
        unused_variables,
        reason = "interner reserved for future Name field conversions"
    )]
    pub fn to_diagnostic_with_interner(&self, interner: &StringInterner) -> Diagnostic {
        match self {
            // ===== Type Mismatches =====
            TypeCheckError::TypeMismatch { span, expected, found } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("type mismatch: expected `{expected}`, found `{found}`"))
                    .with_label(*span, format!("expected `{expected}`"))
            }

            TypeCheckError::ReturnTypeMismatch { span, expected, found, func_name } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "return type mismatch in `{func_name}`: expected `{expected}`, found `{found}`"
                    ))
                    .with_label(*span, format!("expected `{expected}`"))
            }

            TypeCheckError::MatchArmTypeMismatch { span, first_type, this_type, first_span } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "match arms have incompatible types: `{first_type}` vs `{this_type}`"
                    ))
                    .with_label(*span, format!("expected `{first_type}`"))
                    .with_secondary_label(*first_span, "first arm has this type")
            }

            TypeCheckError::PatternTypeMismatch { span, expected, found } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "pattern type mismatch: expected `{expected}`, found `{found}`"
                    ))
                    .with_label(*span, format!("expected `{expected}`"))
            }

            // ===== Function Call Errors =====
            TypeCheckError::ArgCountMismatch { span, expected, found, func_name } => {
                let plural = if *expected == 1 { "" } else { "s" };
                let message = match func_name {
                    Some(name) => format!(
                        "function `{name}` expects {expected} argument{plural}, found {found}"
                    ),
                    None => format!("expected {expected} argument{plural}, found {found}"),
                };
                Diagnostic::error(ErrorCode::E2004)
                    .with_message(message)
                    .with_label(*span, format!("expected {expected} argument{plural}"))
                    .with_suggestion(if *found > *expected {
                        "remove extra arguments"
                    } else {
                        "add missing arguments"
                    })
            }

            TypeCheckError::NamedArgsRequired { span, func_name } => {
                let message = match func_name {
                    Some(name) => format!(
                        "named arguments required when calling `@{name}` (use name: value syntax)"
                    ),
                    None => "named arguments required for function calls (use name: value syntax)".to_string(),
                };
                Diagnostic::error(ErrorCode::E2011)
                    .with_message(message)
                    .with_label(*span, "use named arguments")
            }

            TypeCheckError::NotCallable { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("`{found_type}` is not callable"))
                    .with_label(*span, "cannot call this as a function")
                    .with_note("only functions and lambdas can be called")
            }

            TypeCheckError::MissingCapability { span, func_name, capability } => {
                Diagnostic::error(ErrorCode::E2014)
                    .with_message(format!(
                        "function `{func_name}` uses `{capability}` capability, but caller does not declare or provide it"
                    ))
                    .with_label(*span, format!("requires `{capability}`"))
                    .with_suggestion(format!("add `uses {capability}` to the caller"))
            }

            // ===== Identifier/Resolution Errors =====
            TypeCheckError::UnknownIdentifier { span, name, suggestion } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown identifier `{name}`"))
                    .with_label(*span, "not found in this scope");
                if let Some(suggest) = suggestion {
                    diag = diag.with_suggestion(format!("try using `{suggest}`"));
                }
                diag
            }

            TypeCheckError::UnknownType { span, name, suggestion } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown type `{name}`"))
                    .with_label(*span, format!("type `{name}` not found"));
                if let Some(suggest) = suggestion {
                    diag = diag.with_suggestion(format!("try using `{suggest}`"));
                }
                diag.with_note("check the type name is spelled correctly and imported")
            }

            TypeCheckError::UnknownFunction { span, name, suggestion } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown function `@{name}`"))
                    .with_label(*span, "function not found");
                if let Some(suggest) = suggestion {
                    diag = diag.with_suggestion(format!("try using `@{suggest}`"));
                }
                diag
            }

            TypeCheckError::UndefinedConfig { span, name } => {
                Diagnostic::error(ErrorCode::E2004)
                    .with_message(format!("undefined config variable `${name}`"))
                    .with_label(*span, "config not defined")
            }

            TypeCheckError::SelfOutsideImpl { span } => {
                Diagnostic::error(ErrorCode::E2003)
                    .with_message("`self` can only be used inside impl blocks")
                    .with_label(*span, "invalid use of `self`")
            }

            // ===== Field/Method Access Errors =====
            TypeCheckError::NoSuchField { span, type_name, field_name, suggestion } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("no field `{field_name}` on type `{type_name}`"))
                    .with_label(*span, format!("`{field_name}` is not a field of `{type_name}`"));
                if let Some(suggest) = suggestion {
                    diag = diag.with_suggestion(format!("try using `{suggest}`"));
                }
                diag
            }

            TypeCheckError::NoSuchMethod { span, type_name, method_name, suggestion } => {
                let mut diag = Diagnostic::error(ErrorCode::E2002)
                    .with_message(format!("no method `{method_name}` on type `{type_name}`"))
                    .with_label(*span, format!("`{method_name}` not found on `{type_name}`"));
                if let Some(suggest) = suggestion {
                    diag = diag.with_suggestion(format!("try using `{suggest}`"));
                }
                diag
            }

            TypeCheckError::FieldAccessNotSupported { span, type_name, hint } => {
                let mut diag = Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("type `{type_name}` does not support field access"))
                    .with_label(*span, "field access not supported");
                if let Some(h) = hint {
                    diag = diag.with_note(h.as_str());
                }
                diag
            }

            TypeCheckError::NotIndexable { span, found_type, hint } => {
                let mut diag = Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("type `{found_type}` is not indexable"))
                    .with_label(*span, "indexing not supported");
                if let Some(h) = hint {
                    diag = diag.with_note(h.as_str());
                }
                diag.with_note("only lists, maps, and strings support indexing")
            }

            TypeCheckError::NoSuchExport { span, item_name } => {
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("module has no exported item `{item_name}`"))
                    .with_label(*span, "item not exported")
            }

            // ===== Operator Errors =====
            TypeCheckError::InvalidBinaryOp { span, op, left_type, right_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "cannot apply `{op}` to `{left_type}` and `{right_type}`"
                    ))
                    .with_label(*span, format!("`{op}` not supported for these types"))
            }

            TypeCheckError::InvalidUnaryOp { span, op, operand_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("cannot apply `{op}` to `{operand_type}`"))
                    .with_label(*span, format!("`{op}` not supported for this type"))
            }

            TypeCheckError::OperatorTypeMismatch { span, trait_name, expected, found } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "mismatched types for `{trait_name}` operator: expected `{expected}`, found `{found}`"
                    ))
                    .with_label(*span, format!("expected `{expected}`"))
            }

            // ===== Control Flow Errors =====
            TypeCheckError::NotIterable { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("`{found_type}` is not iterable"))
                    .with_label(*span, "cannot iterate over this")
                    .with_note("expected List, Set, Range, Str, or Map")
            }

            TypeCheckError::InvalidTryOperand { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "the `?` operator can only be applied to `Result` or `Option`, found `{found_type}`"
                    ))
                    .with_label(*span, "not Result or Option")
                    .with_suggestion("wrap the value in Ok() or Some() if it should always succeed")
            }

            TypeCheckError::AwaitNotSupported { span } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message("`.await` is not supported")
                    .with_label(*span, "await not available")
                    .with_suggestion("use `uses Async` capability and `parallel(...)` pattern")
            }

            TypeCheckError::ConditionNotBool { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("condition must be `bool`, found `{found_type}`"))
                    .with_label(*span, "expected `bool`")
                    .with_suggestion("use a comparison operator to get a bool value")
            }

            // ===== Struct Errors =====
            TypeCheckError::UnknownStruct { span, name, suggestion } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown struct type `{name}`"))
                    .with_label(*span, "struct not found");
                if let Some(suggest) = suggestion {
                    diag = diag.with_suggestion(format!("try using `{suggest}`"));
                }
                diag
            }

            TypeCheckError::NotAStruct { span, name } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("`{name}` is not a struct type"))
                    .with_label(*span, "not a struct")
            }

            TypeCheckError::DuplicateField { span, field_name } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("field `{field_name}` specified more than once"))
                    .with_label(*span, "duplicate field")
            }

            TypeCheckError::MissingField { span, struct_name, field_name } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("missing field `{field_name}` in struct `{struct_name}`"))
                    .with_label(*span, "field required")
            }

            // ===== Pattern Errors =====
            TypeCheckError::TupleLengthMismatch { span, expected, found } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "tuple pattern has {found} elements but type has {expected}"
                    ))
                    .with_label(*span, format!("expected {expected} elements"))
            }

            TypeCheckError::ListPatternMismatch { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("list pattern cannot match type `{found_type}`"))
                    .with_label(*span, "expected list type")
            }

            TypeCheckError::TuplePatternMismatch { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("tuple pattern cannot match type `{found_type}`"))
                    .with_label(*span, "expected tuple type")
            }

            TypeCheckError::InvalidVariantPattern { span, variant_name, scrutinee_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "pattern `{variant_name}` is not a valid variant for type `{scrutinee_type}`"
                    ))
                    .with_label(*span, "invalid variant")
            }

            // ===== Inference Errors =====
            TypeCheckError::CannotInfer { span, context } => {
                Diagnostic::error(ErrorCode::E2005)
                    .with_message(format!("cannot infer type for {context}"))
                    .with_label(*span, "type annotation needed")
                    .with_suggestion("add explicit type annotation")
            }

            TypeCheckError::InfiniteType { span } => {
                Diagnostic::error(ErrorCode::E2008)
                    .with_message("infinite type detected")
                    .with_label(*span, "this would create an infinite type")
                    .with_note("a type cannot contain itself")
            }

            // ===== Trait/Impl Errors =====
            TypeCheckError::BoundNotSatisfied { span, type_name, bound_name, generic_name } => {
                let message = match generic_name {
                    Some(gen) => format!(
                        "type `{type_name}` does not satisfy trait bound `{bound_name}` required by generic parameter `{gen}`"
                    ),
                    None => format!(
                        "type `{type_name}` does not satisfy trait bound `{bound_name}`"
                    ),
                };
                Diagnostic::error(ErrorCode::E2009)
                    .with_message(message)
                    .with_label(*span, format!("bound `{bound_name}` not satisfied"))
            }

            TypeCheckError::CapabilityNotImplemented { span, provider_type, capability } => {
                Diagnostic::error(ErrorCode::E2013)
                    .with_message(format!(
                        "provider type `{provider_type}` does not implement capability `{capability}`"
                    ))
                    .with_label(*span, format!("does not implement `{capability}`"))
            }

            TypeCheckError::UnknownCapability { span, name } => {
                Diagnostic::error(ErrorCode::E2012)
                    .with_message(format!(
                        "unknown capability `{name}`: capabilities must be defined traits"
                    ))
                    .with_label(*span, "unknown capability")
            }

            TypeCheckError::CoherenceViolation { span, message, existing_span } => {
                Diagnostic::error(ErrorCode::E2006)
                    .with_message(message.clone())
                    .with_label(*span, "conflicting impl")
                    .with_secondary_label(*existing_span, "previous impl here")
            }

            TypeCheckError::MissingAssocType { span, trait_name, type_name, assoc_name } => {
                Diagnostic::error(ErrorCode::E2018)
                    .with_message(format!(
                        "impl of `{trait_name}` for `{type_name}` missing associated type `{assoc_name}`"
                    ))
                    .with_label(*span, format!("missing `type {assoc_name}`"))
            }

            TypeCheckError::MissingTypeArg { span, trait_name, param_name } => {
                Diagnostic::error(ErrorCode::E2016)
                    .with_message(format!(
                        "impl of `{trait_name}` is missing type argument `{param_name}` which has no default"
                    ))
                    .with_label(*span, "missing type argument")
            }

            TypeCheckError::TooManyTypeArgs { span, trait_name, expected, found } => {
                Diagnostic::error(ErrorCode::E2017)
                    .with_message(format!(
                        "too many type arguments for `{trait_name}`: expected {expected}, found {found}"
                    ))
                    .with_label(*span, format!("expected {expected} type arguments"))
            }

            TypeCheckError::TraitNotFound { span, name } => {
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("trait `{name}` not found"))
                    .with_label(*span, "unknown trait")
            }

            TypeCheckError::TypeParamOrdering { span, non_default_param, default_param } => {
                Diagnostic::error(ErrorCode::E2015)
                    .with_message(format!(
                        "type parameter `{non_default_param}` without default must appear before type parameter `{default_param}` with default"
                    ))
                    .with_label(*span, "invalid parameter order")
            }

            // ===== Closure/Cycle Errors =====
            TypeCheckError::ClosureSelfCapture { span, name } => {
                Diagnostic::error(ErrorCode::E2007)
                    .with_message(format!(
                        "closure cannot capture itself: `{name}` references itself in its body"
                    ))
                    .with_label(*span, "self-reference not allowed")
                    .with_note("closures cannot recursively reference themselves")
            }

            TypeCheckError::CyclicType { span, type_name } => {
                Diagnostic::error(ErrorCode::E2008)
                    .with_message(format!("cyclic type definition for `{type_name}`"))
                    .with_label(*span, "cycle detected here")
            }

            // ===== Generic/Fallback =====
            TypeCheckError::Generic { span, message, code, suggestion } => {
                let mut diag = Diagnostic::error(*code)
                    .with_message(message.clone())
                    .with_label(*span, infer_label(message));
                if let Some(suggest) = suggestion {
                    diag = diag.with_suggestion(suggest.as_str());
                }
                diag
            }
        }
    }

    /// Get the error message (for backwards compatibility).
    pub fn message(&self) -> String {
        self.to_diagnostic().message
    }

    /// Get the optional suggestion (for backwards compatibility).
    pub fn suggestion(&self) -> Option<String> {
        let diag = self.to_diagnostic();
        diag.suggestions.into_iter().next()
    }
}

/// Infer a contextual label from the error message.
fn infer_label(msg: &str) -> &'static str {
    let msg = msg.to_lowercase();
    if msg.contains("cannot infer") || msg.contains("could not infer") {
        "cannot infer type"
    } else if msg.contains("expected") && msg.contains("found") {
        "type mismatch"
    } else if msg.contains("no such field") || msg.contains("has no field") {
        "unknown field"
    } else if msg.contains("unknown type") || msg.contains("undefined type") {
        "unknown type"
    } else if msg.contains("unknown struct") {
        "unknown struct"
    } else if msg.contains("not callable") || msg.contains("cannot call") {
        "not callable"
    } else if msg.contains("missing") {
        "missing"
    } else if msg.contains("duplicate") {
        "duplicate"
    } else {
        "type error"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mismatch_error() {
        let error = TypeCheckError::TypeMismatch {
            span: Span::new(10, 20),
            expected: "int".to_string(),
            found: "str".to_string(),
        };

        assert_eq!(error.span(), Span::new(10, 20));
        assert_eq!(error.code(), ErrorCode::E2001);
        assert!(!error.is_soft());
    }

    #[test]
    fn test_cannot_infer_is_soft() {
        let error = TypeCheckError::CannotInfer {
            span: Span::new(0, 5),
            context: "return value".to_string(),
        };

        assert!(error.is_soft());
        assert_eq!(error.code(), ErrorCode::E2005);
    }

    #[test]
    fn test_error_type_is_soft() {
        let error = TypeCheckError::TypeMismatch {
            span: Span::new(0, 5),
            expected: "int".to_string(),
            found: "<error>".to_string(),
        };

        assert!(error.is_soft());
        assert!(error.is_follow_on());
    }

    #[test]
    fn test_arg_count_mismatch_with_name() {
        let error = TypeCheckError::ArgCountMismatch {
            span: Span::new(0, 10),
            expected: 2,
            found: 3,
            func_name: Some("add".to_string()),
        };

        let diag = error.to_diagnostic();
        assert!(diag.message.contains("add"));
        assert!(diag.message.contains('2'));
        assert!(diag.message.contains('3'));
    }

    #[test]
    fn test_unknown_identifier_with_suggestion() {
        let error = TypeCheckError::UnknownIdentifier {
            span: Span::new(0, 5),
            name: "fo".to_string(),
            suggestion: Some("for".to_string()),
        };

        let diag = error.to_diagnostic();
        assert!(diag.suggestions.iter().any(|s| s.contains("for")));
    }
}
