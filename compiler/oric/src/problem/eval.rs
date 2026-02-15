//! Runtime/eval error to diagnostic conversion.
//!
//! Converts `EvalError` (from `ori_patterns`) into `Diagnostic` (from `ori_diagnostic`)
//! using E6xxx error codes. This conversion lives in `oric` due to the orphan rule:
//! `EvalError` is in `ori_patterns`, `Diagnostic` is in `ori_diagnostic`, and neither
//! crate owns the other.
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

use std::fmt::Write;

use crate::eval::EvalErrorSnapshot;
use ori_diagnostic::span_utils::LineOffsetTable;
use ori_diagnostic::{Diagnostic, ErrorCode};
use ori_patterns::{EvalError, EvalErrorKind};

/// Convert an `EvalError` into a `Diagnostic`.
///
/// Maps `EvalErrorKind` to an E6xxx error code and constructs a diagnostic
/// with the error message, primary span label, notes, and backtrace context.
#[cold]
pub fn eval_error_to_diagnostic(err: &EvalError) -> Diagnostic {
    let code = error_code_for_kind(&err.kind);
    let mut diag = Diagnostic::error(code).with_message(&err.message);

    // Add primary label at the error span
    if let Some(span) = err.span {
        diag = diag.with_label(span, label_for_kind(&err.kind));
    }

    // Add context notes from the error
    for note in &err.notes {
        diag = diag.with_note(&note.message);
    }

    // Add backtrace as a note (if present)
    if let Some(ref bt) = err.backtrace {
        if !bt.is_empty() {
            diag = diag.with_note(format!("call stack:\n{bt}"));
        }
    }

    // Add suggestions for common fixable errors
    if let Some(suggestion) = suggestion_for_kind(&err.kind) {
        diag = diag.with_suggestion(suggestion);
    }

    diag
}

/// Convert an `EvalErrorSnapshot` into a `Diagnostic` with enriched file/line info.
///
/// Unlike [`eval_error_to_diagnostic`] which works with raw `EvalError` (and its
/// `EvalErrorKind` for error code mapping), this function works with the Salsa-compatible
/// snapshot and enriches backtrace spans with `file:line:col` using `LineOffsetTable`.
///
/// Falls back to byte offsets if source is unavailable.
#[cold]
pub fn snapshot_to_diagnostic(
    snapshot: &EvalErrorSnapshot,
    source: &str,
    file_path: &str,
) -> Diagnostic {
    let mut diag = Diagnostic::error(snapshot.error_code).with_message(&snapshot.message);

    let table = LineOffsetTable::build(source);

    // Add primary label at the error span
    if let Some(span) = snapshot.span {
        let (line, col) = table.offset_to_line_col(source, span.start);
        diag = diag.with_label(span, format!("runtime error at {file_path}:{line}:{col}"));
    }

    // Add context notes
    for note in &snapshot.notes {
        diag = diag.with_note(note);
    }

    // Add enriched backtrace as a note
    if !snapshot.backtrace.is_empty() {
        let mut bt_lines = String::from("call stack:");
        for (i, (name, span)) in snapshot.backtrace.iter().enumerate() {
            let _ = write!(bt_lines, "\n  {i}: {name}");
            if let Some(span) = span {
                let (line, col) = table.offset_to_line_col(source, span.start);
                let _ = write!(bt_lines, " at {file_path}:{line}:{col}");
            }
        }
        diag = diag.with_note(bt_lines);
    }

    diag
}

