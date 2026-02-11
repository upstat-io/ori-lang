//! Error types for pattern evaluation.
//!
//! This module provides the core error types used during pattern evaluation
//! and the evaluator's runtime.
//!
//! # Structured Error Categories
//!
//! `EvalErrorKind` provides typed error categories for diagnostic conversion.
//! Factory functions (e.g., `division_by_zero()`) remain the public API —
//! they populate both `kind` and `message` for backward compatibility.

use crate::value::Value;
use ori_ir::{BinaryOp, Span};
use std::fmt;

/// Result of evaluation.
///
/// The `Err` variant carries a `ControlAction`, which distinguishes runtime errors
/// from control flow signals (break, continue, propagate). This separation is
/// enforced by exhaustive matching — callers must handle each signal kind.
///
/// Prior art: Rust Miri uses `InterpResult<T> = Result<T, InterpError>` with
/// control flow as a separate mechanism.
pub type EvalResult = Result<Value, ControlAction>;

/// Non-value outcomes from expression evaluation.
///
/// Replaces the old pattern of stuffing control flow signals into `EvalError` fields.
/// The `Err` variant of `EvalResult`. Exhaustive matching at call sites enforces
/// correct handling of each signal kind.
///
/// `EvalError` is boxed because it is ~128 bytes (kind, message, span, backtrace,
/// notes). Without boxing, `ControlAction` would bloat every `Result<Value, ControlAction>`
/// on the stack. Boxing keeps `ControlAction` at ~max(8, sizeof(Value)) + discriminant.
#[derive(Clone, Debug)]
pub enum ControlAction {
    /// A runtime error (the only variant that represents failure).
    Error(Box<EvalError>),
    /// Break from a loop, optionally with a value.
    Break(Value),
    /// Continue to next iteration, optionally with a substitution value (for...yield).
    Continue(Value),
    /// Propagation from `?` operator — carries the original Err/None value.
    Propagate(Value),
}

impl ControlAction {
    /// Convert to `EvalError`, collapsing control flow variants into descriptive errors.
    ///
    /// Used at Salsa boundaries and other places that need a plain `EvalError`.
    pub fn into_eval_error(self) -> EvalError {
        match self {
            ControlAction::Error(e) => *e,
            ControlAction::Break(v) => EvalError::new(format!("break:{v}")),
            ControlAction::Continue(v) => EvalError::new(format!("continue:{v}")),
            ControlAction::Propagate(v) => EvalError::new(format!("propagated error: {v:?}")),
        }
    }

    /// Attach a span to this action, but only if it is an `Error` without one.
    ///
    /// Control flow signals (Break, Continue, Propagate) pass through unchanged.
    #[must_use]
    pub fn with_span_if_error(self, span: Span) -> Self {
        match self {
            ControlAction::Error(e) if e.span.is_none() => {
                ControlAction::Error(Box::new(e.with_span(span)))
            }
            other => other,
        }
    }

    /// Check if this action is a runtime error (not a control flow signal).
    #[inline]
    pub fn is_error(&self) -> bool {
        matches!(self, ControlAction::Error(_))
    }
}

impl From<EvalError> for ControlAction {
    fn from(e: EvalError) -> Self {
        ControlAction::Error(Box::new(e))
    }
}

// Structured error types

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

