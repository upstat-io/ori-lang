//! Type problem identification.
//!
//! This module identifies WHAT went wrong specifically, transforming generic
//! "type mismatch" errors into actionable diagnoses like `IntFloat` (use conversion)
//! or `FieldTypo` (did you mean X?).
//!
//! # Design
//!
//! Based on Elm's approach from `Reporting/Error/Type.hs`:
//! - Pattern-match on type combinations to identify specific problems
//! - Each problem knows how to generate targeted suggestions
//! - Problems have severity levels (error, warning, info)
//!
//! # Problem Categories
//!
//! - **Numeric**: `IntFloat`, `NumberToString`, `StringToNumber`
//! - **Collections**: `ExpectedList`, `NeedsUnwrap`, `WrongCollectionType`
//! - **Functions**: `WrongArity`, `NotCallable`, `MissingArguments`
//! - **Records**: `MissingField`, `FieldTypo`, `FieldTypeMismatch`
//! - **Type vars**: `RigidMismatch`, `InfiniteType`, `EscapingVariable`

use ori_ir::Name;

use crate::Idx;

/// A specific problem identified by comparing two types.
///
/// These are more actionable than generic "type mismatch" because each
/// variant knows what went wrong and can generate targeted suggestions.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TypeProblem {
    // ════════════════════════════════════════════════════════════════════════
    // Numeric Problems
    // ════════════════════════════════════════════════════════════════════════
    /// Mixing int and float without explicit conversion.
    ///
    /// Example: `1 + 2.0` - int cannot implicitly convert to float.
    IntFloat,

    /// Trying to use a number where a string is expected.
    ///
    /// Example: `print(42)` where print expects a string.
    NumberToString,

    /// Trying to use a string where a number is expected.
    ///
    /// Example: `"42" + 1` - string cannot be added to int.
    StringToNumber,

    /// Using wrong numeric type (e.g., byte vs int).
    NumericTypeMismatch {
        /// Expected numeric type name.
        expected: &'static str,
        /// Found numeric type name.
        found: &'static str,
    },

    // ════════════════════════════════════════════════════════════════════════
    // Collection Problems
    // ════════════════════════════════════════════════════════════════════════
    /// Expected a list but got something else.
    ///
    /// Example: `for x in 42` - int is not iterable.
    ExpectedList {
        /// What was found instead.
        found: &'static str,
    },

    /// List element type mismatch.
    ///
    /// Example: `[1, 2, "three"]` - str doesn't match int.
    ListElementMismatch {
        /// Index of the mismatching element.
        index: usize,
    },

    /// Expected an Option type.
    ExpectedOption,

    /// Value needs to be unwrapped before use.
    ///
    /// Example: Using `Option<int>` where `int` is expected.
    NeedsUnwrap {
        /// Type inside the wrapper (Option/Result).
        inner_type: Idx,
    },

    /// Wrong collection type (list vs set vs map).
    WrongCollectionType {
        /// Expected collection kind.
        expected: &'static str,
        /// Found collection kind.
        found: &'static str,
    },

    /// Map key type mismatch.
    MapKeyMismatch,

    /// Map value type mismatch.
    MapValueMismatch,

    // ════════════════════════════════════════════════════════════════════════
    // Function Problems
    // ════════════════════════════════════════════════════════════════════════
    /// Wrong number of arguments to a function.
    WrongArity {
        /// Expected number of parameters.
        expected: usize,
        /// Found number of arguments.
        found: usize,
    },

    /// Argument type doesn't match parameter type.
    ArgumentMismatch {
        /// Zero-based argument index.
        arg_index: usize,
        /// Expected type (parameter).
        expected: Idx,
        /// Found type (argument).
        found: Idx,
    },

    /// Function return type mismatch.
    ReturnMismatch {
        /// Expected return type.
        expected: Idx,
        /// Actual return type.
        found: Idx,
    },

    /// Trying to call something that isn't a function.
    ///
    /// Example: `42()` - int is not callable.
    NotCallable {
        /// The type that was called.
        actual_type: Idx,
    },

    /// Missing required arguments.
    MissingArguments {
        /// Names of missing parameters.
        missing: Vec<Name>,
    },

    /// Too many arguments provided.
    ExtraArguments {
        /// Number of extra arguments.
        count: usize,
    },

    // ════════════════════════════════════════════════════════════════════════
    // Record/Struct Problems
    // ════════════════════════════════════════════════════════════════════════
    /// Accessing a field that doesn't exist on the type.
    MissingField {
        /// Name of the missing field.
        field_name: Name,
        /// Available fields on the type.
        available: Vec<Name>,
    },

    /// Extra field in struct construction.
    ExtraField {
        /// Name of the extra field.
        field_name: Name,
    },

    /// Field has wrong type.
    FieldTypeMismatch {
        /// Name of the field.
        field_name: Name,
        /// Expected type.
        expected: Idx,
        /// Found type.
        found: Idx,
    },

    /// Field name looks like a typo.
    FieldTypo {
        /// What was written.
        attempted: Name,
        /// What was probably meant.
        suggestion: Name,
        /// Edit distance.
        distance: usize,
    },

    /// Wrong struct/record type.
    WrongRecordType {
        /// Expected struct name.
        expected: Name,
        /// Found struct name.
        found: Name,
    },

    // ════════════════════════════════════════════════════════════════════════
    // Type Variable Problems
    // ════════════════════════════════════════════════════════════════════════
    /// Rigid type variable (from annotation) cannot match concrete type.
    ///
    /// Example: Function declares `fn foo<T>(x: T)`, caller passes int,
    /// but T is used in a context requiring it to be a specific type.
    RigidMismatch {
        /// Name of the rigid variable.
        rigid_name: Name,
        /// Concrete type it was asked to unify with.
        concrete: Idx,
    },

    /// Would create an infinite/recursive type (occurs check failure).
    ///
    /// Example: `a = [a]` would make a = List<a> = List<List<a>> = ...
    InfiniteType {
        /// ID of the variable that would recurse.
        var_id: u32,
    },

    /// Type variable escaping its scope.
    EscapingVariable {
        /// Name of the escaping variable, if known.
        var_name: Option<Name>,
    },

    // ════════════════════════════════════════════════════════════════════════
    // Capability Problems
    // ════════════════════════════════════════════════════════════════════════
    /// Missing required capability.
    MissingCapability {
        /// Name of the required capability.
        required: Name,
    },

    /// Capability conflict.
    CapabilityConflict {
        /// Capability that was provided.
        provided: Name,
        /// Capability that was required.
        required: Name,
    },

    // ════════════════════════════════════════════════════════════════════════
    // Pattern Problems
    // ════════════════════════════════════════════════════════════════════════
    /// Pattern doesn't match the scrutinee type.
    PatternMismatch {
        /// Expected pattern type.
        expected: Idx,
        /// Found pattern type.
        found: Idx,
    },

    /// Non-exhaustive pattern match.
    NonExhaustiveMatch {
        /// Missing patterns (as strings for display).
        missing_patterns: Vec<String>,
    },

    // ════════════════════════════════════════════════════════════════════════
    // Operator Problems
    // ════════════════════════════════════════════════════════════════════════
    /// Operator applied to an unsupported type.
    ///
    /// Example: `5.0 & 3.0` - bitwise operator requires int operands.
    BadOperandType {
        /// The operator symbol (e.g., "-", "!", "&", "&&").
        op: &'static str,
        /// The category of the operator (e.g., "bitwise", "logical").
        op_category: &'static str,
        /// The display name of the type that was found.
        found_type: &'static str,
        /// The required type for this operator.
        required_type: &'static str,
    },

    /// Closure attempts to capture its own binding name (self-referential).
    ///
    /// Example: `let f = () -> f` - closure body references `f`.
    ClosureSelfCapture,

    // ════════════════════════════════════════════════════════════════════════
    // Generic Fallback
    // ════════════════════════════════════════════════════════════════════════
    /// Generic type mismatch when no specific problem is identified.
    TypeMismatch {
        /// Category of expected type.
        expected_category: &'static str,
        /// Category of found type.
        found_category: &'static str,
    },
}

