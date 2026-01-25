//! Diagnostic system for rich error reporting.
//!
//! Per design spec 02-design-principlesmd:
//! - Error codes for searchability
//! - Clear messages (what went wrong)
//! - Primary span (where it went wrong)
//! - Context labels (why it's wrong)
//! - Suggestions (how to fix)

pub mod emitter;
pub mod fixes;
pub mod queue;
pub mod span_utils;

use sigil_ir::Span;
use std::fmt;

/// Error codes for all compiler diagnostics.
///
/// Format: E#### where first digit indicates phase:
/// - E0xxx: Lexer errors
/// - E1xxx: Parser errors
/// - E2xxx: Type errors
/// - E3xxx: Pattern errors
/// - E9xxx: Internal compiler errors
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ErrorCode {
    // ===== Lexer Errors (E0xxx) =====
    /// Unterminated string literal
    E0001,
    /// Invalid character in source
    E0002,
    /// Invalid number literal
    E0003,
    /// Unterminated character literal
    E0004,
    /// Invalid escape sequence
    E0005,

    // ===== Parser Errors (E1xxx) =====
    /// Unexpected token
    E1001,
    /// Expected expression
    E1002,
    /// Unclosed delimiter
    E1003,
    /// Expected identifier
    E1004,
    /// Expected type
    E1005,
    /// Invalid function definition
    E1006,
    /// Missing function body
    E1007,
    /// Invalid pattern syntax
    E1008,
    /// Missing pattern argument
    E1009,
    /// Unknown pattern argument
    E1010,
    /// Multi-arg function call requires named arguments
    E1011,
    /// Invalid `function_seq` syntax
    E1012,
    /// `function_exp` requires named properties
    E1013,
    /// Reserved built-in function name
    E1014,

    // ===== Type Errors (E2xxx) =====
    /// Type mismatch
    E2001,
    /// Unknown type
    E2002,
    /// Unknown identifier
    E2003,
    /// Argument count mismatch
    E2004,
    /// Cannot infer type
    E2005,
    /// Duplicate definition
    E2006,
    /// Closure self-reference (closure cannot capture itself)
    E2007,
    /// Cyclic type definition
    E2008,
    /// Missing trait bound
    E2009,
    /// Coherence violation (conflicting implementations)
    E2010,

    // ===== Pattern Errors (E3xxx) =====
    /// Unknown pattern
    E3001,
    /// Invalid pattern arguments
    E3002,
    /// Pattern type error
    E3003,

    // ===== Internal Errors (E9xxx) =====
    /// Internal compiler error
    E9001,
    /// Too many errors
    E9002,
}