impl fmt::Display for EvalErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Arithmetic
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::ModuloByZero => write!(f, "modulo by zero"),
            Self::IntegerOverflow { operation } => {
                write!(f, "integer overflow in {operation}")
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

/// Additional context note attached to an error.
///
/// Notes provide secondary information about the error, such as
/// "defined here" with a span pointing to a relevant declaration.
#[derive(Clone, Debug)]
pub struct EvalNote {
    pub message: String,
    pub span: Option<Span>,
}

impl EvalNote {
    /// Create a note with just a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    /// Create a note with a message and source location.
    pub fn with_span(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }
}

/// A single frame in an evaluation backtrace.
///
/// Represents one call in the call chain at the point where an error occurred.
#[derive(Clone, Debug)]
pub struct BacktraceFrame {
    /// Function or method name.
    pub name: String,
    /// Source location of the call site.
    pub span: Option<Span>,
}

/// Immutable snapshot of the call stack at an error site.
///
/// Captured from `CallStack` when an error occurs. Stored on `EvalError`
/// for display in diagnostics.
///
/// The `oric` layer enriches frames with file/line info via `enrich()`.
#[derive(Clone, Debug, Default)]
pub struct EvalBacktrace {
    frames: Vec<BacktraceFrame>,
}

impl EvalBacktrace {
    /// Create a backtrace from a list of frames.
    pub fn new(frames: Vec<BacktraceFrame>) -> Self {
        Self { frames }
    }

    /// Get the backtrace frames.
    pub fn frames(&self) -> &[BacktraceFrame] {
        &self.frames
    }

    /// Check if the backtrace is empty.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Number of frames in the backtrace.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Format the backtrace as a human-readable string.
    pub fn display(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for EvalBacktrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.frames.is_empty() {
            return Ok(());
        }
        writeln!(f, "stack backtrace:")?;
        for (i, frame) in self.frames.iter().enumerate() {
            write!(f, "  {i}: {}", frame.name)?;
            if let Some(span) = frame.span {
                write!(f, " at {span}")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// Evaluation error.
///
/// Represents a runtime error only — control flow signals (break, continue,
/// propagate) are handled by `ControlAction` variants, not by `EvalError`.
#[derive(Clone, Debug)]
pub struct EvalError {
    /// Structured error category for diagnostic conversion.
    ///
    /// Enables programmatic error matching, error code assignment (E6xxx),
    /// and machine-readable output. Factory functions set this to the
    /// specific variant; `EvalError::new(msg)` uses `Custom`.
    pub kind: EvalErrorKind,
    /// Human-readable error message.
    ///
    /// For factory-created errors, this equals `kind.to_string()`.
    /// Kept as a field for backward compatibility with code that
    /// accesses `error.message` directly.
    pub message: String,
    /// Source location where the error occurred.
    pub span: Option<Span>,
    /// Call stack backtrace at the error site.
    ///
    /// Populated by the evaluator when `CallStack` is available.
    /// Contains the chain of function calls leading to the error.
    pub backtrace: Option<EvalBacktrace>,
    /// Additional context notes providing secondary information.
    pub notes: Vec<EvalNote>,
}

impl EvalError {
    /// Create an error with just a message.
    ///
    /// Uses `Custom` kind. Prefer specific factory functions (e.g.,
    /// `division_by_zero()`) when a structured kind is available.
    pub fn new(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            kind: EvalErrorKind::Custom {
                message: msg.clone(),
            },
            message: msg,
            span: None,
            backtrace: None,
            notes: Vec::new(),
        }
    }

    /// Create an error from a structured kind.
    ///
    /// The message is computed from the kind's `Display` impl.
    /// Used internally by factory functions.
    fn from_kind(kind: EvalErrorKind) -> Self {
        let message = kind.to_string();
        Self {
            kind,
            message,
            span: None,
            backtrace: None,
            notes: Vec::new(),
        }
    }

    /// Attach a source span to this error.
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attach a backtrace to this error.
    #[must_use]
    pub fn with_backtrace(mut self, backtrace: EvalBacktrace) -> Self {
        self.backtrace = Some(backtrace);
        self
    }

    /// Add a context note to this error.
    #[must_use]
    pub fn with_note(mut self, note: EvalNote) -> Self {
        self.notes.push(note);
        self
    }
}

// Binary Operation Errors

/// Invalid operator for a specific type with operator context.
#[cold]
pub fn invalid_binary_op_for(type_name: &str, op: BinaryOp) -> EvalError {
    EvalError::from_kind(EvalErrorKind::InvalidBinaryOp {
        type_name: type_name.to_string(),
        op,
    })
}

/// Type mismatch in binary operation.
#[cold]
pub fn binary_type_mismatch(left: &str, right: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::BinaryTypeMismatch {
        left: left.to_string(),
        right: right.to_string(),
    })
}

/// Division by zero error.
#[cold]
pub fn division_by_zero() -> EvalError {
    EvalError::from_kind(EvalErrorKind::DivisionByZero)
}

/// Modulo by zero error.
#[cold]
pub fn modulo_by_zero() -> EvalError {
    EvalError::from_kind(EvalErrorKind::ModuloByZero)
}

/// Integer overflow error.
#[cold]
pub fn integer_overflow(operation: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::IntegerOverflow {
        operation: operation.to_string(),
    })
}

/// Maximum recursion depth exceeded error.
#[cold]
pub fn recursion_limit_exceeded(limit: usize) -> EvalError {
    EvalError::from_kind(EvalErrorKind::StackOverflow { depth: limit })
}

// Method Call Errors

/// No such method on a type.
#[cold]
pub fn no_such_method(method: &str, type_name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::UndefinedMethod {
        method: method.to_string(),
        type_name: type_name.to_string(),
    })
}

/// Wrong argument count for a method.
#[cold]
pub fn wrong_arg_count(method: &str, expected: usize, got: usize) -> EvalError {
    EvalError::from_kind(EvalErrorKind::ArityMismatch {
        name: method.to_string(),
        expected,
        got,
    })
}

/// Wrong argument type for a method.
#[cold]
pub fn wrong_arg_type(method: &str, expected: &str) -> EvalError {
    EvalError::new(format!("{method} expects a {expected} argument"))
}

// Variable and Function Errors

/// Undefined variable.
#[cold]
pub fn undefined_variable(name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::UndefinedVariable {
        name: name.to_string(),
    })
}

