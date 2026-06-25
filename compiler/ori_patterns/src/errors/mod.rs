//! Error types for pattern evaluation.
//!
//! This module provides the core error types used during pattern evaluation
//! and the evaluator's runtime.
//!
//! # Structured Error Categories
//!
//! `EvalErrorKind` provides typed error categories for diagnostic conversion.
//! Factory functions (e.g., `division_by_zero()`) are the public API —
//! they populate both `kind` and `message`.
//!
//! # Diagnostic Conversion
//!
//! `EvalErrorKind::error_code()`, `EvalErrorKind::primary_label()`, and
//! `EvalError::to_diagnostic()` convert runtime errors into rich diagnostics
//! with E6xxx error codes. See [`diagnostics`] submodule.

pub mod diagnostics;
mod factories;
mod kind;

pub use factories::*;
pub use kind::EvalErrorKind;

use crate::value::Value;
use ori_ir::{Name, Span};
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
    /// Break from a loop, optionally with a value. `label` is the target loop
    /// label (`Name::EMPTY` = innermost; non-empty = nearest enclosing match).
    /// Spec: Clause 16.3.3.
    Break(Value, Name),
    /// Continue to next iteration, optionally with a substitution value
    /// (for...yield). `label` is the target loop label (`Name::EMPTY` =
    /// innermost). Spec: Clause 16.3.3.
    Continue(Value, Name),
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
            ControlAction::Break(v, _) => EvalError::new(format!("break:{v}")),
            ControlAction::Continue(v, _) => EvalError::new(format!("continue:{v}")),
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
    /// For factory-created errors, this equals `kind.to_string()`; consumers
    /// read it directly via `error.message`.
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
    /// Used by factory functions here and in downstream crates (e.g., `ori_eval::errors`).
    pub fn from_kind(kind: EvalErrorKind) -> Self {
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

#[cfg(test)]
mod tests;
