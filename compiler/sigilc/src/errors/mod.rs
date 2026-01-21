// Error infrastructure for the Sigil compiler
//
// Provides structured error types with spans for rich diagnostics.
// Replaces ad-hoc `Result<T, String>` with `Result<T, Diagnostic>`.

pub mod codes;
pub mod render;

use std::ops::Range;

/// A source location span with filename
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The filename or source identifier
    pub filename: String,
    /// Byte offset range within the source
    pub range: Range<usize>,
}

impl Span {
    pub fn new(filename: impl Into<String>, range: Range<usize>) -> Self {
        Span {
            filename: filename.into(),
            range,
        }
    }

    /// Create a span covering both spans (must be same file)
    pub fn merge(&self, other: &Span) -> Span {
        debug_assert_eq!(self.filename, other.filename);
        Span {
            filename: self.filename.clone(),
            range: self.range.start.min(other.range.start)..self.range.end.max(other.range.end),
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Span {
            filename: "<unknown>".to_string(),
            range: 0..0,
        }
    }
}

/// Severity level for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Compilation cannot continue
    Error,
    /// Something suspicious but compilation can continue
    Warning,
    /// Informational message
    Note,
    /// Suggestion for improvement
    Help,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Error => write!(f, "error"),
            Level::Warning => write!(f, "warning"),
            Level::Note => write!(f, "note"),
            Level::Help => write!(f, "help"),
        }
    }
}

/// A labeled span for additional context in diagnostics
#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
    pub style: LabelStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    /// Primary label (the main error location)
    Primary,
    /// Secondary label (related locations)
    Secondary,
}

impl Label {
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Label {
            span,
            message: message.into(),
            style: LabelStyle::Primary,
        }
    }

    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Label {
            span,
            message: message.into(),
            style: LabelStyle::Secondary,
        }
    }
}

/// A structured compiler diagnostic
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Error/warning/note level
    pub level: Level,
    /// Error code for categorization
    pub code: codes::ErrorCode,
    /// Main error message
    pub message: String,
    /// Labeled spans showing error locations
    pub labels: Vec<Label>,
    /// Additional notes
    pub notes: Vec<String>,
    /// Help suggestions
    pub help: Vec<String>,
}

impl Diagnostic {
    /// Create a new error diagnostic
    pub fn error(code: codes::ErrorCode, message: impl Into<String>) -> Self {
        Diagnostic {
            level: Level::Error,
            code,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
        }
    }

    /// Create a new warning diagnostic
    pub fn warning(code: codes::ErrorCode, message: impl Into<String>) -> Self {
        Diagnostic {
            level: Level::Warning,
            code,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
        }
    }

    /// Add a primary label
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::primary(span, message));
        self
    }

    /// Add a secondary label
    pub fn with_secondary_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::secondary(span, message));
        self
    }

    /// Add a note
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Add a help suggestion
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help.push(help.into());
        self
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", render::render_simple(self))
    }
}

impl std::error::Error for Diagnostic {}

/// Convenience type alias for Results with Diagnostic errors
pub type DiagnosticResult<T> = Result<T, Diagnostic>;

/// Convert a String error to a Diagnostic (for gradual migration)
pub fn from_string_error(msg: String, filename: &str) -> Diagnostic {
    Diagnostic::error(codes::ErrorCode::E0000, msg)
        .with_label(Span::new(filename, 0..0), "error occurred here")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_creation() {
        let span = Span::new("test.si", 0..10);
        assert_eq!(span.filename, "test.si");
        assert_eq!(span.range, 0..10);
    }

    #[test]
    fn test_span_merge() {
        let span1 = Span::new("test.si", 5..10);
        let span2 = Span::new("test.si", 15..20);
        let merged = span1.merge(&span2);
        assert_eq!(merged.range, 5..20);
    }

    #[test]
    fn test_diagnostic_builder() {
        let diag = Diagnostic::error(codes::ErrorCode::E3001, "type mismatch")
            .with_label(Span::new("test.si", 10..20), "expected int")
            .with_note("types must match exactly")
            .with_help("consider using a type conversion");

        assert_eq!(diag.level, Level::Error);
        assert_eq!(diag.message, "type mismatch");
        assert_eq!(diag.labels.len(), 1);
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.help.len(), 1);
    }

    #[test]
    fn test_level_display() {
        assert_eq!(format!("{}", Level::Error), "error");
        assert_eq!(format!("{}", Level::Warning), "warning");
        assert_eq!(format!("{}", Level::Note), "note");
        assert_eq!(format!("{}", Level::Help), "help");
    }
}