/// Undefined function.
#[cold]
pub fn undefined_function(name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::UndefinedFunction {
        name: name.to_string(),
    })
}

/// Undefined constant.
#[cold]
pub fn undefined_const(name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::UndefinedConst {
        name: name.to_string(),
    })
}

/// Value is not callable.
#[cold]
pub fn not_callable(type_name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotCallable {
        type_name: type_name.to_string(),
    })
}

/// Wrong number of arguments in function call.
#[cold]
pub fn wrong_function_args(expected: usize, got: usize) -> EvalError {
    EvalError::from_kind(EvalErrorKind::ArityMismatch {
        name: String::new(),
        expected,
        got,
    })
}

// Index and Field Access Errors

/// Index out of bounds.
#[cold]
pub fn index_out_of_bounds(index: i64) -> EvalError {
    EvalError::from_kind(EvalErrorKind::IndexOutOfBounds { index })
}

/// Key not found in map.
#[cold]
pub fn key_not_found(key: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::KeyNotFound {
        key: key.to_string(),
    })
}

/// Cannot index type with another type.
#[cold]
pub fn cannot_index(receiver: &str, index: &str) -> EvalError {
    EvalError::new(format!("cannot index {receiver} with {index}"))
}

/// Cannot get length of type.
#[cold]
pub fn cannot_get_length(type_name: &str) -> EvalError {
    EvalError::new(format!("cannot get length of {type_name}"))
}

/// No field on struct.
#[cold]
pub fn no_field_on_struct(field: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::UndefinedField {
        field: field.to_string(),
    })
}

/// Invalid tuple field.
#[cold]
pub fn invalid_tuple_field(field: &str) -> EvalError {
    EvalError::new(format!("invalid tuple field: {field}"))
}

/// Tuple index out of bounds.
#[cold]
pub fn tuple_index_out_of_bounds(index: usize) -> EvalError {
    EvalError::from_kind(EvalErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
    })
}

/// Cannot access field on type.
#[cold]
pub fn cannot_access_field(type_name: &str) -> EvalError {
    EvalError::new(format!("cannot access field on {type_name}"))
}

/// No member in module namespace.
#[cold]
pub fn no_member_in_module(member: &str) -> EvalError {
    EvalError::new(format!("module has no member '{member}'"))
}

// Type Conversion and Validation Errors

/// Range start/end must be integer.
#[cold]
pub fn range_bound_not_int(bound: &str) -> EvalError {
    EvalError::new(format!("range {bound} must be an integer"))
}

/// Unbounded range end.
#[cold]
pub fn unbounded_range_end() -> EvalError {
    EvalError::new("unbounded range end")
}

/// Map keys must be hashable types (primitives, tuples of hashables).
#[cold]
pub fn map_key_not_hashable() -> EvalError {
    EvalError::new("map keys must be hashable (primitives, tuples, etc.)")
}

/// Spread requires a map value.
#[cold]
pub fn spread_requires_map() -> EvalError {
    EvalError::new("spread in map literal requires a map value")
}

/// Spread requires a list value.
#[cold]
pub fn spread_requires_list() -> EvalError {
    EvalError::new("spread in list literal requires a list value")
}

/// Spread requires a struct value.
#[cold]
pub fn spread_requires_struct() -> EvalError {
    EvalError::new("spread in struct literal requires a struct value")
}

// Control Flow Errors

/// Non-exhaustive match.
#[cold]
pub fn non_exhaustive_match() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NonExhaustiveMatch)
}

/// Cannot assign to immutable variable.
#[cold]
pub fn cannot_assign_immutable(name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::ImmutableBinding {
        name: name.to_string(),
    })
}

/// Invalid assignment target.
#[cold]
pub fn invalid_assignment_target() -> EvalError {
    EvalError::new("invalid assignment target")
}

