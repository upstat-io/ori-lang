//! Rich diagnostic detail types for Elm-quality error messages.

use ori_diagnostic::{Applicability, Diagnostic, ErrorCode, SourceInfo};
use ori_ir::Span;

/// A related location for richer error context.
///
/// Used to point to related code, like where a delimiter was opened.
/// This will be used for multi-span diagnostics in a future enhancement.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code, reason = "infrastructure for multi-span diagnostics")]
pub(crate) struct Note {
    /// The message explaining this location.
    pub message: String,
    /// The related source location, if any.
    pub span: Option<Span>,
}

/// A secondary label pointing to related code.
///
/// Extra labels provide additional context by highlighting related locations.
/// They're particularly useful for errors like:
/// - "unclosed delimiter" -> pointing to where it was opened
/// - "type mismatch" -> pointing to the expected type declaration
/// - "duplicate definition" -> pointing to the first definition
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
pub(crate) struct ExtraLabel {
    /// The source location to highlight.
    pub span: Span,
    /// Optional source info if this label is in a different file.
    pub src_info: Option<SourceInfo>,
    /// The label text explaining this location.
    pub text: String,
}

#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
impl ExtraLabel {
    /// Create a label in the same file.
    pub fn same_file(span: Span, text: impl Into<String>) -> Self {
        Self {
            span,
            src_info: None,
            text: text.into(),
        }
    }

    /// Create a label in a different file.
    pub fn cross_file(
        span: Span,
        path: impl Into<String>,
        content: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            span,
            src_info: Some(SourceInfo {
                path: path.into(),
                content: content.into(),
            }),
            text: text.into(),
        }
    }
}

/// A code suggestion for auto-fixing an error.
///
/// Suggestions are machine-readable fix instructions that can be:
/// - Applied automatically by formatters/IDEs
/// - Shown to users as "quick fixes"
/// - Used in batch refactoring tools
///
/// # Example
///
/// For the error "use `==` instead of `===`":
/// ```ignore
/// CodeSuggestion {
///     span: Span::new(10, 3), // The "===" location
///     replacement: "==".to_string(),
///     message: "Replace `===` with `==`".to_string(),
///     applicability: Applicability::MachineApplicable,
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
pub(crate) struct CodeSuggestion {
    /// The span to replace (what to remove).
    pub span: Span,
    /// The replacement text (what to insert).
    pub replacement: String,
    /// Human-readable description of the fix.
    pub message: String,
    /// Confidence level for auto-application.
    pub applicability: Applicability,
}

#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
impl CodeSuggestion {
    /// Create a machine-applicable suggestion (safe to auto-apply).
    pub fn machine_applicable(
        span: Span,
        replacement: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span,
            replacement: replacement.into(),
            message: message.into(),
            applicability: Applicability::MachineApplicable,
        }
    }

    /// Create a suggestion that may need review.
    pub fn maybe_incorrect(
        span: Span,
        replacement: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span,
            replacement: replacement.into(),
            message: message.into(),
            applicability: Applicability::MaybeIncorrect,
        }
    }

    /// Create a suggestion with placeholders (don't auto-apply).
    pub fn with_placeholders(
        span: Span,
        replacement: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            span,
            replacement: replacement.into(),
            message: message.into(),
            applicability: Applicability::HasPlaceholders,
        }
    }
}

/// Comprehensive error details for rich diagnostics.
///
/// This struct contains all the information needed to render a beautiful,
/// Elm-quality error message. It separates the error's semantic content
/// from its presentation, enabling:
///
/// - CLI rendering with colors and underlines
/// - IDE integration (LSP diagnostics + quick fixes)
/// - Machine-readable error reports
///
/// # Example Output
///
/// ```text
/// -- UNEXPECTED TOKEN -------------------------------- src/main.ori:10:5
///
/// I ran into something unexpected while parsing an if expression:
///
///    9 |     if count > 0 then
///   10 |         "positive"
///   11 |     else
///   12 |         count
///        ^^^^^
///        found identifier, expected expression
///
/// Hint: The else branch needs an expression. Did you mean to add
/// something after `count`?
/// ```
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
pub(crate) struct ParseErrorDetails {
    /// Error title (e.g., "UNEXPECTED TOKEN", "UNCLOSED DELIMITER").
    ///
    /// Displayed prominently at the top of the error message.
    pub title: &'static str,

