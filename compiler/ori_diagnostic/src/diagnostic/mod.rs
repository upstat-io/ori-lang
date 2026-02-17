//! Core diagnostic types for structured error reporting.
//!
//! Defines [`Diagnostic`], [`Label`], [`Severity`], and [`Suggestion`] — the
//! building blocks that all compiler phases use to report errors and warnings.

use ori_ir::Span;
use std::fmt;

use crate::ErrorCode;

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
/// enabling `ori fix` to safely auto-apply machine-applicable fixes.
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
/// Supports two forms:
/// - **Text-only**: A human-readable message with no code substitutions.
///   Created via `text()`, `did_you_mean()`, `wrap_in()`.
/// - **Span-bearing**: A message with exact code substitutions for `ori fix`.
///   Created via `new()`, `machine_applicable()`, `maybe_incorrect()`, etc.
///
/// Suggestions have a `priority` field (lower = more likely relevant) used
/// for ordering when multiple suggestions are presented.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Suggestion {
    /// Human-readable message describing the fix.
    pub message: String,
    /// The text substitutions to make (empty for text-only suggestions).
    pub substitutions: Vec<Substitution>,
    /// How confident we are in this suggestion.
    pub applicability: Applicability,
    /// Priority (lower = more likely to be relevant).
    /// 0 = most likely, 1 = likely, 2 = possible, 3 = unlikely.
    pub priority: u8,
}

impl Suggestion {
    /// Create a new suggestion with a single substitution.
    pub fn new(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
        applicability: Applicability,
        priority: u8,
    ) -> Self {
        Suggestion {
            message: message.into(),
            substitutions: vec![Substitution::new(span, snippet)],
            applicability,
            priority,
        }
    }

    /// Create a text-only suggestion (no code substitution).
    pub fn text(message: impl Into<String>, priority: u8) -> Self {
        Suggestion {
            message: message.into(),
            substitutions: Vec::new(),
            applicability: Applicability::Unspecified,
            priority,
        }
    }

    /// Create a text-only suggestion with a single code replacement.
    pub fn text_with_replacement(
        message: impl Into<String>,
        priority: u8,
        span: Span,
        new_text: impl Into<String>,
    ) -> Self {
        Suggestion {
            message: message.into(),
            substitutions: vec![Substitution::new(span, new_text)],
            applicability: Applicability::MaybeIncorrect,
            priority,
        }
    }

    /// Create a "did you mean" suggestion (priority 0).
    pub fn did_you_mean(suggestion: impl Into<String>) -> Self {
        Self::text(format!("did you mean `{}`?", suggestion.into()), 0)
    }

    /// Create a suggestion to wrap in something (priority 1).
    pub fn wrap_in(wrapper: &str, example: &str) -> Self {
        Self::text(format!("wrap the value in `{wrapper}`: `{example}`"), 1)
    }

    /// Create a machine-applicable suggestion (safe to auto-apply).
    pub fn machine_applicable(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::MachineApplicable, 0)
    }

    /// Create a suggestion that might be incorrect.
    pub fn maybe_incorrect(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::MaybeIncorrect, 0)
    }

    /// Create a suggestion with placeholders.
    pub fn has_placeholders(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::HasPlaceholders, 0)
    }

    /// Add another substitution to this suggestion.
    #[must_use]
    pub fn with_substitution(mut self, span: Span, snippet: impl Into<String>) -> Self {
        self.substitutions.push(Substitution::new(span, snippet));
        self
    }

    /// Check if this is a text-only suggestion (no code substitutions).
    pub fn is_text_only(&self) -> bool {
        self.substitutions.is_empty()
    }
}

/// Source information for cross-file error labels.
///
/// When an error references code in a different file (e.g., showing where
/// an imported function is defined), this struct provides the file path
/// and content needed to render the snippet.
///
/// # Example
///
/// ```text
/// error[E2001]: type mismatch
///   --> src/main.ori:10:5
///    |
/// 10 |     let x: int = get_name()
///    |                  ^^^^^^^^^^ expected `int`, found `str`
///    |
///   ::: src/lib.ori:25:1
///    |
/// 25 | @get_name () -> str
///    | ------------------- return type defined here
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceInfo {
    /// The file path relative to the project root.
    pub path: String,
    /// The source content (or relevant portion).
    ///
    /// For efficiency, this may contain just the lines around the span
    /// rather than the entire file.
    pub content: String,
}