/// For loop requires iterable.
#[cold]
pub fn for_requires_iterable() -> EvalError {
    EvalError::new("for requires an iterable")
}

// Pattern Binding Errors

/// Tuple pattern length mismatch.
#[cold]
pub fn tuple_pattern_mismatch() -> EvalError {
    EvalError::new("tuple pattern length mismatch")
}

/// Expected tuple value.
#[cold]
pub fn expected_tuple() -> EvalError {
    EvalError::new("expected tuple value")
}

/// Expected struct value.
#[cold]
pub fn expected_struct() -> EvalError {
    EvalError::new("expected struct value")
}

/// Expected list value.
#[cold]
pub fn expected_list() -> EvalError {
    EvalError::new("expected list value")
}

/// List pattern too long for value.
#[cold]
pub fn list_pattern_too_long() -> EvalError {
    EvalError::new("list pattern too long for value")
}

/// Missing struct field.
#[cold]
pub fn missing_struct_field() -> EvalError {
    EvalError::new("missing struct field")
}

// Miscellaneous Errors

/// Self used outside of method context.
#[cold]
pub fn self_outside_method() -> EvalError {
    EvalError::new("'self' used outside of method context")
}

/// Parse error placeholder.
#[cold]
pub fn parse_error() -> EvalError {
    EvalError::new("parse error")
}

/// Hash length used outside index brackets.
#[cold]
pub fn hash_outside_index() -> EvalError {
    EvalError::new("# can only be used inside index brackets")
}

/// Await not supported.
#[cold]
pub fn await_not_supported() -> EvalError {
    EvalError::new("await not supported in interpreter")
}

/// Invalid literal pattern.
#[cold]
pub fn invalid_literal_pattern() -> EvalError {
    EvalError::new("invalid literal pattern")
}

// Collection Method Errors

/// Map requires a collection (list or range).
#[cold]
pub fn map_requires_collection() -> EvalError {
    EvalError::new("map requires a collection")
}

/// Filter requires a collection (list or range).
#[cold]
pub fn filter_requires_collection() -> EvalError {
    EvalError::new("filter requires a collection")
}

/// Fold requires a collection (list or range).
#[cold]
pub fn fold_requires_collection() -> EvalError {
    EvalError::new("fold requires a collection")
}

/// Find requires a list.
#[cold]
pub fn find_requires_list() -> EvalError {
    EvalError::new("find requires a list")
}

/// Collect requires a range.
#[cold]
pub fn collect_requires_range() -> EvalError {
    EvalError::new("collect requires a range")
}

/// Any requires a list.
#[cold]
pub fn any_requires_list() -> EvalError {
    EvalError::new("any requires a list")
}

/// All requires a list.
#[cold]
pub fn all_requires_list() -> EvalError {
    EvalError::new("all requires a list")
}

/// Map entries requires a map.
#[cold]
pub fn map_entries_requires_map() -> EvalError {
    EvalError::new("map entries requires a map")
}

/// Filter entries requires a map.
#[cold]
pub fn filter_entries_requires_map() -> EvalError {
    EvalError::new("filter entries requires a map")
}

// Not Implemented Errors

/// Map entries not yet implemented.
#[cold]
pub fn map_entries_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "map_entries() is not yet implemented".to_string(),
        suggestion: "use map() with entry destructuring: map.entries().map((k, v) -> ...)"
            .to_string(),
    })
}

/// Filter entries not yet implemented.
#[cold]
pub fn filter_entries_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "filter_entries() is not yet implemented".to_string(),
        suggestion: "use filter() with entry destructuring: map.entries().filter((k, v) -> ...)"
            .to_string(),
    })
}

/// Index assignment not yet implemented.
#[cold]
pub fn index_assignment_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "index assignment (list[i] = x) is not yet implemented".to_string(),
        suggestion: "use list.set(index: i, value: x) instead".to_string(),
    })
}

/// Field assignment not yet implemented.
#[cold]
pub fn field_assignment_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "field assignment (obj.field = x) is not yet implemented".to_string(),
        suggestion: "use spread syntax: { ...obj, field: x }".to_string(),
    })
}

/// Default requires type context.
#[cold]
pub fn default_requires_type_context() -> EvalError {
    EvalError::new("default() requires type context; use explicit construction instead")
}

// Index Context Errors

/// Operator not supported in index context.
#[cold]
pub fn operator_not_supported_in_index() -> EvalError {
    EvalError::new("operator not supported in index context")
}