impl ErrorCode {
    /// Get the numeric code as a string (e.g., "E1001").
    pub fn as_str(&self) -> &'static str {
        match self {
            // Lexer
            ErrorCode::E0001 => "E0001",
            ErrorCode::E0002 => "E0002",
            ErrorCode::E0003 => "E0003",
            ErrorCode::E0004 => "E0004",
            ErrorCode::E0005 => "E0005",
            // Parser
            ErrorCode::E1001 => "E1001",
            ErrorCode::E1002 => "E1002",
            ErrorCode::E1003 => "E1003",
            ErrorCode::E1004 => "E1004",
            ErrorCode::E1005 => "E1005",
            ErrorCode::E1006 => "E1006",
            ErrorCode::E1007 => "E1007",
            ErrorCode::E1008 => "E1008",
            ErrorCode::E1009 => "E1009",
            ErrorCode::E1010 => "E1010",
            ErrorCode::E1011 => "E1011",
            ErrorCode::E1012 => "E1012",
            ErrorCode::E1013 => "E1013",
            ErrorCode::E1014 => "E1014",
            // Type
            ErrorCode::E2001 => "E2001",
            ErrorCode::E2002 => "E2002",
            ErrorCode::E2003 => "E2003",
            ErrorCode::E2004 => "E2004",
            ErrorCode::E2005 => "E2005",
            ErrorCode::E2006 => "E2006",
            ErrorCode::E2007 => "E2007",
            ErrorCode::E2008 => "E2008",
            ErrorCode::E2009 => "E2009",
            ErrorCode::E2010 => "E2010",
            // Pattern
            ErrorCode::E3001 => "E3001",
            ErrorCode::E3002 => "E3002",
            ErrorCode::E3003 => "E3003",
            // Internal
            ErrorCode::E9001 => "E9001",
            ErrorCode::E9002 => "E9002",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Severity level for diagnostics.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
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

/// Applicability level for code suggestions.
///
/// Indicates how confident we are that a suggestion is correct,
/// enabling `sigil fix` to safely auto-apply machine-applicable fixes.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum Applicability {
    /// The suggestion is definitely correct and can be auto-applied.
    /// Used for simple fixes like typos, missing semicolons, etc.
    MachineApplicable,

    /// The suggestion might be correct but requires human verification.
    /// Used when we're fairly confident but there could be edge cases.
    MaybeIncorrect,

    /// The suggestion contains placeholders that need user input.
    /// For example: "consider adding a type annotation: `: <type>`"
    HasPlaceholders,

    /// We don't know how confident the suggestion is.
    /// Default for suggestions where applicability wasn't specified.
    #[default]
    Unspecified,
}

impl Applicability {
    /// Check if this suggestion can be safely auto-applied.
    pub fn is_machine_applicable(&self) -> bool {
        matches!(self, Applicability::MachineApplicable)
    }
}

/// A text substitution for a code fix.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Substitution {
    /// The span to replace.
    pub span: Span,
    /// The replacement text.
    pub snippet: String,
}

impl Substitution {
    /// Create a new substitution.
    pub fn new(span: Span, snippet: impl Into<String>) -> Self {
        Substitution {
            span,
            snippet: snippet.into(),
        }
    }
}

/// A structured suggestion with substitutions and applicability.
///
/// Unlike simple string suggestions, this provides:
/// - Exact spans for what to replace
/// - Replacement text
/// - Confidence level for auto-application
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Suggestion {
    /// Human-readable message describing the fix.
    pub message: String,
    /// The text substitutions to make.
    pub substitutions: Vec<Substitution>,
    /// How confident we are in this suggestion.
    pub applicability: Applicability,
}

impl Suggestion {
    /// Create a new suggestion with a single substitution.
    pub fn new(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
        applicability: Applicability,
    ) -> Self {
        Suggestion {
            message: message.into(),
            substitutions: vec![Substitution::new(span, snippet)],
            applicability,
        }
    }

    /// Create a machine-applicable suggestion (safe to auto-apply).
    pub fn machine_applicable(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::MachineApplicable)
    }

    /// Create a suggestion that might be incorrect.
    pub fn maybe_incorrect(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::MaybeIncorrect)
    }

    /// Create a suggestion with placeholders.
    pub fn has_placeholders(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::HasPlaceholders)
    }

    /// Add another substitution to this suggestion.
    #[must_use]
    pub fn with_substitution(mut self, span: Span, snippet: impl Into<String>) -> Self {
        self.substitutions.push(Substitution::new(span, snippet));
        self
    }
}

/// A labeled span with a message.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Label {
    pub span: Span,
    pub message: String,
    pub is_primary: bool,
}

impl Label {
    /// Create a primary label (the main error location).
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Label {
            span,
            message: message.into(),
            is_primary: true,
        }
    }

    /// Create a secondary label (related context).
    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Label {
            span,
            message: message.into(),
            is_primary: false,
        }
    }
}

/// A rich diagnostic with all context needed for great error messages.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
#[must_use = "diagnostics should be reported or returned, not silently dropped"]
pub struct Diagnostic {
    /// Error code for searchability.
    pub code: ErrorCode,
    /// Severity level.
    pub severity: Severity,
    /// Main error message.
    pub message: String,
    /// Labeled spans showing where the error occurred.
    pub labels: Vec<Label>,
    /// Additional notes providing context.
    pub notes: Vec<String>,
    /// Simple text suggestions for fixing the error (human-readable).
    pub suggestions: Vec<String>,
    /// Structured suggestions with spans and applicability (for `sigil fix`).
    pub structured_suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// Create a new error diagnostic.
    pub fn error(code: ErrorCode) -> Self {
        Diagnostic {
            code,
            severity: Severity::Error,
            message: String::new(),
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            structured_suggestions: Vec::new(),
        }
    }

