//! Runtime/eval error to diagnostic conversion.
//!
//! Converts `EvalError` into `Diagnostic` (from `ori_diagnostic`) using E6xxx
//! error codes. These methods live here (rather than in `oric`) because
//! `EvalError` and `EvalErrorKind` are defined in this crate — no orphan rule
//! issue.
//!
//! # Error Code Ranges (E6xxx)
//!
//! - E6001–E6009: Arithmetic (division by zero, overflow)
//! - E6010–E6019: Type/operator errors
//! - E6020–E6029: Access errors (undefined variable/function/field/method)
//! - E6030–E6039: Function call errors (arity, stack overflow, not callable)
//! - E6040–E6049: Pattern/match errors
//! - E6050–E6059: Assertion/test errors
//! - E6060–E6069: Capability errors
//! - E6070–E6079: Const-eval errors
//! - E6080–E6089: Not-implemented errors
//! - E6099: Custom/uncategorized

use ori_diagnostic::{Diagnostic, ErrorCode};

use super::{EvalError, EvalErrorKind};

impl EvalErrorKind {
    /// Map this error kind to its corresponding `ErrorCode`.
    pub fn error_code(&self) -> ErrorCode {
        match self {
            // Arithmetic
            Self::DivisionByZero => ErrorCode::E6001,
            Self::ModuloByZero => ErrorCode::E6002,
            Self::IntegerOverflow { .. } => ErrorCode::E6003,
            Self::SizeWouldBeNegative => ErrorCode::E6004,
            Self::SizeNegativeMultiply => ErrorCode::E6005,
            Self::SizeNegativeDivide => ErrorCode::E6006,

            // Type/Operator
            Self::TypeMismatch { .. } => ErrorCode::E6010,
            Self::InvalidBinaryOp { .. } => ErrorCode::E6011,
            Self::BinaryTypeMismatch { .. } => ErrorCode::E6012,

            // Access
            Self::UndefinedVariable { .. } => ErrorCode::E6020,
            Self::UndefinedFunction { .. } => ErrorCode::E6021,
            Self::UndefinedConst { .. } => ErrorCode::E6022,
            Self::UndefinedField { .. } => ErrorCode::E6023,
            Self::UndefinedMethod { .. } => ErrorCode::E6024,
            Self::IndexOutOfBounds { .. } => ErrorCode::E6025,
            Self::KeyNotFound { .. } => ErrorCode::E6026,
            Self::ImmutableBinding { .. } => ErrorCode::E6027,

            // Function
            Self::ArityMismatch { .. } => ErrorCode::E6030,
            Self::StackOverflow { .. } => ErrorCode::E6031,
            Self::NotCallable { .. } => ErrorCode::E6032,

            // Pattern
            Self::NonExhaustiveMatch => ErrorCode::E6040,

            // Assertion/Test
            Self::AssertionFailed { .. } => ErrorCode::E6050,
            Self::PanicCalled { .. } => ErrorCode::E6051,

            // Capability
            Self::MissingCapability { .. } => ErrorCode::E6060,

            // Const Eval
            Self::ConstEvalBudgetExceeded => ErrorCode::E6070,

            // Not Implemented
            Self::NotImplemented { .. } => ErrorCode::E6080,

            // Custom/catch-all
            Self::Custom { .. } => ErrorCode::E6099,
        }
    }

    /// Produce a concise label for the primary span.
    pub fn primary_label(&self) -> &'static str {
        match self {
            Self::DivisionByZero => "division by zero here",
            Self::ModuloByZero => "modulo by zero here",
            Self::IntegerOverflow { .. } => "overflow occurred here",
            Self::SizeWouldBeNegative => "result would be negative",
            Self::SizeNegativeMultiply | Self::SizeNegativeDivide => "negative operand",
            Self::TypeMismatch { .. } => "type mismatch",
            Self::InvalidBinaryOp { .. } => "operator not supported",
            Self::BinaryTypeMismatch { .. } => "mismatched types",
            Self::UndefinedVariable { .. } => "not found in this scope",
            Self::UndefinedFunction { .. } => "function not found",
            Self::UndefinedConst { .. } => "constant not found",
            Self::UndefinedField { .. } => "field not found",
            Self::UndefinedMethod { .. } => "method not found",
            Self::IndexOutOfBounds { .. } => "index out of bounds",
            Self::KeyNotFound { .. } => "key not found",
            Self::ImmutableBinding { .. } => "cannot assign to immutable binding",
            Self::ArityMismatch { .. } => "wrong number of arguments",
            Self::StackOverflow { .. } => "recursion limit exceeded",
            Self::NotCallable { .. } => "not callable",
            Self::NonExhaustiveMatch => "non-exhaustive match",
            Self::AssertionFailed { .. } => "assertion failed",
            Self::PanicCalled { .. } => "panic",
            Self::MissingCapability { .. } => "missing capability",
            Self::ConstEvalBudgetExceeded => "budget exceeded",
            Self::NotImplemented { .. } => "not implemented",
            Self::Custom { .. } => "runtime error",
        }
    }

    /// Produce an actionable suggestion for fixable errors.
    pub fn suggestion(&self) -> Option<String> {
        match self {
            Self::DivisionByZero => Some("add a zero check before dividing".to_string()),
            Self::ImmutableBinding { name } => Some(format!(
                "declare `{name}` as mutable with `mut {name} = ...`"
            )),
            Self::NonExhaustiveMatch => {
                Some("add a wildcard `_` arm to cover remaining cases".to_string())
            }
            Self::NotImplemented { suggestion, .. } if !suggestion.is_empty() => {
                Some(suggestion.clone())
            }
            Self::MissingCapability { capability } => {
                Some(format!("add `uses {capability}` to the function signature"))
            }
            _ => None,
        }
    }
}

impl EvalError {
    /// Convert this `EvalError` into a `Diagnostic`.
    ///
    /// Maps `EvalErrorKind` to an E6xxx error code and constructs a diagnostic
    /// with the error message, primary span label, notes, and backtrace context.
    #[cold]
    pub fn to_diagnostic(&self) -> Diagnostic {
        let code = self.kind.error_code();
        let mut diag = Diagnostic::error(code).with_message(&self.message);

        // Add primary label at the error span
        if let Some(span) = self.span {
            diag = diag.with_label(span, self.kind.primary_label());
        }

        // Add context notes from the error
        for note in &self.notes {
            diag = diag.with_note(&note.message);
        }

        // Add backtrace as a note (if present)
        if let Some(ref bt) = self.backtrace {
            if !bt.is_empty() {
                diag = diag.with_note(format!("call stack:\n{bt}"));
            }
        }

        // Add suggestions for common fixable errors
        if let Some(suggestion) = self.kind.suggestion() {
            diag = diag.with_suggestion(suggestion);
        }

        diag
    }
}

#[cfg(test)]
mod tests;