/// Non-integer in index context.
#[cold]
pub fn non_integer_in_index() -> EvalError {
    EvalError::new("non-integer in index context")
}

/// Collection too large for indexing.
#[cold]
pub fn collection_too_large() -> EvalError {
    EvalError::new("collection too large")
}

// Pattern Errors

/// Unknown pattern kind.
#[cold]
pub fn unknown_pattern(kind: &str) -> EvalError {
    EvalError::new(format!("unknown pattern: {kind}"))
}

/// For pattern requires a list.
#[cold]
pub fn for_pattern_requires_list(actual: &str) -> EvalError {
    EvalError::new(format!("for pattern requires a list, got {actual}"))
}

// Propagation Helpers

/// Create a standardized propagated error message.
///
/// This ensures consistent formatting of propagated errors across all call sites.
#[cold]
pub fn propagated_error_message(value: &Value) -> String {
    format!("propagated error: {value:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Kind → message round-trip

    #[test]
    fn division_by_zero_has_correct_kind() {
        let err = division_by_zero();
        assert_eq!(err.kind, EvalErrorKind::DivisionByZero);
        assert_eq!(err.message, "division by zero");
    }

    #[test]
    fn modulo_by_zero_has_correct_kind() {
        let err = modulo_by_zero();
        assert_eq!(err.kind, EvalErrorKind::ModuloByZero);
        assert_eq!(err.message, "modulo by zero");
    }

    #[test]
    fn integer_overflow_has_correct_kind() {
        let err = integer_overflow("addition");
        assert_eq!(
            err.kind,
            EvalErrorKind::IntegerOverflow {
                operation: "addition".to_string()
            }
        );
        assert_eq!(err.message, "integer overflow in addition");
    }

    #[test]
    fn undefined_variable_has_correct_kind() {
        let err = undefined_variable("x");
        assert_eq!(
            err.kind,
            EvalErrorKind::UndefinedVariable {
                name: "x".to_string()
            }
        );
        assert_eq!(err.message, "undefined variable: x");
    }

    #[test]
    fn undefined_function_has_correct_kind() {
        let err = undefined_function("foo");
        assert_eq!(
            err.kind,
            EvalErrorKind::UndefinedFunction {
                name: "foo".to_string()
            }
        );
        assert_eq!(err.message, "undefined function: @foo");
    }

    #[test]
    fn arity_mismatch_with_name() {
        let err = wrong_arg_count("push", 1, 2);
        assert_eq!(
            err.kind,
            EvalErrorKind::ArityMismatch {
                name: "push".to_string(),
                expected: 1,
                got: 2
            }
        );
        assert_eq!(err.message, "push expects 1 argument, got 2");
    }

    #[test]
    fn arity_mismatch_without_name() {
        let err = wrong_function_args(3, 1);
        assert_eq!(
            err.kind,
            EvalErrorKind::ArityMismatch {
                name: String::new(),
                expected: 3,
                got: 1
            }
        );
        assert_eq!(err.message, "expected 3 arguments, got 1");
    }

    #[test]
    fn stack_overflow_has_correct_kind() {
        let err = recursion_limit_exceeded(200);
        assert_eq!(err.kind, EvalErrorKind::StackOverflow { depth: 200 });
        assert_eq!(err.message, "maximum recursion depth exceeded (limit: 200)");
    }

    #[test]
    fn non_exhaustive_match_has_correct_kind() {
        let err = non_exhaustive_match();
        assert_eq!(err.kind, EvalErrorKind::NonExhaustiveMatch);
        assert_eq!(err.message, "non-exhaustive match");
    }

    #[test]
    fn not_implemented_has_correct_kind() {
        let err = index_assignment_not_implemented();
        assert!(matches!(err.kind, EvalErrorKind::NotImplemented { .. }));
        assert!(err.message.contains("not yet implemented"));
        assert!(err.message.contains("list.set"));
    }

    #[test]
    fn custom_kind_for_new() {
        let err = EvalError::new("something broke");
        assert_eq!(
            err.kind,
            EvalErrorKind::Custom {
                message: "something broke".to_string()
            }
        );
        assert_eq!(err.message, "something broke");
    }

    // Builder methods

    #[test]
    fn with_span_sets_span() {
        let span = Span::new(10, 20);
        let err = division_by_zero().with_span(span);
        assert_eq!(err.span, Some(span));
    }

    #[test]
    fn with_backtrace_sets_backtrace() {
        let bt = EvalBacktrace::new(vec![BacktraceFrame {
            name: "foo".to_string(),
            span: None,
        }]);
        let err = division_by_zero().with_backtrace(bt);
        assert!(err.backtrace.is_some());
        assert_eq!(err.backtrace.as_ref().map(EvalBacktrace::len), Some(1));
    }

    #[test]
    fn with_note_adds_note() {
        let err = division_by_zero().with_note(EvalNote::new("denominator was 0"));
        assert_eq!(err.notes.len(), 1);
        assert_eq!(err.notes[0].message, "denominator was 0");
    }

    // Backtrace display

    #[test]
    fn empty_backtrace_display() {
        let bt = EvalBacktrace::default();
        assert!(bt.is_empty());
        assert_eq!(bt.display(), "");
    }

    #[test]
    fn backtrace_display_with_frames() {
        let bt = EvalBacktrace::new(vec![
            BacktraceFrame {
                name: "bar".to_string(),
                span: Some(Span::new(100, 110)),
            },
            BacktraceFrame {
                name: "foo".to_string(),
                span: None,
            },
        ]);
        let display = bt.display();
        assert!(display.contains("0: bar"));
        assert!(display.contains("1: foo"));
    }

    // Kind display round-trip: verify Display matches message for all factory funcs

    #[test]
    fn kind_display_matches_message() {
        let errors: Vec<EvalError> = vec![
            division_by_zero(),
            modulo_by_zero(),
            integer_overflow("mul"),
            no_such_method("len", "int"),
            wrong_arg_count("push", 1, 3),
            wrong_function_args(2, 0),
            undefined_variable("x"),
            undefined_function("main"),
            undefined_const("PI"),
            not_callable("int"),
            index_out_of_bounds(5),
            key_not_found("name"),
            no_field_on_struct("age"),
            non_exhaustive_match(),
            cannot_assign_immutable("x"),
            recursion_limit_exceeded(100),
        ];
        for err in &errors {
            assert_eq!(
                err.message,
                err.kind.to_string(),
                "message/kind mismatch for {:?}",
                err.kind
            );
        }
    }

    // ControlAction tests

    #[test]
    fn control_action_break_carries_value() {
        let action = ControlAction::Break(Value::int(42));
        assert!(!action.is_error());
        if let ControlAction::Break(v) = action {
            assert_eq!(v, Value::int(42));
        } else {
            panic!("expected Break");
        }
    }

    #[test]
    fn control_action_continue_carries_value() {
        let action = ControlAction::Continue(Value::Void);
        assert!(!action.is_error());
        assert!(matches!(action, ControlAction::Continue(Value::Void)));
    }

    #[test]
    fn control_action_propagate_carries_value() {
        let action = ControlAction::Propagate(Value::None);
        assert!(!action.is_error());
        assert!(matches!(action, ControlAction::Propagate(Value::None)));
    }

    #[test]
    fn control_action_error_is_error() {
        let action: ControlAction = EvalError::new("test").into();
        assert!(action.is_error());
    }

    #[test]
    fn control_action_from_eval_error() {
        let err = division_by_zero();
        let action: ControlAction = err.into();
        assert!(action.is_error());
        if let ControlAction::Error(e) = action {
            assert_eq!(e.kind, EvalErrorKind::DivisionByZero);
        } else {
            panic!("expected Error");
        }
    }

    #[test]
    fn control_action_into_eval_error_roundtrip() {
        let err = division_by_zero();
        let msg = err.message.clone();
        let action: ControlAction = err.into();
        let recovered = action.into_eval_error();
        assert_eq!(recovered.message, msg);
    }

    #[test]
    fn control_action_into_eval_error_from_break() {
        let action = ControlAction::Break(Value::int(5));
        let err = action.into_eval_error();
        assert!(err.message.contains("break"));
    }

    #[test]
    fn control_action_with_span_if_error_attaches_span() {
        let span = Span::new(10, 20);
        let action: ControlAction = EvalError::new("test").into();
        let action = action.with_span_if_error(span);
        if let ControlAction::Error(e) = action {
            assert_eq!(e.span, Some(span));
        } else {
            panic!("expected Error");
        }
    }

    #[test]
    fn control_action_with_span_if_error_ignores_control_flow() {
        let span = Span::new(10, 20);
        let action = ControlAction::Break(Value::Void);
        let action = action.with_span_if_error(span);
        // Break should pass through unchanged
        assert!(matches!(action, ControlAction::Break(Value::Void)));
    }
}
