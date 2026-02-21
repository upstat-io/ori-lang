//! Salsa-compatible eval error snapshot to diagnostic conversion.
//!
//! The core `EvalError` → `Diagnostic` conversion (E6xxx error codes) lives in
//! `ori_patterns::errors::diagnostics`, where `EvalError` and `EvalErrorKind` are
//! defined. This module handles the Salsa-specific `EvalErrorSnapshot` → `Diagnostic`
//! conversion, which enriches backtraces with file/line/col from `LineOffsetTable`.

use std::fmt::Write;

use crate::eval::EvalErrorSnapshot;
use ori_diagnostic::span_utils::LineOffsetTable;
use ori_diagnostic::Diagnostic;

/// Convert an `EvalErrorSnapshot` into a `Diagnostic` with enriched file/line info.
///
/// Unlike [`EvalError::to_diagnostic()`](ori_patterns::EvalError::to_diagnostic)
/// which works with raw `EvalError` (and its `EvalErrorKind` for error code mapping),
/// this function works with the Salsa-compatible snapshot and enriches backtrace
/// spans with `file:line:col` using `LineOffsetTable`.
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

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