    /// Main explanation text using empathetic phrasing.
    ///
    /// This is the human-readable description using first-person language
    /// like "I ran into..." or "I was expecting...".
    pub text: String,

    /// Inline label at the primary error location.
    ///
    /// Appears directly below the code snippet, explaining what was found
    /// or expected at this exact position.
    pub label_text: String,

    /// Additional labels for related locations.
    ///
    /// Used to show context like "the `{` was opened here" for unclosed
    /// delimiter errors. May reference other files.
    pub extra_labels: Vec<ExtraLabel>,

    /// Actionable hint for fixing the error.
    ///
    /// Provides concrete guidance on how to resolve the issue.
    pub hint: Option<String>,

    /// Machine-applicable code suggestion for auto-fix.
    ///
    /// When present, IDEs can offer "quick fix" functionality.
    pub suggestion: Option<CodeSuggestion>,

    /// Structured error code (e.g., E1001).
    pub error_code: ErrorCode,
}

#[allow(
    dead_code,
    reason = "infrastructure for ParseErrorKind rich diagnostic system"
)]
impl ParseErrorDetails {
    /// Create new error details with required fields.
    pub fn new(
        title: &'static str,
        text: impl Into<String>,
        label_text: impl Into<String>,
        error_code: ErrorCode,
    ) -> Self {
        Self {
            title,
            text: text.into(),
            label_text: label_text.into(),
            extra_labels: Vec::new(),
            hint: None,
            suggestion: None,
            error_code,
        }
    }

    /// Add an extra label for related context.
    #[must_use]
    pub fn with_extra_label(mut self, label: ExtraLabel) -> Self {
        self.extra_labels.push(label);
        self
    }

    /// Add a hint for fixing the error.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Add a code suggestion for auto-fix.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: CodeSuggestion) -> Self {
        self.suggestion = Some(suggestion);
        self
    }

    /// Check if this error has any extra context (labels, hints, or suggestions).
    pub fn has_extra_context(&self) -> bool {
        !self.extra_labels.is_empty() || self.hint.is_some() || self.suggestion.is_some()
    }

    /// Convert to a `Diagnostic` for rendering.
    ///
    /// This bridges the parser's rich error infrastructure with the diagnostic
    /// system, enabling cross-file labels and structured suggestions to flow
    /// through to the terminal/JSON/SARIF emitters.
    ///
    /// # Arguments
    ///
    /// * `primary_span` - The span of the primary error location
    pub fn to_diagnostic(&self, primary_span: Span) -> Diagnostic {
        let mut diag = Diagnostic::error(self.error_code)
            .with_message(&self.text)
            .with_label(primary_span, &self.label_text);

        // Add extra labels (supports both same-file and cross-file)
        for extra in &self.extra_labels {
            if let Some(ref src_info) = extra.src_info {
                diag =
                    diag.with_cross_file_secondary_label(extra.span, &extra.text, src_info.clone());
            } else {
                diag = diag.with_secondary_label(extra.span, &extra.text);
            }
        }

        // Add hint as a suggestion
        if let Some(ref hint) = self.hint {
            diag = diag.with_suggestion(hint);
        }

        // Add code suggestion as structured fix
        if let Some(ref suggestion) = self.suggestion {
            diag = diag.with_structured_suggestion(ori_diagnostic::Suggestion::new(
                &suggestion.message,
                suggestion.span,
                &suggestion.replacement,
                suggestion.applicability,
                0,
            ));
        }

        diag
    }
}
