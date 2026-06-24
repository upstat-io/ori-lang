//! Structured error category: [`EvalErrorKind`].

use ori_ir::BinaryOp;
use std::fmt;

/// Typed error category for structured diagnostics.
///
/// Each variant carries structured data for the error condition, enabling:
/// - Programmatic error matching (switch on kind, not string parsing)
/// - Error code assignment (E6xxx ranges)
/// - Machine-readable diagnostic output
///
/// Factory functions populate both `kind` and `message`. The `Display` impl
/// produces the same message strings as the legacy factory functions, ensuring
/// backward compatibility.
///
/// Prior art: Rust `InterpError` (categorized into UB, Unsupported, `InvalidProgram`,
/// `ResourceExhaustion`), Elm contextual errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalErrorKind {
    // Arithmetic
    DivisionByZero,
    ModuloByZero,
    IntegerOverflow {
        operation: String,
    },
    SizeWouldBeNegative,
    SizeNegativeMultiply,
    SizeNegativeDivide,

    // Type/Operator
    TypeMismatch {
        expected: String,
        got: String,
    },
    InvalidBinaryOp {
        type_name: String,
        op: BinaryOp,
    },
    BinaryTypeMismatch {
        left: String,
        right: String,
    },

    // Access
    UndefinedVariable {
        name: String,
    },
    UndefinedFunction {
        name: String,
    },
    UndefinedConst {
        name: String,
    },
    UndefinedField {
        field: String,
    },
    UndefinedMethod {
        method: String,
        type_name: String,
    },
    IndexOutOfBounds {
        index: i64,
    },
    KeyNotFound {
        key: String,
    },
    ImmutableBinding {
        name: String,
    },

    // Function
    ArityMismatch {
        name: String,
        expected: usize,
        got: usize,
    },
    StackOverflow {
        depth: usize,
    },
    NotCallable {
        type_name: String,
    },

    // Pattern
    NonExhaustiveMatch,

    // Assertion/Test
    AssertionFailed {
        message: String,
    },
    PanicCalled {
        message: String,
    },

    // Capability
    MissingCapability {
        capability: String,
    },

    // Const Eval
    ConstEvalBudgetExceeded,

    // Not Implemented
    NotImplemented {
        feature: String,
        suggestion: String,
    },

    /// Catch-all for errors not yet categorized into structured kinds.
    ///
    /// Used by `EvalError::new(msg)` and factory functions that don't map
    /// cleanly to a specific variant. Over time, these should be migrated
    /// to specific variants.
    Custom {
        message: String,
    },
}

impl EvalErrorKind {
    /// Stable, machine-readable variant name.
    ///
    /// Returns a `&'static str` for the variant, independent of `Debug`
    /// formatting. Used by `EvalErrorSnapshot::from_eval_error` at the
    /// Salsa boundary to produce a deterministic `kind_name` field.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::DivisionByZero => "DivisionByZero",
            Self::ModuloByZero => "ModuloByZero",
            Self::IntegerOverflow { .. } => "IntegerOverflow",
            Self::SizeWouldBeNegative => "SizeWouldBeNegative",
            Self::SizeNegativeMultiply => "SizeNegativeMultiply",
            Self::SizeNegativeDivide => "SizeNegativeDivide",
            Self::TypeMismatch { .. } => "TypeMismatch",
            Self::InvalidBinaryOp { .. } => "InvalidBinaryOp",
            Self::BinaryTypeMismatch { .. } => "BinaryTypeMismatch",
            Self::UndefinedVariable { .. } => "UndefinedVariable",
            Self::UndefinedFunction { .. } => "UndefinedFunction",
            Self::UndefinedConst { .. } => "UndefinedConst",
            Self::UndefinedField { .. } => "UndefinedField",
            Self::UndefinedMethod { .. } => "UndefinedMethod",
            Self::IndexOutOfBounds { .. } => "IndexOutOfBounds",
            Self::KeyNotFound { .. } => "KeyNotFound",
            Self::ImmutableBinding { .. } => "ImmutableBinding",
            Self::ArityMismatch { .. } => "ArityMismatch",
            Self::StackOverflow { .. } => "StackOverflow",
            Self::NotCallable { .. } => "NotCallable",
            Self::NonExhaustiveMatch => "NonExhaustiveMatch",
            Self::AssertionFailed { .. } => "AssertionFailed",
            Self::PanicCalled { .. } => "PanicCalled",
            Self::MissingCapability { .. } => "MissingCapability",
            Self::ConstEvalBudgetExceeded => "ConstEvalBudgetExceeded",
            Self::NotImplemented { .. } => "NotImplemented",
            Self::Custom { .. } => "Custom",
        }
    }
}

impl fmt::Display for EvalErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Arithmetic
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::ModuloByZero => write!(f, "modulo by zero"),
            Self::IntegerOverflow { operation } => {
                write!(f, "integer overflow in {operation}")
            }
            Self::SizeWouldBeNegative => {
                write!(f, "size subtraction would result in negative value")
            }
            Self::SizeNegativeMultiply => {
                write!(f, "cannot multiply Size by negative integer")
            }
            Self::SizeNegativeDivide => {
                write!(f, "cannot divide Size by negative integer")
            }

            // Type/Operator
            Self::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
            Self::InvalidBinaryOp { type_name, op } => {
                write!(
                    f,
                    "operator `{}` cannot be applied to {type_name}",
                    op.as_symbol()
                )
            }
            Self::BinaryTypeMismatch { left, right } => {
                write!(f, "cannot apply operator to `{left}` and `{right}`")
            }

            // Access
            Self::UndefinedVariable { name } => write!(f, "undefined variable: {name}"),
            Self::UndefinedFunction { name } => write!(f, "undefined function: @{name}"),
            Self::UndefinedConst { name } => write!(f, "undefined constant: ${name}"),
            Self::UndefinedField { field } => write!(f, "no field {field} on struct"),
            Self::UndefinedMethod { method, type_name } => {
                write!(f, "no method '{method}' on type {type_name}")
            }
            Self::IndexOutOfBounds { index } => write!(f, "index {index} out of bounds"),
            Self::KeyNotFound { key } => write!(f, "key not found: {key}"),
            Self::ImmutableBinding { name } => {
                write!(f, "cannot assign to immutable variable: {name}")
            }

            // Function
            Self::ArityMismatch {
                name,
                expected,
                got,
            } => {
                let arg_word = if *expected == 1 {
                    "argument"
                } else {
                    "arguments"
                };
                if name.is_empty() {
                    write!(f, "expected {expected} {arg_word}, got {got}")
                } else {
                    write!(f, "{name} expects {expected} {arg_word}, got {got}")
                }
            }
            Self::StackOverflow { depth } => {
                write!(f, "maximum recursion depth exceeded (limit: {depth})")
            }
            Self::NotCallable { type_name } => write!(f, "{type_name} is not callable"),

            // Pattern
            Self::NonExhaustiveMatch => write!(f, "non-exhaustive match"),

            // Assertion/Test
            Self::AssertionFailed { message } => write!(f, "assertion failed: {message}"),
            Self::PanicCalled { message } => write!(f, "panic: {message}"),

            // Capability
            Self::MissingCapability { capability } => {
                write!(f, "missing capability: {capability}")
            }

            // Const Eval
            Self::ConstEvalBudgetExceeded => write!(f, "const eval budget exceeded"),

            // Not Implemented
            Self::NotImplemented {
                feature,
                suggestion,
            } => write!(f, "{feature}; {suggestion}"),

            // Custom
            Self::Custom { message } => write!(f, "{message}"),
        }
    }
}