/// Map an `EvalErrorKind` to its corresponding `ErrorCode`.
pub(crate) fn error_code_for_kind(kind: &EvalErrorKind) -> ErrorCode {
    match kind {
        // Arithmetic
        EvalErrorKind::DivisionByZero => ErrorCode::E6001,
        EvalErrorKind::ModuloByZero => ErrorCode::E6002,
        EvalErrorKind::IntegerOverflow { .. } => ErrorCode::E6003,
        EvalErrorKind::SizeWouldBeNegative => ErrorCode::E6004,
        EvalErrorKind::SizeNegativeMultiply => ErrorCode::E6005,
        EvalErrorKind::SizeNegativeDivide => ErrorCode::E6006,

        // Type/Operator
        EvalErrorKind::TypeMismatch { .. } => ErrorCode::E6010,
        EvalErrorKind::InvalidBinaryOp { .. } => ErrorCode::E6011,
        EvalErrorKind::BinaryTypeMismatch { .. } => ErrorCode::E6012,

        // Access
        EvalErrorKind::UndefinedVariable { .. } => ErrorCode::E6020,
        EvalErrorKind::UndefinedFunction { .. } => ErrorCode::E6021,
        EvalErrorKind::UndefinedConst { .. } => ErrorCode::E6022,
        EvalErrorKind::UndefinedField { .. } => ErrorCode::E6023,
        EvalErrorKind::UndefinedMethod { .. } => ErrorCode::E6024,
        EvalErrorKind::IndexOutOfBounds { .. } => ErrorCode::E6025,
        EvalErrorKind::KeyNotFound { .. } => ErrorCode::E6026,
        EvalErrorKind::ImmutableBinding { .. } => ErrorCode::E6027,

        // Function
        EvalErrorKind::ArityMismatch { .. } => ErrorCode::E6030,
        EvalErrorKind::StackOverflow { .. } => ErrorCode::E6031,
        EvalErrorKind::NotCallable { .. } => ErrorCode::E6032,

        // Pattern
        EvalErrorKind::NonExhaustiveMatch => ErrorCode::E6040,

        // Assertion/Test
        EvalErrorKind::AssertionFailed { .. } => ErrorCode::E6050,
        EvalErrorKind::PanicCalled { .. } => ErrorCode::E6051,

        // Capability
        EvalErrorKind::MissingCapability { .. } => ErrorCode::E6060,

        // Const Eval
        EvalErrorKind::ConstEvalBudgetExceeded => ErrorCode::E6070,

        // Not Implemented
        EvalErrorKind::NotImplemented { .. } => ErrorCode::E6080,

        // Custom/catch-all
        EvalErrorKind::Custom { .. } => ErrorCode::E6099,
    }
}

/// Produce a concise label for the primary span.
fn label_for_kind(kind: &EvalErrorKind) -> &'static str {
    match kind {
        EvalErrorKind::DivisionByZero => "division by zero here",
        EvalErrorKind::ModuloByZero => "modulo by zero here",
        EvalErrorKind::IntegerOverflow { .. } => "overflow occurred here",
        EvalErrorKind::SizeWouldBeNegative => "result would be negative",
        EvalErrorKind::SizeNegativeMultiply | EvalErrorKind::SizeNegativeDivide => {
            "negative operand"
        }
        EvalErrorKind::TypeMismatch { .. } => "type mismatch",
        EvalErrorKind::InvalidBinaryOp { .. } => "operator not supported",
        EvalErrorKind::BinaryTypeMismatch { .. } => "mismatched types",
        EvalErrorKind::UndefinedVariable { .. } => "not found in this scope",
        EvalErrorKind::UndefinedFunction { .. } => "function not found",
        EvalErrorKind::UndefinedConst { .. } => "constant not found",
        EvalErrorKind::UndefinedField { .. } => "field not found",
        EvalErrorKind::UndefinedMethod { .. } => "method not found",
        EvalErrorKind::IndexOutOfBounds { .. } => "index out of bounds",
        EvalErrorKind::KeyNotFound { .. } => "key not found",
        EvalErrorKind::ImmutableBinding { .. } => "cannot assign to immutable binding",
        EvalErrorKind::ArityMismatch { .. } => "wrong number of arguments",
        EvalErrorKind::StackOverflow { .. } => "recursion limit exceeded",
        EvalErrorKind::NotCallable { .. } => "not callable",
        EvalErrorKind::NonExhaustiveMatch => "non-exhaustive match",
        EvalErrorKind::AssertionFailed { .. } => "assertion failed",
        EvalErrorKind::PanicCalled { .. } => "panic",
        EvalErrorKind::MissingCapability { .. } => "missing capability",
        EvalErrorKind::ConstEvalBudgetExceeded => "budget exceeded",
        EvalErrorKind::NotImplemented { .. } => "not implemented",
        EvalErrorKind::Custom { .. } => "runtime error",
    }
}

/// Produce an actionable suggestion for fixable errors.
fn suggestion_for_kind(kind: &EvalErrorKind) -> Option<String> {
    match kind {
        EvalErrorKind::DivisionByZero => Some("add a zero check before dividing".to_string()),
        EvalErrorKind::ImmutableBinding { name } => Some(format!(
            "declare `{name}` as mutable with `mut {name} = ...`"
        )),
        EvalErrorKind::NonExhaustiveMatch => {
            Some("add a wildcard `_` arm to cover remaining cases".to_string())
        }
        EvalErrorKind::NotImplemented { suggestion, .. } if !suggestion.is_empty() => {
            Some(suggestion.clone())
        }
        EvalErrorKind::MissingCapability { capability } => {
            Some(format!("add `uses {capability}` to the function signature"))
        }
        _ => None,
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
