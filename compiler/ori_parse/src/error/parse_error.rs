//! The `ParseError` struct and its constructors.

use ori_diagnostic::queue::DiagnosticSeverity;
use ori_diagnostic::ErrorCode;
use ori_ir::{Span, TokenKind};

use super::kind::ParseErrorKind;
use super::mistakes::detect_common_mistake;

/// Parse error with error code for rich diagnostics.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseError {
    /// Error code for searchability.
    pub(crate) code: ErrorCode,
    /// Human-readable message.
    pub(crate) message: String,
    /// Location of the error.
    pub(crate) span: Span,
    /// Optional context for suggestions.
    pub(crate) context: Option<String>,
    /// Optional help messages.
    pub(crate) help: Vec<String>,
    /// Severity level for diagnostic queue suppression.
    ///
    /// `Hard` errors are always reported; `Soft` errors (from `EmptyErr` — the parser
    /// didn't consume any tokens) can be suppressed after a hard error to reduce noise.
    pub(crate) severity: DiagnosticSeverity,
}

impl ParseError {
    // --- Accessors ---

    /// Error code for searchability.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Location of the error.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Optional context for suggestions.
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Optional help messages.
    pub fn help(&self) -> &[String] {
        &self.help
    }

    /// Severity level for diagnostic queue suppression.
    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    // --- Constructors ---

    /// Create a new parse error.
    ///
    /// Defaults to `Hard` severity (always reported).
    #[cold]
    pub fn new(code: ori_diagnostic::ErrorCode, message: impl Into<String>, span: Span) -> Self {
        ParseError {
            code,
            message: message.into(),
            span,
            context: None,
            help: Vec::new(),
            severity: DiagnosticSeverity::Hard,
        }
    }

    // --- Series Combinator Helpers ---

    /// Error when expecting an item in a series but none was found.
    #[cold]
    pub fn expected_item(span: Span, terminator: &TokenKind) -> Self {
        ParseError::new(
            ErrorCode::E1002,
            format!("expected item before `{}`", terminator.display_name()),
            span,
        )
    }

    /// Error when a trailing separator was found but not allowed.
    #[cold]
    pub fn unexpected_trailing_separator(span: Span, separator: &TokenKind) -> Self {
        ParseError::new(
            ErrorCode::E1001,
            format!("unexpected trailing `{}`", separator.display_name()),
            span,
        )
    }

    /// Error when expecting separator or terminator but found something else.
    #[cold]
    pub fn expected_separator_or_terminator(
        span: Span,
        separator: &TokenKind,
        terminator: &TokenKind,
    ) -> Self {
        ParseError::new(
            ErrorCode::E1001,
            format!(
                "expected `{}` or `{}`",
                separator.display_name(),
                terminator.display_name()
            ),
            span,
        )
    }

    /// Error when a series has too few items.
    #[cold]
    pub fn too_few_items(span: Span, min: usize, actual: usize) -> Self {
        ParseError::new(
            ErrorCode::E1002,
            format!("expected at least {min} items, found {actual}"),
            span,
        )
    }

    /// Error when a series has too many items.
    #[cold]
    pub fn too_many_items(span: Span, max: usize, actual: usize) -> Self {
        ParseError::new(
            ErrorCode::E1002,
            format!("expected at most {max} items, found {actual}"),
            span,
        )
    }