impl SourceInfo {
    /// Create new source info for a file.
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        SourceInfo {
            path: path.into(),
            content: content.into(),
        }
    }
}

/// A labeled span with a message.
///
/// Labels highlight specific locations in source code and attach explanatory
/// messages. They can reference the primary file or cross-reference other files.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Label {
    /// The source location to highlight.
    pub span: Span,
    /// The label text explaining this location.
    pub message: String,
    /// Whether this is the primary error location.
    pub is_primary: bool,
    /// Optional source info for cross-file references.
    ///
    /// When `None`, the label refers to the primary file being diagnosed.
    /// When `Some`, the label refers to a different file and the emitter
    /// should render it with `::: path` notation.
    pub source_info: Option<SourceInfo>,
}

impl Label {
    /// Create a primary label (the main error location).
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Label {
            span,
            message: message.into(),
            is_primary: true,
            source_info: None,
        }
    }

    /// Create a secondary label (related context).
    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Label {
            span,
            message: message.into(),
            is_primary: false,
            source_info: None,
        }
    }

    /// Create a primary label referencing a different file.
    pub fn primary_cross_file(
        span: Span,
        message: impl Into<String>,
        source_info: SourceInfo,
    ) -> Self {
        Label {
            span,
            message: message.into(),
            is_primary: true,
            source_info: Some(source_info),
        }
    }

    /// Create a secondary label referencing a different file.
    pub fn secondary_cross_file(
        span: Span,
        message: impl Into<String>,
        source_info: SourceInfo,
    ) -> Self {
        Label {
            span,
            message: message.into(),
            is_primary: false,
            source_info: Some(source_info),
        }
    }

    /// Check if this label references a different file.
    pub fn is_cross_file(&self) -> bool {
        self.source_info.is_some()
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
    /// Structured suggestions with spans and applicability (for `ori fix`).
    pub structured_suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// Create a new diagnostic with the given severity.
    fn new_with_severity(code: ErrorCode, severity: Severity) -> Self {
        Diagnostic {
            code,
            severity,
            message: String::new(),
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            structured_suggestions: Vec::new(),
        }
    }

    /// Create a new error diagnostic.
    #[cold]
    pub fn error(code: ErrorCode) -> Self {
        Self::new_with_severity(code, Severity::Error)
    }

    /// Create a new warning diagnostic.
    #[cold]
    pub fn warning(code: ErrorCode) -> Self {
        Self::new_with_severity(code, Severity::Warning)
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

    /// Add a primary label referencing a different file.
    ///
    /// Cross-file labels are rendered with `::: path` notation to distinguish
    /// them from same-file labels.
    pub fn with_cross_file_label(
        mut self,
        span: Span,
        message: impl Into<String>,
        source_info: SourceInfo,
    ) -> Self {
        self.labels
            .push(Label::primary_cross_file(span, message, source_info));
        self
    }

    /// Add a secondary label referencing a different file.
    ///
    /// Use this for related context in other files, such as:
    /// - Where an imported symbol is defined
    /// - Where a type was declared
    /// - Where a conflicting definition exists
    pub fn with_cross_file_secondary_label(
        mut self,
        span: Span,
        message: impl Into<String>,
        source_info: SourceInfo,
    ) -> Self {
        self.labels
            .push(Label::secondary_cross_file(span, message, source_info));
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
    /// Structured suggestions can be used by `ori fix` to auto-apply fixes.
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
        self.structured_suggestions
            .push(Suggestion::machine_applicable(message, span, snippet));
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
        self.structured_suggestions
            .push(Suggestion::maybe_incorrect(message, span, snippet));
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
        self.structured_suggestions
            .iter()
            .any(|s| s.applicability.is_machine_applicable())
    }

    /// Get all machine-applicable suggestions.
    pub fn machine_applicable_fixes(&self) -> impl Iterator<Item = &Suggestion> {
        self.structured_suggestions
            .iter()
            .filter(|s| s.applicability.is_machine_applicable())
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]: {}", self.severity, self.code, self.message)?;

        for label in &self.labels {
            // Cross-file labels use `::: path` notation
            let marker = if label.is_cross_file() {
                ":::"
            } else if label.is_primary {
                "-->"
            } else {
                "   "
            };

            // Include file path for cross-file labels
            if let Some(ref src) = label.source_info {
                write!(
                    f,
                    "\n  {} {} {:?}: {}",
                    marker, src.path, label.span, label.message
                )?;
            } else {
                write!(f, "\n  {} {:?}: {}", marker, label.span, label.message)?;
            }
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

/// Configuration for a type mismatch diagnostic.
///
/// Used by `type_mismatch` to create a diagnostic with all relevant context.
/// This config struct pattern improves API clarity for functions with 4+ parameters.
#[derive(Clone, Debug)]
pub struct TypeMismatchConfig<'a> {
    /// The source location of the mismatch.
    pub span: Span,
    /// The expected type name.
    pub expected: &'a str,
    /// The found type name.
    pub found: &'a str,
    /// Context describing where the mismatch occurred (e.g., "return value").
    pub context: &'a str,
}

impl<'a> TypeMismatchConfig<'a> {
    /// Create a new type mismatch configuration.
    pub fn new(span: Span, expected: &'a str, found: &'a str, context: &'a str) -> Self {
        TypeMismatchConfig {
            span,
            expected,
            found,
            context,
        }
    }

    /// Convert this configuration into a diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        Diagnostic::error(ErrorCode::E2001)
            .with_message(format!(
                "type mismatch: expected `{}`, found `{}`",
                self.expected, self.found
            ))
            .with_label(self.span, self.context)
    }
}

/// Create a "type mismatch" diagnostic.
///
/// For more explicit parameter naming, use `TypeMismatchConfig::new(...).into_diagnostic()`.
pub fn type_mismatch(span: Span, expected: &str, found: &str, context: &str) -> Diagnostic {
    TypeMismatchConfig::new(span, expected, found, context).into_diagnostic()
}

/// Create an "unexpected token" diagnostic.
pub fn unexpected_token(span: Span, expected: &str, found: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E1001)
        .with_message(format!(
            "unexpected token: expected {expected}, found `{found}`"
        ))
        .with_label(span, format!("expected {expected}"))
}