    /// Create a new warning diagnostic.
    pub fn warning(code: ErrorCode) -> Self {
        Diagnostic {
            code,
            severity: Severity::Warning,
            message: String::new(),
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            structured_suggestions: Vec::new(),
        }
    }

    /// Set the main message.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Add a primary label at the error location.
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::primary(span, message));
        self
    }

    /// Add a secondary label for context.
    pub fn with_secondary_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::secondary(span, message));
        self
    }

    /// Add a note providing additional context.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Add a suggestion for fixing the error.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Add a structured suggestion with applicability information.
    ///
    /// Structured suggestions can be used by `sigil fix` to auto-apply fixes.
    pub fn with_structured_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.structured_suggestions.push(suggestion);
        self
    }

    /// Add a machine-applicable suggestion (safe to auto-apply).
    ///
    /// Use this for fixes that are definitely correct:
    /// - Typo corrections
    /// - Missing delimiters
    /// - Simple syntax fixes
    pub fn with_fix(
        mut self,
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        self.structured_suggestions.push(Suggestion::machine_applicable(message, span, snippet));
        self
    }

    /// Add a suggestion that might be incorrect.
    ///
    /// Use this when we're fairly confident but not certain:
    /// - Type conversions
    /// - Import suggestions
    pub fn with_maybe_fix(
        mut self,
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        self.structured_suggestions.push(Suggestion::maybe_incorrect(message, span, snippet));
        self
    }

    /// Get the primary span (first primary label's span).
    pub fn primary_span(&self) -> Option<Span> {
        self.labels.iter().find(|l| l.is_primary).map(|l| l.span)
    }

    /// Check if this is an error (vs warning/note).
    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }

    /// Check if this diagnostic has any machine-applicable fixes.
    pub fn has_machine_applicable_fix(&self) -> bool {
        self.structured_suggestions.iter().any(|s| s.applicability.is_machine_applicable())
    }

    /// Get all machine-applicable suggestions.
    pub fn machine_applicable_fixes(&self) -> impl Iterator<Item = &Suggestion> {
        self.structured_suggestions.iter().filter(|s| s.applicability.is_machine_applicable())
    }
}

// ===== Diagnostic Helpers (DRY) =====

/// Create a "type mismatch" diagnostic.
pub fn type_mismatch(
    span: Span,
    expected: &str,
    found: &str,
    context: &str,
) -> Diagnostic {
    Diagnostic::error(ErrorCode::E2001)
        .with_message(format!("type mismatch: expected `{expected}`, found `{found}`"))
        .with_label(span, context)
}

/// Create an "unexpected token" diagnostic.
pub fn unexpected_token(
    span: Span,
    expected: &str,
    found: &str,
) -> Diagnostic {
    Diagnostic::error(ErrorCode::E1001)
        .with_message(format!("unexpected token: expected {expected}, found `{found}`"))
        .with_label(span, format!("expected {expected}"))
}

/// Create an "expected expression" diagnostic.
pub fn expected_expression(span: Span, found: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E1002)
        .with_message(format!("expected expression, found `{found}`"))
        .with_label(span, "expected expression here")
}

/// Create an "unclosed delimiter" diagnostic.
pub fn unclosed_delimiter(
    open_span: Span,
    close_span: Span,
    delimiter: char,
) -> Diagnostic {
    let expected = match delimiter {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => delimiter,
    };
    Diagnostic::error(ErrorCode::E1003)
        .with_message(format!("unclosed delimiter `{delimiter}`"))
        .with_label(close_span, format!("expected `{expected}`"))
        .with_secondary_label(open_span, "unclosed delimiter opened here")
}

/// Create an "unknown identifier" diagnostic.
pub fn unknown_identifier(span: Span, name: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E2003)
        .with_message(format!("unknown identifier `{name}`"))
        .with_label(span, "not found in this scope")
}