    /// Create a parse error from a set of expected tokens.
    ///
    /// Used by `ParseOutcome::EmptyErr` when converting to `ParseError`.
    /// The `position` is a **byte offset** in the source, converted to a
    /// zero-length span at that location.
    ///
    /// Returns a `Soft` error: the parser didn't consume any tokens, so this
    /// is a speculative failure that can be suppressed after a hard error.
    #[cold]
    pub fn from_expected_tokens(expected: &crate::TokenSet, position: usize) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "position fits in u32 for source files"
        )]
        let span = Span::point(position as u32);
        let expected_str = expected.format_expected();
        ParseError::new(ErrorCode::E1001, format!("expected {expected_str}"), span).as_soft()
    }

    /// Create a parse error from expected tokens with additional context.
    ///
    /// Used by the `require!` macro to convert soft errors to hard errors
    /// with context about what was being parsed.
    #[cold]
    pub fn from_expected_tokens_with_context(
        expected: &crate::TokenSet,
        position: usize,
        context: &str,
    ) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "position fits in u32 for source files"
        )]
        let span = Span::point(position as u32);
        let expected_str = expected.format_expected();
        ParseError::new(ErrorCode::E1002, format!("expected {expected_str}"), span)
            .with_context(format!("while parsing {context}"))
    }

    /// Add context for better error messages.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Add a help message.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help.push(help.into());
        self
    }

    /// Mark this error as soft (suppressible after a hard error).
    #[must_use]
    pub fn as_soft(mut self) -> Self {
        self.severity = DiagnosticSeverity::Soft;
        self
    }

    /// Convert to a full Diagnostic for rich error reporting.
    pub fn to_diagnostic(&self) -> ori_diagnostic::Diagnostic {
        let mut diag = ori_diagnostic::Diagnostic::error(self.code)
            .with_message(&self.message)
            .with_label(self.span, self.context.as_deref().unwrap_or("here"));

        for help in &self.help {
            diag = diag.with_note(help);
        }

        diag
    }

    /// Convert to a Diagnostic bundled with severity for `DiagnosticQueue` routing.
    ///
    /// Convenience method that avoids callers needing to separately access
    /// `to_diagnostic()` and `severity()` when routing through a `DiagnosticQueue`.
    pub fn to_queued_diagnostic(&self) -> (ori_diagnostic::Diagnostic, DiagnosticSeverity) {
        (self.to_diagnostic(), self.severity)
    }

    /// Create a [`ParseError`] from a structured [`ParseErrorKind`].
    ///
    /// This is the preferred way to create errors in new code.
    /// The kind provides all context needed to generate helpful messages.
    #[cold]
    pub fn from_kind(kind: &ParseErrorKind, span: Span) -> Self {
        let code = kind.error_code();
        let message = kind.message();
        let hint = kind.hint();
        let educational = kind.educational_note();

        let mut error = ParseError {
            code,
            message,
            span,
            context: None,
            help: Vec::new(),
            severity: DiagnosticSeverity::Hard,
        };

        // Add hint first (most actionable)
        if let Some(hint) = hint {
            error.help.push(hint.to_string());
        }

        // Add educational note (more context)
        if let Some(note) = educational {
            error.help.push(note.to_string());
        }

        // Add related location context for unclosed delimiters
        if let ParseErrorKind::UnclosedDelimiter {
            open_span, open, ..
        } = &kind
        {
            error.context = Some(format!("`{}` opened here", open.display_name()));
            // Note: the open_span would be used in a multi-span diagnostic
            let _ = open_span; // Mark as intentionally unused for now
        }

        error
    }

    /// Create a [`ParseError`] for an error token with source-based mistake detection.
    ///
    /// This examines the actual source text that caused the lexer error
    /// to provide targeted help for common patterns from other languages.
    #[cold]
    pub fn from_error_token(span: Span, source_text: &str) -> Self {
        if let Some((description, help)) = detect_common_mistake(source_text) {
            ParseError {
                code: ErrorCode::E1001,
                message: format!("unrecognized {description}: `{source_text}`"),
                span,
                context: None,
                help: vec![help.to_string()],
                severity: DiagnosticSeverity::Hard,
            }
        } else {
            ParseError {
                code: ErrorCode::E1001,
                message: format!("unrecognized token: `{source_text}`"),
                span,
                context: None,
                help: Vec::new(),
                severity: DiagnosticSeverity::Hard,
            }
        }
    }
}

impl From<(ParseErrorKind, Span)> for ParseError {
    fn from((kind, span): (ParseErrorKind, Span)) -> Self {
        ParseError::from_kind(&kind, span)
    }
}
