//! Diagnostic system for error reporting.
//!
//! This module provides structured error reporting with:
//! - Severity levels (error, warning, note, help)
//! - Source spans for error locations
//! - Labels for primary and secondary information
//! - Suggestions for fixes

use crate::syntax::Span;
use std::fmt;

/// Severity level of a diagnostic.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Note => write!(f, "note"),
            Severity::Help => write!(f, "help"),
        }
    }
}

/// A diagnostic message with source location.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// Severity level.
    pub severity: Severity,
    /// Error code (e.g., E0001).
    pub code: Option<String>,
    /// Main message.
    pub message: String,
    /// Primary span where the error occurred.
    pub span: Span,
    /// Additional labels.
    pub labels: Vec<Label>,
    /// Notes attached to this diagnostic.
    pub notes: Vec<String>,
    /// Suggestions for fixes.
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// Create a new error diagnostic.
    pub fn error(message: String, span: Span) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code: None,
            message,
            span,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Create a new warning diagnostic.
    pub fn warning(message: String, span: Span) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code: None,
            message,
            span,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Add an error code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Add a primary label at the main span.
    pub fn with_label(mut self, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span: self.span,
            message: message.into(),
            is_primary: true,
        });
        self
    }

    /// Add a secondary label at a different span.
    pub fn with_secondary_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
            is_primary: false,
        });
        self
    }

    /// Add a note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Add a suggestion.
    pub fn with_suggestion(
        mut self,
        span: Span,
        message: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.suggestions.push(Suggestion {
            span,
            message: message.into(),
            replacement: replacement.into(),
        });
        self
    }

    /// Check if this is an error.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// A label pointing to a span in the source.
#[derive(Clone, Debug)]
pub struct Label {
    /// Span in the source.
    pub span: Span,
    /// Label message.
    pub message: String,
    /// Whether this is the primary label.
    pub is_primary: bool,
}

/// A suggestion for fixing an error.
#[derive(Clone, Debug)]
pub struct Suggestion {
    /// Span to replace.
    pub span: Span,
    /// Description of the fix.
    pub message: String,
    /// Replacement text.
    pub replacement: String,
}

/// Collection of diagnostics with convenience methods.
#[derive(Clone, Debug, Default)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::error(message.into(), span));
    }

    pub fn warning(&mut self, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::warning(message.into(), span));
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    pub fn extend(&mut self, other: impl IntoIterator<Item = Diagnostic>) {
        self.diagnostics.extend(other);
    }
}

impl IntoIterator for DiagnosticBag {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.into_iter()
    }
}

impl<'a> IntoIterator for &'a DiagnosticBag {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.iter()
    }
}

impl FromIterator<Diagnostic> for DiagnosticBag {
    fn from_iter<I: IntoIterator<Item = Diagnostic>>(iter: I) -> Self {
        DiagnosticBag {
            diagnostics: iter.into_iter().collect(),
        }
    }
}