/// Create an "expected expression" diagnostic.
pub fn expected_expression(span: Span, found: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E1002)
        .with_message(format!("expected expression, found `{found}`"))
        .with_label(span, "expected expression here")
}

/// Create an "unclosed delimiter" diagnostic.
pub fn unclosed_delimiter(open_span: Span, close_span: Span, delimiter: char) -> Diagnostic {
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
        .with_message(format!(
            "missing required argument `.{arg}:` in `{pattern}` pattern"
        ))
        .with_label(span, format!("missing `.{arg}:`"))
        .with_suggestion(format!("add `.{arg}: <value>` to the pattern arguments"))
}

/// Configuration for an unknown pattern argument diagnostic.
///
/// Used by `unknown_pattern_arg` to create a diagnostic with all relevant context.
/// This config struct pattern improves API clarity for functions with 4+ parameters.
#[derive(Clone, Debug)]
pub struct UnknownPatternArgConfig<'a> {
    /// The source location of the unknown argument.
    pub span: Span,
    /// The pattern name (e.g., "map", "filter").
    pub pattern: &'a str,
    /// The unknown argument name.
    pub arg: &'a str,
    /// The list of valid argument names.
    pub valid: &'a [&'a str],
}

impl<'a> UnknownPatternArgConfig<'a> {
    /// Create a new unknown pattern argument configuration.
    pub fn new(span: Span, pattern: &'a str, arg: &'a str, valid: &'a [&'a str]) -> Self {
        UnknownPatternArgConfig {
            span,
            pattern,
            arg,
            valid,
        }
    }

    /// Convert this configuration into a diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        let valid_list = self.valid.join("`, `.");
        Diagnostic::error(ErrorCode::E1010)
            .with_message(format!(
                "unknown argument `.{}:` in `{}` pattern",
                self.arg, self.pattern
            ))
            .with_label(self.span, "unknown argument")
            .with_note(format!("valid arguments are: `.{valid_list}`"))
    }
}

/// Create an "unknown pattern argument" diagnostic.
///
/// For more explicit parameter naming, use `UnknownPatternArgConfig::new(...).into_diagnostic()`.
pub fn unknown_pattern_arg(span: Span, pattern: &str, arg: &str, valid: &[&str]) -> Diagnostic {
    UnknownPatternArgConfig::new(span, pattern, arg, valid).into_diagnostic()
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
