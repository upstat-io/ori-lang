// Error infrastructure for the Sigil compiler
//
// Provides structured error types with spans for rich diagnostics.
// Replaces ad-hoc `Result<T, String>` with `Result<T, Diagnostic>`.

pub mod codes;
pub mod collector;
pub mod json;
pub mod render;
pub mod result;

pub use collector::DiagnosticCollector;
pub use result::{from_string_result, PhaseResult};

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

/// A suggested fix for a diagnostic
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Description of the suggestion
    pub message: String,
    /// Optional replacement to apply
    pub replacement: Option<Replacement>,
}

/// A text replacement to fix an issue
#[derive(Debug, Clone)]
pub struct Replacement {
    /// The span to replace
    pub span: Span,
    /// The replacement text
    pub text: String,
}

impl Suggestion {
    /// Create a suggestion with a message only (no automatic fix).
    pub fn hint(message: impl Into<String>) -> Self {
        Suggestion {
            message: message.into(),
            replacement: None,
        }
    }

    /// Create a suggestion with a replacement.
    pub fn with_replacement(
        message: impl Into<String>,
        span: Span,
        text: impl Into<String>,
    ) -> Self {
        Suggestion {
            message: message.into(),
            replacement: Some(Replacement {
                span,
                text: text.into(),
            }),
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
    /// Suggested fixes (for AI tooling and quick-fixes)
    pub suggestions: Vec<Suggestion>,
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
            suggestions: Vec::new(),
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
            suggestions: Vec::new(),
        }
    }

    /// Create a new note diagnostic
    pub fn note(code: codes::ErrorCode, message: impl Into<String>) -> Self {
        Diagnostic {
            level: Level::Note,
            code,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
            suggestions: Vec::new(),
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

    /// Add a suggestion (for AI tooling and quick-fixes)
    pub fn with_suggestion(
        mut self,
        message: impl Into<String>,
        span: Option<Span>,
        replacement: Option<String>,
    ) -> Self {
        let suggestion = match (span, replacement) {
            (Some(span), Some(text)) => Suggestion::with_replacement(message, span, text),
            _ => Suggestion::hint(message),
        };
        self.suggestions.push(suggestion);
        self
    }

    /// Add a suggestion object directly
    pub fn with_suggestion_obj(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
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

/// Result type that can hold multiple diagnostics (errors and warnings)
/// Use this when you want to accumulate and report all errors at once
pub type MultiDiagnosticResult<T> = Result<T, Vec<Diagnostic>>;

/// Convert a String error to a Diagnostic (for gradual migration)
pub fn from_string_error(msg: String, filename: &str) -> Diagnostic {
    Diagnostic::error(codes::ErrorCode::E0000, msg)
        .with_label(Span::new(filename, 0..0), "error occurred here")
}

/// Trait for converting String errors to Diagnostic
pub trait IntoDiagnostic {
    fn into_diagnostic(self, filename: &str) -> Diagnostic;
}

impl IntoDiagnostic for String {
    fn into_diagnostic(self, filename: &str) -> Diagnostic {
        from_string_error(self, filename)
    }
}

/// Extension trait for Result<T, String> -> DiagnosticResult<T>
pub trait ResultExt<T> {
    fn map_err_diagnostic(self, filename: &str) -> DiagnosticResult<T>;
}

impl<T> ResultExt<T> for Result<T, String> {
    fn map_err_diagnostic(self, filename: &str) -> DiagnosticResult<T> {
        self.map_err(|e| from_string_error(e, filename))
    }
}

// Convenience constructors for common errors

/// Create a type mismatch diagnostic
pub fn type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        codes::ErrorCode::E3001,
        format!("type mismatch: expected {}, found {}", expected, found),
    )
    .with_label(span, format!("expected {}", expected))
}

/// Create an unknown identifier diagnostic
pub fn unknown_identifier(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        codes::ErrorCode::E3002,
        format!("unknown identifier '{}'", name),
    )
    .with_label(span, "not found in this scope")
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

    #[test]
    fn test_into_diagnostic() {
        let error = "some error".to_string();
        let diag = error.into_diagnostic("test.si");
        assert_eq!(diag.level, Level::Error);
        assert!(diag.message.contains("some error"));
    }

    #[test]
    fn test_result_ext() {
        let ok_result: Result<i32, String> = Ok(42);
        let converted = ok_result.map_err_diagnostic("test.si");
        assert_eq!(converted.unwrap(), 42);

        let err_result: Result<i32, String> = Err("failed".to_string());
        let converted = err_result.map_err_diagnostic("test.si");
        assert!(converted.is_err());
        let diag = converted.unwrap_err();
        assert!(diag.message.contains("failed"));
    }

    #[test]
    fn test_type_mismatch() {
        let diag = type_mismatch("int", "string", Span::new("test.si", 10..20));
        assert_eq!(diag.level, Level::Error);
        assert!(diag.message.contains("type mismatch"));
        assert!(diag.message.contains("int"));
        assert!(diag.message.contains("string"));
    }

    #[test]
    fn test_unknown_identifier() {
        let diag = unknown_identifier("foo", Span::new("test.si", 5..8));
        assert_eq!(diag.level, Level::Error);
        assert!(diag.message.contains("foo"));
        assert!(diag.message.contains("unknown identifier"));
    }
}