/// Create a "missing pattern argument" diagnostic.
pub fn missing_pattern_arg(span: Span, pattern: &str, arg: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E1009)
        .with_message(format!("missing required argument `.{arg}:` in `{pattern}` pattern"))
        .with_label(span, format!("missing `.{arg}:`"))
        .with_suggestion(format!("add `.{arg}: <value>` to the pattern arguments"))
}

/// Create an "unknown pattern argument" diagnostic.
pub fn unknown_pattern_arg(span: Span, pattern: &str, arg: &str, valid: &[&str]) -> Diagnostic {
    let valid_list = valid.join("`, `.");
    Diagnostic::error(ErrorCode::E1010)
        .with_message(format!("unknown argument `.{arg}:` in `{pattern}` pattern"))
        .with_label(span, "unknown argument")
        .with_note(format!("valid arguments are: `.{valid_list}`"))
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]: {}", self.severity, self.code, self.message)?;

        for label in &self.labels {
            let marker = if label.is_primary { "-->" } else { "   " };
            write!(f, "\n  {} {:?}: {}", marker, label.span, label.message)?;
        }

        for note in &self.notes {
            write!(f, "\n  = note: {note}")?;
        }

        for suggestion in &self.suggestions {
            write!(f, "\n  = help: {suggestion}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_display() {
        assert_eq!(ErrorCode::E1001.to_string(), "E1001");
        assert_eq!(ErrorCode::E2001.as_str(), "E2001");
    }

    #[test]
    fn test_diagnostic_builder() {
        let diag = Diagnostic::error(ErrorCode::E1001)
            .with_message("test error")
            .with_label(Span::new(0, 5), "here")
            .with_note("some context")
            .with_suggestion("try this");

        assert_eq!(diag.code, ErrorCode::E1001);
        assert_eq!(diag.message, "test error");
        assert!(diag.is_error());
        assert_eq!(diag.labels.len(), 1);
        assert!(diag.labels[0].is_primary);
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.suggestions.len(), 1);
    }

    #[test]
    fn test_type_mismatch_helper() {
        let diag = type_mismatch(
            Span::new(10, 15),
            "int",
            "str",
            "return value",
        );

        assert_eq!(diag.code, ErrorCode::E2001);
        assert!(diag.message.contains("int"));
        assert!(diag.message.contains("str"));
        assert_eq!(diag.primary_span(), Some(Span::new(10, 15)));
    }

    #[test]
    fn test_unclosed_delimiter() {
        let diag = unclosed_delimiter(
            Span::new(0, 1),
            Span::new(10, 10),
            '(',
        );

        assert_eq!(diag.code, ErrorCode::E1003);
        assert_eq!(diag.labels.len(), 2);
        assert!(diag.labels[0].is_primary);
        assert!(!diag.labels[1].is_primary);
    }

    #[test]
    fn test_missing_pattern_arg() {
        let diag = missing_pattern_arg(
            Span::new(0, 10),
            "map",
            "over",
        );

        assert_eq!(diag.code, ErrorCode::E1009);
        assert!(diag.message.contains("over"));
        assert!(diag.message.contains("map"));
        assert!(!diag.suggestions.is_empty());
    }

    #[test]
    fn test_diagnostic_display() {
        let diag = Diagnostic::error(ErrorCode::E1001)
            .with_message("test error")
            .with_label(Span::new(0, 5), "here");

        let output = diag.to_string();
        assert!(output.contains("error"));
        assert!(output.contains("E1001"));
        assert!(output.contains("test error"));
    }

    #[test]
    fn test_diagnostic_salsa_traits() {
        use std::collections::HashSet;

        let d1 = Diagnostic::error(ErrorCode::E1001).with_message("test");
        let d2 = Diagnostic::error(ErrorCode::E1001).with_message("test");
        let d3 = Diagnostic::error(ErrorCode::E1002).with_message("other");

        // Eq
        assert_eq!(d1, d2);
        assert_ne!(d1, d3);

        // Hash
        let mut set = HashSet::new();
        set.insert(d1.clone());
        set.insert(d2); // duplicate
        set.insert(d3);
        assert_eq!(set.len(), 2);
    }
}