/// Severity level for type problems.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Severity {
    /// Informational (e.g., style suggestion).
    Info,
    /// Warning (code works but may be wrong).
    Warning,
    /// Error (code won't work).
    Error,
}

impl TypeProblem {
    /// Get the severity of this problem.
    pub fn severity(&self) -> Severity {
        match self {
            // Most problems are errors
            Self::FieldTypo { .. } => Severity::Warning, // Might be intentional
            _ => Severity::Error,
        }
    }

    /// Get a short description of this problem.
    pub fn description(&self) -> &'static str {
        match self {
            Self::IntFloat => "int and float are different types",
            Self::NumberToString => "cannot use number as string",
            Self::StringToNumber => "cannot use string as number",
            Self::NumericTypeMismatch { .. } => "numeric type mismatch",

            Self::ExpectedList { .. } => "expected a list",
            Self::ListElementMismatch { .. } => "list element type mismatch",
            Self::ExpectedOption => "expected an option type",
            Self::NeedsUnwrap { .. } => "value needs to be unwrapped",
            Self::WrongCollectionType { .. } => "wrong collection type",
            Self::MapKeyMismatch => "map key type mismatch",
            Self::MapValueMismatch => "map value type mismatch",

            Self::WrongArity { .. } => "wrong number of arguments",
            Self::ArgumentMismatch { .. } => "argument type mismatch",
            Self::ReturnMismatch { .. } => "return type mismatch",
            Self::NotCallable { .. } => "value is not callable",
            Self::MissingArguments { .. } => "missing required arguments",
            Self::ExtraArguments { .. } => "too many arguments",

            Self::MissingField { .. } => "field not found",
            Self::ExtraField { .. } => "unexpected field",
            Self::FieldTypeMismatch { .. } => "field type mismatch",
            Self::FieldTypo { .. } => "possible field name typo",
            Self::WrongRecordType { .. } => "wrong record type",

            Self::RigidMismatch { .. } => "type parameter mismatch",
            Self::InfiniteType { .. } => "would create infinite type",
            Self::EscapingVariable { .. } => "type variable escapes scope",

            Self::MissingCapability { .. } => "missing capability",
            Self::CapabilityConflict { .. } => "capability conflict",

            Self::PatternMismatch { .. } => "pattern type mismatch",
            Self::NonExhaustiveMatch { .. } => "non-exhaustive match",

            Self::BadOperandType { .. } => "operator type error",
            Self::ClosureSelfCapture => "closure cannot capture itself",

            Self::TypeMismatch { .. } => "type mismatch",
        }
    }

    /// Get a hint for fixing this problem.
    ///
    /// Returns a short phrase that can be displayed after the error message.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::IntFloat => Some("use `to_float()` or `to_int()` for explicit conversion"),
            Self::NumberToString => Some("use `to_str()` to convert to string"),
            Self::StringToNumber => Some("use `parse()` to convert string to number"),

            Self::NeedsUnwrap { .. } => {
                Some("use `?` to propagate none, or `match` to handle both cases")
            }
            Self::ExpectedList { .. } => Some("wrap the value in a list: `[value]`"),

            Self::WrongArity { expected, found } => {
                if found > expected {
                    Some("remove extra arguments")
                } else {
                    Some("add missing arguments")
                }
            }

            Self::NotCallable { .. } => Some("only functions can be called with `()`"),

            Self::MissingField { .. } => Some("check spelling or add the missing field"),
            // FieldTypo handled by suggestion system (returns None via wildcard)
            Self::InfiniteType { .. } => Some("use a newtype wrapper to break the cycle"),

            Self::MissingCapability { .. } => {
                Some("add `uses CapabilityName` to the function signature")
            }

            Self::NonExhaustiveMatch { .. } => {
                Some("add missing patterns or use `_ =>` as a catch-all")
            }

            Self::ClosureSelfCapture => Some("use recursion through named functions instead"),

            _ => None,
        }
    }

    /// Check if this problem is related to numeric types.
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::IntFloat
                | Self::NumberToString
                | Self::StringToNumber
                | Self::NumericTypeMismatch { .. }
        )
    }

    /// Check if this problem is related to function calls.
    pub fn is_function_related(&self) -> bool {
        matches!(
            self,
            Self::WrongArity { .. }
                | Self::ArgumentMismatch { .. }
                | Self::ReturnMismatch { .. }
                | Self::NotCallable { .. }
                | Self::MissingArguments { .. }
                | Self::ExtraArguments { .. }
        )
    }

    /// Check if this problem is related to records/structs.
    pub fn is_record_related(&self) -> bool {
        matches!(
            self,
            Self::MissingField { .. }
                | Self::ExtraField { .. }
                | Self::FieldTypeMismatch { .. }
                | Self::FieldTypo { .. }
                | Self::WrongRecordType { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problem_severity() {
        assert_eq!(TypeProblem::IntFloat.severity(), Severity::Error);
        assert_eq!(
            TypeProblem::FieldTypo {
                attempted: Name::from_raw(1),
                suggestion: Name::from_raw(2),
                distance: 1
            }
            .severity(),
            Severity::Warning
        );
    }

    #[test]
    fn problem_descriptions() {
        assert_eq!(
            TypeProblem::IntFloat.description(),
            "int and float are different types"
        );
        assert_eq!(
            TypeProblem::WrongArity {
                expected: 2,
                found: 3
            }
            .description(),
            "wrong number of arguments"
        );
    }

    #[test]
    fn problem_hints() {
        assert!(TypeProblem::IntFloat.hint().is_some());
        assert!(TypeProblem::NeedsUnwrap {
            inner_type: Idx::INT
        }
        .hint()
        .is_some());
    }

    #[test]
    fn problem_categories() {
        assert!(TypeProblem::IntFloat.is_numeric());
        assert!(!TypeProblem::IntFloat.is_function_related());

        assert!(TypeProblem::WrongArity {
            expected: 1,
            found: 2
        }
        .is_function_related());
        assert!(!TypeProblem::WrongArity {
            expected: 1,
            found: 2
        }
        .is_numeric());

        assert!(TypeProblem::MissingField {
            field_name: Name::from_raw(1),
            available: vec![]
        }
        .is_record_related());
    }
}
